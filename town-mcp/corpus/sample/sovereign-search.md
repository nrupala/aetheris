# Sovereign Search

sovereign_search embeds the query with a local model, compares it against stored
document embeddings using cosine similarity, and returns the top matching chunks
with their source. Indexing is idempotent and can be re-run at any time. The whole
retrieval path is local and private.
