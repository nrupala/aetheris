"""Town-facing MCP server exposing sovereign tools: sovereign_search + rag_answer.

Transport: MCP Streamable HTTP in **stateless + JSON-response** mode. The edge
MCP Worker (mcp.devinfo.dev / mcp.aimlds.org) relays a single JSON-RPC request
per tool call rather than holding an MCP session, so the server must accept a
bare POST to /mcp and answer with JSON -- not require an initialize/session
handshake and reply over an SSE stream. `stateless_http=True` +
`json_response=True` make /mcp behave that way. Same pattern used by oc-bridge.
Auth: bearer token checked on every request (Authorization: Bearer <token>),
layered behind Cloudflare Access (service token) at the edge. The process binds
to 127.0.0.1 and is published only through the Aetheris cloudflared tunnel.
"""
from __future__ import annotations
from starlette.middleware.base import BaseHTTPMiddleware
from starlette.requests import Request
from starlette.responses import JSONResponse, PlainTextResponse
from mcp.server.fastmcp import FastMCP

from . import config
from .store import Store
from .embeddings import embed_one
from .rag import rag_answer as _rag_answer

store = Store(config.DB_PATH)
mcp = FastMCP("town-sovereign", host=config.HOST, port=config.PORT,
              streamable_http_path=config.MCP_PATH,
              # The edge Worker relays plain JSON-RPC (no MCP session), so serve
              # stateless with JSON responses -- a bare POST to /mcp must work.
              stateless_http=True, json_response=True)


@mcp.tool()
def sovereign_search(query: str, k: int = 5) -> dict:
    """Semantic search over a private local corpus. Returns the top-k most
    relevant chunks with source path + title. Fully local: the query is embedded
    by a local Ollama model and matched against a local vector store. Nothing
    leaves the box."""
    q = (query or "").strip()
    if not q:
        return {"results": [], "note": "empty query"}
    qvec = embed_one(q)
    hits = store.search(qvec, k=k)
    return {
        "query": q,
        "count": len(hits),
        "results": [
            {"score": round(h["score"], 4), "title": h["title"],
             "source": h["path"], "chunk": h["ord"], "text": h["text"]}
            for h in hits
        ],
    }


@mcp.tool()
def rag_answer(query: str, k: int = 5) -> dict:
    """Answer a question from the private local corpus, with citations.

    Retrieves the most relevant local chunks and has a LOCAL Ollama model compose
    a grounded answer whose claims cite their sources by number ([1], [2], ...).
    If nothing relevant is found locally, it says it lacks local context instead
    of guessing. Fully local: retrieval and generation both stay on the box.
    Returns {answer, grounded, citations, contexts, model}."""
    return _rag_answer(query, k=k, store=store)


@mcp.custom_route("/health", methods=["GET"])
async def health(_request: Request):
    return PlainTextResponse("ok")


class BearerAuthMiddleware(BaseHTTPMiddleware):
    async def dispatch(self, request: Request, call_next):
        if request.url.path == "/health":
            return await call_next(request)
        token = config.MCP_BEARER_TOKEN
        if token:
            auth = request.headers.get("authorization", "")
            if not (auth.startswith("Bearer ") and auth[7:].strip() == token):
                return JSONResponse({"error": "unauthorized"}, status_code=401)
        return await call_next(request)


def create_app():
    app = mcp.streamable_http_app()
    app.add_middleware(BearerAuthMiddleware)
    return app


app = create_app()

if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host=config.HOST, port=config.PORT)
