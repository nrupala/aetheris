"""
Pipeline — End-to-end RAG orchestration with reasoning loop and KG integration.

Usage:
    pipeline = RAGPipeline()
    result = pipeline.query("How do I configure WireGuard?")
    
    # With reasoning loop
    result = pipeline.query("Complex question", reasoning=True, max_iterations=3)

For ingest:
    pipeline.ingest_file("docs/manual.pdf")
    pipeline.ingest_directory("docs/")
    
    # With KG entity extraction
    pipeline.ingest_file("docs/manual.pdf", extract_entities=True)
"""

import os
import time
import uuid
import logging
from typing import List, Optional, Dict
from dataclasses import dataclass, field
from datetime import datetime

from .config import RAGConfig, config
from .chunker import TextChunker as Chunker, Chunk, chunk_file
from .embedder import Embedder
from .vector_store import VectorStore
from .retriever import Retriever
from .generator import Generator, LLMResponse
from .model_router import create_default_router, Agent

logger = logging.getLogger(__name__)


@dataclass
class RAGResult:
    """Complete RAG query result."""
    answer: str
    sources: List[Dict]
    query: str
    model: str
    response_time: float
    tokens_used: int
    chunks_searched: int
    # Reasoning loop fields
    confidence: float = 0.0
    iterations_used: int = 1
    reasoning_trace: List[Dict] = field(default_factory=list)
    converged: bool = False
    verification: Dict = field(default_factory=dict)


class RAGPipeline:
    """Main RAG pipeline: Ingest → Embed → Store → Retrieve → Generate."""

    def __init__(self, cfg: Optional[RAGConfig] = None, kg=None):
        self.cfg = cfg or config
        self.kg = kg
        self.chunker = Chunker(chunk_size=self.cfg.chunk_size, chunk_overlap=self.cfg.chunk_overlap)
        self.embedder = Embedder(cfg=self.cfg)
        self.store = VectorStore(cfg=self.cfg)
        self.retriever = Retriever(
            store=self.store,
            embedder=self.embedder,
            cfg=self.cfg
        )
        self.generator = Generator(cfg=self.cfg)
        self._history: List[Dict] = []
        
        # Reasoning loop components (lazy init)
        self._agent = None
        self._entity_extractor = None
    
    def _get_agent(self) -> Agent:
        """Lazy init model router + agent."""
        if self._agent is None:
            router = create_default_router(
                lmstudio_endpoint=self.cfg.ai_endpoint,
                lmstudio_model=self.cfg.chat_model,
            )
            self._agent = Agent(router)
        return self._agent
    
    def _get_entity_extractor(self):
        """Lazy init entity extractor."""
        if self._entity_extractor is None:
            from .reasoning_loop import EntityExtractor
            self._entity_extractor = EntityExtractor(self._get_agent())
        return self._entity_extractor

    def ingest_file(self, file_path: str, metadata: Optional[Dict] = None,
                    progress_callback=None, extract_entities: bool = False) -> Dict:
        """
        Ingest a single file: chunk → embed → store.
        
        Args:
            file_path: Path to file
            metadata: Optional metadata to attach
            progress_callback: Optional callback(step, count)
            extract_entities: Extract entities/relations into KG
        
        Returns:
            Ingest statistics
        """
        if not os.path.exists(file_path):
            raise FileNotFoundError(f"File not found: {file_path}")

        start = time.time()

        if progress_callback:
            progress_callback("chunking", 0)
        chunks = chunk_file(file_path, chunk_size=self.chunker.chunk_size, chunk_overlap=self.chunker.chunk_overlap)
        if progress_callback:
            progress_callback("chunking", len(chunks))

        if metadata:
            for chunk in chunks:
                chunk.metadata.update(metadata)

        if progress_callback:
            progress_callback("embedding", 0)
        embeddings = self.embedder.embed_chunks(chunks)
        if progress_callback:
            progress_callback("embedding", len(chunks))

        if progress_callback:
            progress_callback("storing", 0)
        chunk_ids = self.store.add(chunks, embeddings)
        if progress_callback:
            progress_callback("storing", len(chunk_ids))

        # KG entity extraction
        kg_stats = {}
        if extract_entities and self.kg:
            try:
                full_text = "\n".join(c.text for c in chunks)
                extractor = self._get_entity_extractor()
                kg_stats = extractor.ingest_to_kg(full_text, metadata.get("source", file_path), self.kg)
                if progress_callback:
                    progress_callback("kg_extract", kg_stats.get("entities_added", 0))
            except Exception as e:
                logger.warning(f"KG entity extraction failed: {e}")
                kg_stats = {"error": str(e)}

        elapsed = time.time() - start

        return {
            "file": file_path,
            "chunks_created": len(chunks),
            "chunk_ids": chunk_ids,
            "time_seconds": round(elapsed, 2),
            "chunks_per_second": round(len(chunks) / max(elapsed, 0.01), 1),
            "kg_stats": kg_stats,
        }

    def ingest_directory(
        self,
        dir_path: str,
        extensions: Optional[List[str]] = None,
        progress_callback=None,
        extract_entities: bool = False,
    ) -> Dict:
        """
        Ingest all files in a directory recursively.

        Args:
            dir_path: Path to scan
            extensions: File extensions to include
            progress_callback: Optional callback(file_path, status)
            extract_entities: Extract entities into KG for each file

        Returns:
            Directory ingest summary
        """
        extensions = extensions or self.cfg.supported_extensions
        start = time.time()

        total_files = 0
        total_chunks = 0
        total_entities = 0
        results = []

        for root, _, files in os.walk(dir_path):
            for fname in sorted(files):
                ext = os.path.splitext(fname)[1].lower()
                if ext not in extensions:
                    continue

                file_path = os.path.join(root, fname)
                total_files += 1

                if progress_callback:
                    progress_callback(file_path, "chunking")

                try:
                    stats = self.ingest_file(
                        file_path,
                        metadata={"source": file_path},
                        progress_callback=None,
                        extract_entities=extract_entities,
                    )
                    total_chunks += stats["chunks_created"]
                    total_entities += stats.get("kg_stats", {}).get("entities_added", 0)
                    results.append(stats)

                    if progress_callback:
                        status = f"done ({stats['chunks_created']} chunks"
                        if stats.get("kg_stats", {}).get("entities_added"):
                            status += f", {stats['kg_stats']['entities_added']} entities"
                        status += ")"
                        progress_callback(file_path, status)

                except Exception as e:
                    if progress_callback:
                        progress_callback(file_path, f"error: {str(e)}")

        elapsed = time.time() - start

        return {
            "directory": dir_path,
            "files_processed": total_files,
            "total_chunks": total_chunks,
            "total_entities_extracted": total_entities,
            "files_results": results,
            "time_seconds": round(elapsed, 2)
        }

    def ingest_text(self, text: str, source: str, metadata: Optional[Dict] = None,
                    extract_entities: bool = False) -> Dict:
        """
        Ingest raw text string (for API-ingested content).
        """
        chunks = self.chunker.chunk(text, source)
        if metadata:
            for chunk in chunks:
                chunk.metadata.update(metadata)

        embeddings = self.embedder.embed_chunks(chunks)
        chunk_ids = self.store.add(chunks, embeddings)

        kg_stats = {}
        if extract_entities and self.kg:
            try:
                extractor = self._get_entity_extractor()
                kg_stats = extractor.ingest_to_kg(text, source, self.kg)
            except Exception as e:
                logger.warning(f"KG entity extraction failed: {e}")

        return {
            "source": source,
            "chunks_created": len(chunks),
            "chunk_ids": chunk_ids,
            "kg_stats": kg_stats,
        }

    def query(
        self,
        query: str,
        top_k: Optional[int] = None,
        source_filter: Optional[str] = None,
        threshold: Optional[float] = None,
        use_rag: bool = True,
        include_history: bool = True,
        # Reasoning loop params
        reasoning: bool = False,
        max_iterations: int = 3,
        confidence_threshold: float = 0.7,
        task_id: Optional[str] = None,
    ) -> RAGResult:
        """
        Full RAG query: retrieve → generate.

        Args:
            query: User question
            top_k: Number of context chunks to retrieve
            source_filter: Limit search to specific source
            threshold: Minimum similarity score
            use_rag: If False, skip retrieval (pure LLM)
            include_history: Include conversation history
            reasoning: Enable iterative reasoning loop
            max_iterations: Max reasoning iterations (1-10)
            confidence_threshold: Confidence to stop iterating
            task_id: Task ID for Pregel checkpointing

        Returns:
            RAGResult with answer and metadata
        """
        start = time.time()

        if use_rag:
            context = self.retriever.retrieve(
                query,
                top_k=top_k,
                source_filter=source_filter,
                threshold=threshold
            )

            # Inject KG personal context
            personal_context = ""
            if self.kg:
                personal_context = self.kg.get_personal_context(query)

            if reasoning:
                # Use reasoning loop
                if not task_id:
                    task_id = f"query_{uuid.uuid4().hex[:12]}"
                
                from .reasoning_loop import ReasoningLoop
                
                loop = ReasoningLoop(
                    agent=self._get_agent(),
                    task_id=task_id,
                    workspace_root=os.environ.get("WORKSPACE_ROOT", "/workspace"),
                )
                
                result = loop.run(
                    question=query,
                    context=context,
                    max_iterations=max_iterations,
                    confidence_threshold=confidence_threshold,
                )
                
                reasoning_trace = result.reasoning_trace
                confidence = result.confidence
                iterations_used = result.iterations_used
                converged = result.converged
                verification = result.verification
                answer = result.answer
                tokens_used = result.tokens_used
                model = result.model
                sources = [{"source": s, "score": 0} for s in result.sources_used]
                
            else:
                # Standard generation
                history = self._history if include_history else []
                
                # Build system prompt with personal context
                system_prompt = self.generator._build_system_prompt(
                    self.generator._format_context(context)
                )
                if personal_context:
                    system_prompt += f"\n\n## Personal Context\n{personal_context}"
                
                # Override system prompt temporarily
                original_system_prompt = self.generator._build_system_prompt
                self.generator._build_system_prompt = lambda ctx: system_prompt
                
                try:
                    response = self.generator.generate(query, context, history)
                finally:
                    self.generator._build_system_prompt = original_system_prompt

                if include_history:
                    self._history.append({"role": "user", "content": query})
                    self._history.append({"role": "assistant", "content": response.text})
                    if len(self._history) > self.cfg.max_history * 2:
                        self._history = self._history[-self.cfg.max_history * 2:]
                
                reasoning_trace = []
                confidence = 0.0
                iterations_used = 1
                converged = True
                verification = {}
                answer = response.text
                tokens_used = response.usage.get("total_tokens", 0)
                model = response.model
                sources = [
                    {
                        "source": r.source,
                        "score": round(r.score, 3),
                        "chunk_index": r.chunk_index
                    }
                    for r in context
                ]

        else:
            history = self._history if include_history else []
            response = self.generator.generate_direct(query, history)
            context = []

            if include_history:
                self._history.append({"role": "user", "content": query})
                self._history.append({"role": "assistant", "content": response.text})
            
            reasoning_trace = []
            confidence = 0.0
            iterations_used = 1
            converged = True
            verification = {}
            answer = response.text
            tokens_used = response.usage.get("total_tokens", 0)
            model = response.model
            sources = []

        elapsed = time.time() - start

        return RAGResult(
            answer=answer,
            sources=sources,
            query=query,
            model=model,
            response_time=round(elapsed, 3),
            tokens_used=tokens_used,
            chunks_searched=len(context),
            confidence=confidence,
            iterations_used=iterations_used,
            reasoning_trace=reasoning_trace,
            converged=converged,
            verification=verification,
        )

    def clear_history(self):
        """Reset conversation history."""
        self._history = []

    def delete_source(self, source: str) -> int:
        """Remove all content from a source."""
        return self.store.delete_source(source)

    def list_sources(self) -> List[Dict]:
        """List all indexed sources."""
        return self.store.list_sources()

    def stats(self) -> Dict:
        """Get pipeline statistics."""
        return self.store.stats()

    def reset(self):
        """Delete all data and reset the store."""
        import os
        self.store.close()
        if os.path.exists(self.cfg.db_path):
            os.remove(self.cfg.db_path)
        self.store = VectorStore(cfg=self.cfg)
        self.retriever = Retriever(
            store=self.store,
            embedder=self.embedder,
            cfg=self.cfg
        )

    def close(self):
        self.store.close()

    def __enter__(self):
        return self

    def __exit__(self, *args):
        self.close()
