"""
Retriever — combines vector similarity with intelligent re-ranking.

Strategies:
1. Pure vector search (fast, good enough for most queries)
2. Source-filtered search (when you know where to look)
3. Multi-query expansion (query variations for better recall)
4. Reciprocal Rank Fusion (RRF) for combining multiple result sets
"""

from typing import List, Optional
from dataclasses import dataclass
import numpy as np

from .config import RAGConfig, config
from .embedder import Embedder
from .vector_store import VectorStore, SearchResult


@dataclass
class RetrievalResult:
    """Final result after retrieval + re-ranking."""
    text: str
    source: str
    score: float
    chunk_index: int
    metadata: dict


class Retriever:
    """Semantic document retrieval with optional re-ranking."""

    def __init__(
        self,
        store: VectorStore,
        embedder: Embedder,
        cfg: Optional[RAGConfig] = None
    ):
        self.store = store
        self.embedder = embedder
        self.cfg = cfg or config

    def retrieve(
        self,
        query: str,
        top_k: Optional[int] = None,
        source_filter: Optional[str] = None,
        threshold: Optional[float] = None
    ) -> List[RetrievalResult]:
        """
        Retrieve relevant chunks for a query.

        Args:
            query: User's question/search
            top_k: Number of results (default from config)
            source_filter: Limit to specific source (e.g. "docs/")
            threshold: Minimum similarity score

        Returns:
            Ranked list of RetrievalResult
        """
        top_k = top_k or self.cfg.top_k
        threshold = threshold or self.cfg.score_threshold

        # Embed query
        query_vec = self.embedder.embed_query(query)

        # Search
        if source_filter:
            raw = self.store.search_by_source(
                query_vec, source_filter, top_k=top_k * 2, threshold=threshold
            )
        else:
            raw = self.store.search(query_vec, top_k=top_k * 2, threshold=threshold)

        # Convert and return
        results = [
            RetrievalResult(
                text=r.text,
                source=r.source,
                score=r.score,
                chunk_index=r.chunk_index,
                metadata=r.metadata
            )
            for r in raw
        ]

        return results[:top_k]

    def retrieve_multi_query(
        self,
        query: str,
        variations: List[str],
        top_k: Optional[int] = None
    ) -> List[RetrievalResult]:
        """
        Search with query variations and fuse results via RRF.
        Increases recall at cost of extra embedding calls.
        """
        top_k = top_k or self.cfg.top_k

        # Generate query + variations
        queries = [query] + variations
        result_sets = []

        for q in queries:
            results = self.retrieve(q, top_k=top_k * 2)
            result_sets.append(results)

        # Reciprocal Rank Fusion
        rrf_scores = {}
        for result_set in result_sets:
            for rank, result in enumerate(result_set):
                key = (result.text, result.source)
                rrf_score = 1.0 / (self.cfg.rrf_k + rank)
                if key not in rrf_scores:
                    rrf_scores[key] = {
                        "result": result,
                        "score": 0.0
                    }
                rrf_scores[key]["score"] += rrf_score

        # Sort by RRF score
        fused = sorted(rrf_scores.values(), key=lambda x: x["score"], reverse=True)
        return [item["result"] for item in fused[:top_k]]

    def retrieve_conversation(
        self,
        query: str,
        history: List[str],
        top_k: Optional[int] = None
    ) -> List[RetrievalResult]:
        """
        Retrieve using query + recent conversation history.
        Provides context for follow-up questions.
        """
        # Combine with recent history for better context
        recent = history[-self.cfg.max_history:]
        context_query = " ".join(recent + [query])
        return self.retrieve(context_query, top_k=top_k)


class HybridRetriever(Retriever):
    """
    Future: Add keyword search (BM25) + vector search hybrid.
    For now, this is a placeholder showing the extension point.
    """

    def retrieve_hybrid(self, query: str, top_k: int = 5) -> List[RetrievalResult]:
        """
        Combine vector similarity with keyword matching.
        TODO: Add simple TF-IDF or BM25 scoring here.
        """
        # Phase 1: Vector search
        vector_results = self.retrieve(query, top_k=top_k * 2)

        # Phase 2: Keyword boost (simple)
        query_terms = set(query.lower().split())
        for result in vector_results:
            text_terms = set(result.text.lower().split())
            overlap = len(query_terms & text_terms) / max(len(query_terms), 1)
            # Blend: 80% vector score + 20% keyword overlap
            result.score = 0.8 * result.score + 0.2 * overlap

        vector_results.sort(key=lambda r: r.score, reverse=True)
        return vector_results[:top_k]
