# Town Sovereign MCP — Phase 1 (sovereign search)

A local-model companion for **Town**. It exposes a single MCP tool, `sovereign_search`,
so your Town assistant and routines can run **private semantic search** over your own
corpus. Query embeddings are computed by a **local Ollama** model on the Aetheris box
and matched against a **local vector store** — nothing in the search path touches a
cloud model.

This is Phase 1 of the phased Ollama × Town plan. Town stays the frontier-model brain;
Ollama takes the work that is private, bulk, embedding-heavy, or cost-sensitive.

## Architecture

```
Town (assistant + routines)
        │  MCP Streamable HTTP over HTTPS
        │  headers: Authorization: Bearer <token> + CF-Access-Client-Id/Secret
        ▼
Cloudflare edge ── Access (service token) ── tunnel ──► oracle-aetheris
                                                          ├── ollama.service      (127.0.0.1:11434)
                                                          ├── town-mcp.service     (127.0.0.1:8787)  ← this repo
                                                          └── SQLite index (chunks + float32 embeddings)
```

- The MCP server binds to `127.0.0.1` and is published **only** through the existing
  cloudflared tunnel. It never listens on a public port directly.
- It calls Ollama **locally** at `127.0.0.1:11434`, not through the Access-gated public
  hostname.
- **No Docker.** Python venv + `systemd`. Vector search is numpy brute-force over a
  SQLite table (the corpus is small; this is instant). `sqlite-vec` is the scale-up path.

## Layout

```
town-mcp/
  bootstrap.sh              # no-Docker installer: venv, pip, .env+token, model pull, systemd
  requirements.txt
  .env.example
  systemd/town-mcp.service  # reference unit (bootstrap.sh writes the live one)
  src/
    config.py               # env config
    embeddings.py           # local Ollama /api/embeddings client
    chunker.py              # paragraph-aware chunking
    store.py                # SQLite store + cosine search
    indexer.py              # walk corpus -> chunk -> embed -> upsert
    server.py               # FastMCP server; tool: sovereign_search; bearer auth; /health
  scripts/smoke_test.py     # offline pipeline test (stub embedder, no network)
  corpus/sample/            # public-safe synthetic docs so the pipeline is testable
```

## Deploy on Aetheris (no Docker)

Run **on the box**, as the user that should own the service:

```bash
git clone https://github.com/nrupala/aetheris && cd aetheris/town-mcp
./bootstrap.sh                 # venv + deps + .env (generates bearer) + pulls nomic-embed-text + systemd
curl -s http://127.0.0.1:8787/health   # -> ok
```

Then expose + gate it:

1. **Tunnel route** — add an ingress rule to your cloudflared config mapping a hostname
   (proposed `mcp.devinfo.dev`) to `http://127.0.0.1:8787`, then restart the tunnel.
2. **Cloudflare Access** — create an Access application for that hostname with a
   **service-token** policy. Note the `CF-Access-Client-Id` and `CF-Access-Client-Secret`.
3. **Register in Town** — Settings → MCP → Add Server:
   - **URL:** `https://mcp.devinfo.dev/mcp`
   - **Headers:**
     - `Authorization: Bearer <MCP_BEARER_TOKEN from .env>`
     - `CF-Access-Client-Id: <id>`
     - `CF-Access-Client-Secret: <secret>`
4. Enable the server on a throwaway test routine and call `sovereign_search`.

## Index a corpus

```bash
./.venv/bin/python -m src.indexer --corpus /path/to/corpus --clear
```

Indexes `.md` / `.markdown` / `.txt` files. Re-runnable (idempotent per file path).

### Corpus source (the one open input)

The Phase-1 corpus is `research/` + `products/`, which live in Town's Content Library.
The on-box indexer needs a local source. Pick one:
- **(a)** a one-way export/sync of those collections to a folder on the box, or
- **(b)** point the indexer at the box's existing local git checkouts for `products/` and
  an exported copy of the `research/` dossiers.

Private research is **never** committed to this repo. `corpus/sample/` holds only
public-safe synthetic docs for testing.

## Acceptance test — Phase 1 is "done" when

- `GET /health` returns `ok`.
- A request without a valid bearer / Access session returns `401`.
- `sovereign_search` returns relevant, correctly-cited chunks for 5 sample queries.
- Town lists the server (Settings → MCP) and a test routine calls the tool successfully.
- Nothing indexed leaves the box; no cloud model is touched in the search path.

## Security

- Bearer token on every request (checked in-process) **and** Cloudflare Access service
  token at the edge — two layers.
- Binds to localhost; reachable only via the tunnel.
- Secrets live only in `.env` (gitignored). Rotate the bearer periodically.

## Scale-up path (later phases)

- Swap numpy brute-force for `sqlite-vec` when the corpus grows large.
- Add PDF text extraction to index `research/` PDFs.
- Add a local chat model for `sovereign_summarize` / `sovereign_draft` (Phase 2).

## Caveat (honest)

The MCP Python SDK surface (`FastMCP.streamable_http_app()`, `custom_route`) is
version-sensitive; `mcp>=1.9` is pinned. The data layer (chunking, store, search) is
locally tested (`scripts/smoke_test.py`). The MCP-server + Ollama wiring is validated
on the box during deploy — that is the intended DEPLOY/TEST step, not a claim it has
already run against live Ollama.
