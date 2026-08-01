import os

OLLAMA_URL = os.environ.get("OLLAMA_URL", "http://127.0.0.1:11434")
EMBED_MODEL = os.environ.get("EMBED_MODEL", "nomic-embed-text")
CHAT_MODEL = os.environ.get("CHAT_MODEL", "llama3.2")
CORPUS_DIR = os.environ.get("CORPUS_DIR", "./corpus")
DB_PATH = os.environ.get("DB_PATH", "./data/index.db")
MCP_BEARER_TOKEN = os.environ.get("MCP_BEARER_TOKEN", "")
HOST = os.environ.get("HOST", "127.0.0.1")
PORT = int(os.environ.get("PORT", "8787"))
MCP_PATH = os.environ.get("MCP_PATH", "/mcp")
CHUNK_CHARS = int(os.environ.get("CHUNK_CHARS", "1200"))
CHUNK_OVERLAP = int(os.environ.get("CHUNK_OVERLAP", "150"))

# RAG (rag_answer) knobs.
RAG_TOP_K = int(os.environ.get("RAG_TOP_K", "5"))
RAG_MIN_SCORE = float(os.environ.get("RAG_MIN_SCORE", "0.2"))
