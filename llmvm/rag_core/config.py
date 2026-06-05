"""
Configuration for Aetheris RAG Core.
All settings centralized. Override via environment variables.
"""

import os
from dataclasses import dataclass, field


@dataclass
class RAGConfig:
    # AI endpoint (Ollama or OpenAI-compatible)
    ai_endpoint: str = os.environ.get("AI_ENDPOINT", "http://ollama:11434")

    # Default models
    chat_model: str = os.environ.get("AI_MODEL", "qwen2.5:14b")
    embedding_model: str = os.environ.get("EMBEDDING_MODEL", "text-embedding-nomic-embed-text-v1.5")

    # API key for remote endpoints
    api_key: str = os.environ.get("AI_API_KEY", "")

    # Chunking
    chunk_size: int = int(os.environ.get("CHUNK_SIZE", "512"))
    chunk_overlap: int = int(os.environ.get("CHUNK_OVERLAP", "64"))

    # Retrieval
    top_k: int = int(os.environ.get("TOP_K", "5"))
    similarity_threshold: float = float(os.environ.get("SIMILARITY_THRESHOLD", "0.65"))
    score_threshold: float = float(os.environ.get("SCORE_THRESHOLD", "0.0"))
    rrf_k: int = int(os.environ.get("RRF_K", "60"))
    max_history: int = int(os.environ.get("MAX_HISTORY", "10"))

    # Generation
    default_model: str = os.environ.get("AI_MODEL", "qwen2.5:14b")
    temperature: float = float(os.environ.get("TEMPERATURE", "0.1"))
    max_tokens: int = int(os.environ.get("MAX_TOKENS", "2048"))
    request_timeout: int = int(os.environ.get("REQUEST_TIMEOUT", "120"))

    # Supported file extensions for directory ingest
    supported_extensions: list = field(default_factory=lambda: [".txt", ".md", ".py", ".rs", ".js", ".ts", ".html", ".css", ".json", ".yaml", ".yml", ".toml", ".cfg", ".ini"])

    # System prompt
    system_prompt: str = os.environ.get(
        "SYSTEM_PROMPT",
        "You are Aetheris, a personal AI assistant. Answer questions based on the provided context. "
        "If the context doesn't contain relevant information, say so clearly. "
        "Never fabricate information."
    )

    # Vector store
    db_path: str = os.environ.get("RAG_DB_PATH", "/app/rag_data/vectors.db")

    # File storage paths
    upload_dir: str = os.environ.get("RAG_UPLOAD_DIR", "/app/uploads")
    storage_dir: str = os.environ.get("RAG_STORAGE_DIR", "/app/storage")
    max_upload_size: int = int(os.environ.get("RAG_MAX_UPLOAD_MB", "50")) * 1024 * 1024  # 50MB default

    # Knowledge graph
    graph_db_path: str = os.environ.get("RAG_GRAPH_DB_PATH", "/app/rag_data/knowledge_graph.db")
    graph_extraction_enabled: bool = os.environ.get("RAG_GRAPH_EXTRACT", "true").lower() == "true"
    graph_entity_types: list = field(default_factory=lambda: [
        "concept", "person", "organization", "location",
        "technology", "file", "service", "configuration",
        "event", "tool"
    ])
    graph_relation_types: list = field(default_factory=lambda: [
        "depends_on", "references", "contains", "created_by",
        "located_at", "uses", "configures", "belongs_to",
        "related_to", "implements"
    ])

    @property
    def headers(self) -> dict:
        h = {"Content-Type": "application/json"}
        if self.api_key:
            h["Authorization"] = f"Bearer {self.api_key}"
        return h


# Singleton
config = RAGConfig()
