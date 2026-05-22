LLMVM REQUIREMENTS.DOC

Domain: aimlds.tech
- AI Inference endpoint: ai.aimlds.tech
- RAG endpoint: rag.aimlds.tech (later)
- Dashboard: console.aimlds.tech (later)
- Clean, professional, tech-focused. Fits the project perfectly.
API Keys You Need to Generate (2FA workaround)
I can't bypass 2FA, but I can work with API keys/tokens. You generate these once, paste them, and I handle the rest.
Oracle Cloud API Key
1. Login to OCI Console → Identity → Users → your user
2. Scroll to Resources → API Keys → Add API Key
3. Choose Generate API Key Pair → download the .pem private key
4. Copy these 4 values:
   - Tenancy OCID (from Tenancy details page)
   - User OCID (from your user page)
   - Fingerprint (shown after key upload)
   - Private Key (the .pem file content)
Cloudflare API Token
1. Go to https://dash.cloudflare.com/profile/api-tokens
2. Create Token → use Edit Cloudflare Workers template
3. Add permissions:
   - Account > Cloudflare Tunnel > Edit
   - Zone > DNS > Edit
   - Zone > Zone > Read
4. Copy the token (starts with vU...)
Phase 1 Execution Plan (Starting Now)
While you generate those keys, I'll build the infrastructure skeleton. Here's what I'm doing in parallel:
1. LLMVM project structure — already created
2. Oracle Terraform — already written, needs your credentials
3. Cloudflare Tunnel config — already written, needs your token
4. RAG Framework (no fluff) — starting now
RAG Framework — "Keeping It Real"
No LangChain, no Haystack, no 200 dependencies. Just:
rag_core/
├── chunker.py          # Split docs into 512-token chunks (pure Python)
├── embedder.py         # Call LMStudio /v1/embeddings (requests only)
├── vector_store.py     # SQLite + numpy cosine similarity (no Milvus needed initially)
├── retriever.py        # Top-K semantic search + rerank
├── generator.py        # Call LMStudio /v1/chat/completions with context
└── pipeline.py         # Orchestrate: ingest → embed → store → retrieve → generate
Dependencies: requests, numpy, sqlite3 (stdlib), tiktoken (token counting). That's it.
Let me know when you have the OCI + Cloudflare credentials, and I'll:
1. Fill in Terraform variables
2. Run terraform apply to provision the VM
3. Deploy LMStudio + Cloudflare Tunnel
4. Test ai.aimlds.tech → phi-4-reasoning-plus response
5. Build the RAG pipeline alongside it