"""Walk CORPUS_DIR, chunk text files, embed via Ollama, upsert into the store."""
from __future__ import annotations
import argparse
import os
from typing import Callable, List
from . import config
from .chunker import chunk_text
from .store import Store

TEXT_EXTS = {".md", ".markdown", ".txt"}


def iter_text_files(root: str):
    for dirpath, _, filenames in os.walk(root):
        for fn in sorted(filenames):
            if os.path.splitext(fn)[1].lower() in TEXT_EXTS:
                yield os.path.join(dirpath, fn)


def _title_for(path: str, text: str) -> str:
    for line in text.splitlines():
        s = line.strip()
        if s.startswith("# "):
            return s[2:].strip()
        if s:
            return s[:80]
    return os.path.basename(path)


def reindex(store: Store, corpus_dir: str, embed_fn: Callable[[List[str]], List[List[float]]]) -> dict:
    files = list(iter_text_files(corpus_dir))
    for path in files:
        with open(path, "r", encoding="utf-8", errors="replace") as fh:
            text = fh.read()
        chunks = chunk_text(text, config.CHUNK_CHARS, config.CHUNK_OVERLAP)
        if not chunks:
            continue
        embs = embed_fn(chunks)
        rel = os.path.relpath(path, corpus_dir)
        store.upsert_document(rel, _title_for(path, text), chunks, embs)
    return {"files": len(files), **store.stats()}


def main():
    ap = argparse.ArgumentParser(description="Index a corpus for sovereign_search")
    ap.add_argument("--corpus", default=config.CORPUS_DIR)
    ap.add_argument("--db", default=config.DB_PATH)
    ap.add_argument("--clear", action="store_true")
    args = ap.parse_args()
    from .embeddings import embed_many
    store = Store(args.db)
    if args.clear:
        store.clear()
    stats = reindex(store, args.corpus, lambda ch: embed_many(ch))
    print(f"Indexed: {stats}")


if __name__ == "__main__":
    main()
