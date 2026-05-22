"""
Embedder client for LMStudio / OpenAI-compatible API.
Generates embeddings for text chunks via /v1/embeddings endpoint.
"""

import numpy as np
from typing import List, Optional
from .config import RAGConfig, config
from .chunker import Chunk


class Embedder:
    """Generates embeddings via LMStudio embedding model."""

    def __init__(self, cfg: Optional[RAGConfig] = None):
        self.cfg = cfg or config
        self._dimension: Optional[int] = None

    @property
    def dimension(self) -> int:
        """Cached embedding dimension (detected on first call)."""
        if self._dimension is None:
            # Probe with a test embedding
            test = self.embed(["test"])[0]
            self._dimension = len(test)
        return self._dimension

    def embed(self, texts: List[str]) -> List[np.ndarray]:
        """
        Generate embeddings for a list of texts.
        Returns list of numpy arrays (float32).

        Batches if needed to avoid request size limits.
        """
        if not texts:
            return []

        # LMStudio supports batch embedding
        import requests

        payload = {
            "model": self.cfg.embedding_model,
            "input": texts,
            "encoding_format": "float"
        }

        url = f"{self.cfg.ai_endpoint}/v1/embeddings"
        resp = requests.post(url, json=payload, headers=self.cfg.headers, timeout=60)

        if resp.status_code != 200:
            raise RuntimeError(f"Embedding API error {resp.status_code}: {resp.text}")

        data = resp.json()
        # Sort by index (API may return out of order)
        embeddings = sorted(data["data"], key=lambda x: x["index"])
        return [np.array(e["embedding"], dtype=np.float32) for e in embeddings]

    def embed_chunks(self, chunks: List[Chunk]) -> List[np.ndarray]:
        """Embed a list of Chunks (batched)."""
        texts = [c.text for c in chunks]
        return self.embed(texts)

    def embed_query(self, query: str) -> np.ndarray:
        """Embed a single query string."""
        return self.embed([query])[0]


# Convenience function
def embed_text(text: str, cfg: Optional[RAGConfig] = None) -> np.ndarray:
    """Quick embed a single string."""
    return Embedder(cfg).embed_query(text)
