# Town Sovereign MCP — Phase 1 (sovereign search + cited RAG)

A local-model companion for **Town**. It exposes two MCP tools — `sovereign_search`
(private semantic search) and `rag_answer` (retrieval + a **locally generated, cited**
answer) — so your Town assistant and routines can work over your own corpus. Query
embeddings and answer generation both run on **local Ollama** models on the Aetheris
box, matched against a **local vector store** — nothing in the search or answer path
touches a cloud model.

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

## Tools

### `sovereign_search(query, k=5)`
Semantic retrieval only. Returns the top-`k` local chunks with score, title, source
path, and text. Use when Town's frontier model should do the reasoning and just needs
the raw local context.

### `rag_answer(query, k=5)`
Retrieval + **local** generation. Retrieves the top-`k` chunks, then a local Ollama chat
model (`CHAT_MODEL`, default `llama3.2`) composes a grounded answer whose claims cite
their sources by number (`[1]`, `[2]`, ...). Returns
`{query, answer, grounded, citations, contexts, model}`.

Trust guarantees:
- **Fully local** — retrieval and generation both stay on the box; nothing leaves.
- **No hallucination past the corpus** — if nothing scores above `RAG_MIN_SCORE`
  (default `0.2`), it returns `grounded: false` with an "insufficient local context"
  answer and **does not call the model**.
- **Auditable** — `contexts` is the exact evidence set; `citations` maps each `[n]`
  marker in the answer back to its source.

Requires a local chat model: `ollama pull llama3.2` (or set `CHAT_MODEL`). `bootstrap.sh`
pulls it for you.

## Layout

```
town-mcp/
  bootstrap.sh              # no-Docker installer: venv, pip, .env+token, model pulls, systemd
  requirements.txt
  requirements-dev.txt      # pytest (test-only)
  .env.example
  systemd/town-mcp.service  # reference unit (bootstrap.sh writes the live one)
  ci/python-tests.yml       # PROPOSED workflow — move to .github/workflows/ to activate
  src/
    config.py               # env config
    embeddings.py           # local Ollama /api/embeddings client
    generate.py             # local Ollama /api/chat client (grounded generation)
    chunker.py              # paragraph-aware chunking
    store.py                # SQLite store + cosine search
    indexer.py              # walk corpus -> chunk -> embed -> upsert
    rag.py                  # rag_answer: retrieve -> grounded, cited answer
    server.py               # FastMCP server; tools: sovereign_search, rag_answer; bearer auth; /health
  scripts/smoke_test.py     # offline pipeline test (stub embedder, no network)
  tests/                    # offline pytest suite (rag + generate; injected stubs)
```

## Deploy on Aetheris (no Docker)

Run **on the box**, as the user that should own the service:

```bash
git clone https://github.com/nrupala/aetheris && cd aetheris/town-mcp
./bootstrap.sh                 # venv + deps + .env (generates bearer) + pulls embed + chat models + systemd
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
4. Enable the server on a throwaway test routine and call `sovereign_search` / `rag_answer`.

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

## Tests

Offline unit tests (no network / no Ollama — embed and chat are injected stubs):

```bash
pip install -r requirements-dev.txt numpy httpx
python -m pytest tests -q
```

CI note: this repo's GitHub Actions currently build/test only the Rust `core/`. The
proposed `ci/python-tests.yml` adds a `town-mcp` pytest gate — move it to
`.github/workflows/` (requires the `workflow` scope) to activate it.

## Acceptance test — Phase 1 is "done" when

- `GET /health` returns `ok`.
- A request without a valid bearer / Access session returns `401`.
- `sovereign_search` returns relevant chunks for 5 sample queries.
- `rag_answer` returns a grounded, correctly-cited answer for those queries, and returns
  `grounded: false` (without calling the model) for an off-corpus question.
- Town lists the server (Settings → MCP) and a test routine calls the tools successfully.
- Nothing indexed leaves the box; no cloud model is touched in the search/answer path.

## Security

- Bearer token on every request (checked in-process) **and** Cloudflare Access service
  token at the edge — two layers.
- Binds to localhost; reachable only via the tunnel.
- Secrets live only in `.env` (gitignored). Rotate the bearer periodically.

## Scale-up path (later phases)

- Swap numpy brute-force for `sqlite-vec` when the corpus grows large.
- Add PDF text extraction to index `research/` PDFs.
- Add `sovereign_summarize` / `sovereign_draft` on top of `generate.py` (Phase 2).

## Caveat (honest)

The MCP Python SDK surface (`FastMCP.streamable_http_app()`, `custom_route`) is
version-sensitive; `mcp>=1.9` is pinned. The data + RAG layers (chunking, store, search,
retrieve→prompt→cite) are locally unit-tested (`tests/`, `scripts/smoke_test.py`). The
MCP-server + live-Ollama wiring (embeddings and chat) is validated on the box during
deploy — that is the intended DEPLOY/TEST step, not a claim it has already run against
live Ollama.
