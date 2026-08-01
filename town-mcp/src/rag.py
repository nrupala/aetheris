"""Local RAG: retrieve top-k chunks from the sovereign store, then have a LOCAL
Ollama model compose a grounded answer that cites its sources by number.

Sovereign + trustworthy by design:
- Nothing leaves the box: retrieval (embeddings + vector store) and generation
  (Ollama) are both local.
- No ungrounded answers: if retrieval finds nothing above the relevance floor,
  rag_answer returns a clear "insufficient local context" result WITHOUT calling
  the model, so it never hallucinates past the corpus.
- Auditable: the exact context chunks are returned, and [n] citation markers map
  back to those sources.

The public entrypoint rag_answer() takes injectable `store`, `embed_fn`, and
`chat_fn` so the retrieve -> prompt -> cite wiring is unit-testable offline.
"""
from __future__ import annotations
from typing import Any, Callable, Dict, List
import re

from . import config
from .store import Store

SYSTEM_PROMPT = (
    "You are Aetheris, a sovereign assistant that answers strictly from the "
    "user's own private local documents. Use ONLY the numbered sources provided. "
    "Cite every claim with bracketed markers like [1] or [2] referring to those "
    "source numbers. If the sources do not contain the answer, say you do not "
    "have enough local context; do not use outside knowledge. Be concise."
)

_CITATION_RE = re.compile(r"\[(\d+)\]")


def format_sources(hits: List[dict]) -> str:
    """Render retrieved chunks as a numbered source block for the prompt."""
    blocks = []
    for i, h in enumerate(hits, start=1):
        title = h.get("title") or h.get("path") or f"source {i}"
        path = h.get("path") or ""
        text = (h.get("text") or "").strip()
        blocks.append(f"[{i}] {title} ({path})\n{text}")
    return "\n\n".join(blocks)


def build_messages(query: str, hits: List[dict]) -> List[Dict[str, str]]:
    user = (
        f"Question: {query}\n\n"
        f"Sources:\n{format_sources(hits)}\n\n"
        "Answer using only the sources above, citing them by number."
    )
    return [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": user},
    ]


def used_citations(answer: str, hits: List[dict]) -> List[Dict[str, Any]]:
    """Map [n] markers actually present in the answer back to their sources."""
    nums = sorted({int(m) for m in _CITATION_RE.findall(answer or "")
                   if 1 <= int(m) <= len(hits)})
    out = []
    for n in nums:
        h = hits[n - 1]
        out.append({"n": n, "title": h.get("title"), "source": h.get("path"),
                    "chunk": h.get("ord"), "score": round(float(h.get("score", 0.0)), 4)})
    return out


def rag_answer(query: str, *, store: Store = None, k: int = None,
               min_score: float = None,
               embed_fn: Callable[[str], List[float]] = None,
               chat_fn: Callable[..., str] = None) -> Dict[str, Any]:
    """Retrieve local context for `query` and compose a grounded, cited answer."""
    q = (query or "").strip()
    if not q:
        return {"query": q, "answer": "", "grounded": False,
                "citations": [], "contexts": [], "note": "empty query"}

    if embed_fn is None:
        from .embeddings import embed_one as embed_fn
    if chat_fn is None:
        from .generate import ollama_chat as chat_fn

    st = store if store is not None else Store(config.DB_PATH)
    k = k or config.RAG_TOP_K
    floor = config.RAG_MIN_SCORE if min_score is None else min_score

    qvec = embed_fn(q)
    hits = [h for h in st.search(qvec, k=k) if float(h.get("score", 0.0)) >= floor]

    contexts = [
        {"n": i + 1, "title": h.get("title"), "source": h.get("path"),
         "chunk": h.get("ord"), "score": round(float(h.get("score", 0.0)), 4),
         "text": h.get("text")}
        for i, h in enumerate(hits)
    ]

    if not hits:
        return {"query": q,
                "answer": "I don't have enough local context to answer that.",
                "grounded": False, "citations": [], "contexts": [],
                "model": config.CHAT_MODEL}

    answer = (chat_fn(build_messages(q, hits)) or "").strip()
    return {"query": q, "answer": answer, "grounded": True,
            "citations": used_citations(answer, hits), "contexts": contexts,
            "model": config.CHAT_MODEL}
