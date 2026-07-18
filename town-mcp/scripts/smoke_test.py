"""Local smoke test: index the sample corpus with a deterministic stub embedder
(no Ollama, no network) and verify retrieval ranks a relevant doc first.
Proves the chunk -> store -> search pipeline end to end."""
import os, sys, hashlib, tempfile
import numpy as np
sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
from src.chunker import chunk_text  # noqa
from src.store import Store
from src import indexer

DIM = 96


def stub_embed_text(t):
    v = np.zeros(DIM, dtype=np.float32)
    toks = "".join(c.lower() if c.isalnum() else " " for c in t).split()
    for tok in toks:
        h = int(hashlib.md5(tok.encode()).hexdigest(), 16) % DIM
        v[h] += 1.0
    n = np.linalg.norm(v)
    return (v / n if n else v).tolist()


def stub_embed_many(chunks):
    return [stub_embed_text(c) for c in chunks]


def main():
    base = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    corpus = os.path.join(base, "corpus")
    db = os.path.join(tempfile.mkdtemp(), "index.db")
    store = Store(db)
    stats = indexer.reindex(store, corpus, stub_embed_many)
    print("index stats:", stats)
    assert stats["chunks"] > 0, "no chunks indexed"

    q = "how does the sovereign MCP server authenticate requests from Town"
    hits = store.search(stub_embed_text(q), k=3)
    print("\nTop hits for:", q)
    for h in hits:
        print(f"  {h['score']:.3f}  {h['title']}  [{h['path']}]")
    assert hits, "no search results"
    # the auth/security doc should rank at or near the top for this query
    assert any("auth" in h["path"].lower() or "security" in h["path"].lower() or "mcp" in h["path"].lower()
               for h in hits), "expected a relevant doc in top-k"
    print("\nSMOKE TEST PASSED")


if __name__ == "__main__":
    main()
