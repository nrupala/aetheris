"""Paragraph-aware text chunking with character-based sizing and overlap."""
from __future__ import annotations
import re
from typing import List


def _normalize(text: str) -> str:
    text = text.replace("\r\n", "\n").replace("\r", "\n")
    return re.sub(r"\n{3,}", "\n\n", text).strip()


def chunk_text(text: str, chunk_chars: int = 1200, overlap: int = 150) -> List[str]:
    """Split text into ~chunk_chars pieces, breaking on paragraph boundaries
    where possible, with a soft character overlap between consecutive chunks."""
    text = _normalize(text)
    if not text:
        return []
    paras = [p.strip() for p in text.split("\n\n") if p.strip()]
    units: List[str] = []
    for p in paras:
        if len(p) <= chunk_chars:
            units.append(p)
        else:
            step = max(1, chunk_chars - overlap)
            for i in range(0, len(p), step):
                units.append(p[i:i + chunk_chars])
    chunks: List[str] = []
    buf = ""
    for u in units:
        if not buf:
            buf = u
        elif len(buf) + 2 + len(u) <= chunk_chars:
            buf = f"{buf}\n\n{u}"
        else:
            chunks.append(buf)
            tail = buf[-overlap:] if overlap > 0 else ""
            buf = f"{tail}\n\n{u}" if tail else u
    if buf.strip():
        chunks.append(buf)
    return chunks
