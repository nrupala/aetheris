"""
Aetheris RAG Core — Lightweight Retrieval-Augmented Generation.

No LangChain. No Milvus. No Haystack.
Just clean, focused modules that do one thing well.

Modules:
    config            — Configuration (LMStudio endpoint, paths, models)
    chunker           — Semantic document chunking with tiktoken
    embedder          — Vector embedding via LMStudio /v1/embeddings
    vector_store      — SQLite + NumPy cosine similarity search
    retriever         — Semantic search with source filtering
    generator         — LLM answer generation via /v1/chat/completions
    pipeline          — End-to-end RAG orchestration
    knowledge_graph   — Personal knowledge graph (entities, relations, profile)
    coordinator       — Processing coordinator (state machine, circuit breaker, audit)
    model_router      — Model abstraction (Pydantic AI, provider routing, fallback)
    reasoning_loop    — Iterative self-improving reasoning with Pregel checkpointing
    pregel_checkpoint — Deterministic execution with crash recovery
    a2a_protocol      — Agent-to-Agent message protocol with OPA gate
"""

__version__ = "2.0.0"
__all__ = [
    "RAGConfig",
    "Chunker",
    "Embedder",
    "VectorStore",
    "Retriever",
    "Generator",
    "RAGPipeline",
    "KnowledgeGraph",
    "ProcessingCoordinator",
    "ModelRouter",
    "ReasoningLoop",
    "PregelCheckpoint",
    "A2AMessageBus",
    "A2ARouter",
    "MessageFactory",
    "EntityExtractor",
]

from .config import RAGConfig, config
from .chunker import TextChunker as Chunker, Chunk
from .embedder import Embedder
from .vector_store import VectorStore
from .retriever import Retriever
from .generator import Generator
from .pipeline import RAGPipeline
from .knowledge_graph import KnowledgeGraph
from .coordinator import ProcessingCoordinator, get_coordinator
from .model_router import ModelRouter, create_default_router, Agent
from .reasoning_loop import ReasoningLoop, EntityExtractor
from .pregel_checkpoint import PregelCheckpoint, CheckpointManager
from .a2a_protocol import A2AMessageBus, A2ARouter, MessageFactory
