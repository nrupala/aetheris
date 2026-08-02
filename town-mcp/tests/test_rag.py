"""Offline unit tests for the rag_answer orchestration.

No network / no Ollama: embed_fn and chat_fn are injected. Retrieval runs against
a real (temp) SQLite store with deterministic bag-of-words vectors so the correct
source ranks first, letting us assert the retrieve -> prompt -> cite wiring.
"""
import os
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)  # town-mcp/
sys.path.insert(0, ROOT)

from src.store import Store  # noqa: E402
from src import rag  # noqa: E402

VOCAB = ["sovereign", "local", "bearer", "access", "mcp", "http", "citation"]


def _embed(text):
    t = text.lower()
    return [float(t.count(w)) for w in VOCAB]


DOCS = {
    "sovereign-search.md": ("Sovereign Search",
                            "Sovereign search keeps all data local on the box. "
                            "Nothing local leaves the machine."),
    "security-and-auth.md": ("Security and Auth",
                             "A bearer token guards the MCP endpoint behind "
                             "Cloudflare Access."),
    "mcp-architecture.md": ("MCP Architecture",
                            "The MCP server exposes tools over streamable http."),
}


def _fresh_store():
    fd, path = tempfile.mkstemp(suffix=".db")
    os.close(fd)
    st = Store(path)
    for p, (title, text) in DOCS.items():
        st.upsert_document(p, title, [text], [_embed(text)])
    return st


def test_retrieves_and_cites_top_source():
    st = _fresh_store()
    seen = {}

    def chat_fn(messages, **kw):
        seen["messages"] = messages
        return "Sovereign search keeps all data local on the box [1]."

    res = rag.rag_answer("How does sovereign search keep data local?",
                         store=st, k=3, embed_fn=_embed, chat_fn=chat_fn)

    assert res["grounded"] is True
    assert res["contexts"][0]["source"] == "sovereign-search.md"
    user_msg = seen["messages"][-1]["content"]
    assert "Sovereign search keeps all data local" in user_msg
    assert res["citations"] and res["citations"][0]["source"] == "sovereign-search.md"


def test_no_context_does_not_call_model():
    st = _fresh_store()
    called = {"n": 0}

    def chat_fn(messages, **kw):
        called["n"] += 1
        return "should not be called"

    res = rag.rag_answer("anything at all", store=st, k=3,
                         min_score=1.1, embed_fn=_embed, chat_fn=chat_fn)

    assert res["grounded"] is False
    assert called["n"] == 0
    assert "local context" in res["answer"].lower()


def test_empty_query_short_circuits():
    st = _fresh_store()

    def chat_fn(messages, **kw):
        raise AssertionError("model must not be called for empty query")

    res = rag.rag_answer("   ", store=st, embed_fn=_embed, chat_fn=chat_fn)
    assert res["grounded"] is False
    assert res["citations"] == []


def test_used_citations_ignores_out_of_range():
    hits = [{"title": "A", "path": "a.md", "ord": 0, "score": 0.9}]
    cites = rag.used_citations("Claim [1] and bogus [5].", hits)
    assert [c["n"] for c in cites] == [1]
