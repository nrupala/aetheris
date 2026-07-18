"""Ollama embeddings client. Talks to a LOCAL Ollama on the same box
(127.0.0.1:11434) -- never through the Access-gated public hostname."""
from __future__ import annotations
from typing import List
import httpx
from . import config


def embed_one(text: str, *, url: str = None, model: str = None, timeout: float = 60.0) -> List[float]:
    url = url or config.OLLAMA_URL
    model = model or config.EMBED_MODEL
    resp = httpx.post(f"{url}/api/embeddings", json={"model": model, "prompt": text}, timeout=timeout)
    resp.raise_for_status()
    data = resp.json()
    emb = data.get("embedding")
    if not emb:
        raise RuntimeError(f"Ollama returned no embedding: {data}")
    return emb


def embed_many(texts: List[str], **kw) -> List[List[float]]:
    return [embed_one(t, **kw) for t in texts]
