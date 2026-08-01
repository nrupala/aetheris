"""Ollama chat/generation client. Talks to a LOCAL Ollama on the same box
(127.0.0.1:11434) -- never through the Access-gated public hostname. Used by the
RAG layer (rag_answer) to turn retrieved local context into a grounded, cited
answer. Non-streaming and deterministic by default (temperature 0)."""
from __future__ import annotations
from typing import Dict, List
import httpx
from . import config


def ollama_chat(messages: List[Dict[str, str]], *, url: str = None, model: str = None,
                timeout: float = 120.0, num_ctx: int = None, temperature: float = 0.0) -> str:
    """POST to Ollama /api/chat and return the assistant message content."""
    url = url or config.OLLAMA_URL
    model = model or config.CHAT_MODEL
    options: Dict[str, object] = {"temperature": temperature}
    if num_ctx:
        options["num_ctx"] = num_ctx
    resp = httpx.post(
        f"{url}/api/chat",
        json={"model": model, "messages": messages, "stream": False, "options": options},
        timeout=timeout,
    )
    resp.raise_for_status()
    data = resp.json()
    content = (data.get("message") or {}).get("content")
    if content is None:
        raise RuntimeError(f"Ollama returned no message content: {data}")
    return content
