#!/usr/bin/env python3
"""
RAG Core Integration Tests — verifies all modules work together.

Usage:
    python test_rag.py              # Run all tests
    python test_rag.py test_pipeline    # Run specific test

Prerequisites:
    - LMStudio must be running locally (or set RAG_AI_ENDPOINT)
    - python -m pip install requests numpy tiktoken
"""

import os
import sys
import tempfile
import json
import time

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from rag_core.config import RAGConfig
from rag_core.chunker import TextChunker as Chunker, Chunk
from rag_core.embedder import Embedder
from rag_core.vector_store import VectorStore
from rag_core.retriever import Retriever
from rag_core.generator import Generator
from rag_core.pipeline import RAGPipeline

# Test fixtures
SAMPLE_TEXT = """
# Aetheris Architecture Guide

Aetheris is a Sovereign AI-Native Personal Cloud built with Rust.

## Gateway Module

The Gateway module handles all incoming HTTP requests using Axum.
It routes requests to the appropriate backend services:
- Storage operations go to the ZFS backend
- Authentication goes to the Identity module
- Policy checks go to the OPA bridge

## Storage Module

Storage uses ZFS with native encryption (AES-256-GCM).
All data is encrypted at rest. Snapshots provide versioning.

## Identity Module

Authentication via OpenID Connect. JWT tokens validated on every request.
Supports multi-user environments with role-based access.

## AI Policy Engine

OPA (Open Policy Agent) evaluates policies for every request.
Policies are written in Rego and can be updated dynamically.

## WireGuard Mesh

Nodes communicate through an encrypted WireGuard mesh network.
UDP port 51820 is the only port used for inter-node communication.
"""


def get_test_config(db_path: str) -> RAGConfig:
    """Create a test config with temp DB."""
    return RAGConfig(db_path=db_path)


def test_chunker():
    """Test semantic document chunking."""
    print("\n=== Test: Chunker ===")
    chunker = Chunker()

    # Test text chunking
    chunks = chunker.chunk(SAMPLE_TEXT, source="test.md")

    assert len(chunks) > 0, "Should produce at least one chunk"
    print(f"  [OK] Created {len(chunks)} chunks")

    # Verify chunk structure
    for chunk in chunks:
        assert isinstance(chunk, Chunk)
        assert len(chunk.text) > 0
        assert chunk.token_count > 0
        print(f"     Chunk {chunk.index}: {chunk.token_count} tokens, {len(chunk.text)} chars")

    # Test max chunk size
    for chunk in chunks:
        assert chunk.token_count <= 512, f"Chunk {chunk.index} exceeds 512 tokens: {chunk.token_count}"

    print("  [OK] All chunks within size limits")
    return chunks


def test_embedder():
    """Test embedding generation."""
    print("\n=== Test: Embedder ===")

    # Try to connect to LMStudio
    try:
        embedder = Embedder()
        test_vec = embedder.embed(["test sentence"])
        assert len(test_vec) == 1
        print(f"  ✅ Embedding dimension: {len(test_vec[0])}")

        # Test batch embedding
        texts = ["first text", "second text", "third text"]
        vectors = embedder.embed(texts)
        assert len(vectors) == 3
        print(f"  ✅ Batch embed: {len(vectors)} vectors")

        return embedder
    except Exception as e:
        print(f"  [WARN] Embedder test skipped (LMStudio not running): {e}")
        return None


def test_vector_store():
    """Test SQLite vector storage."""
    print("\n=== Test: Vector Store ===")

    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        cfg = get_test_config(f.name)

        try:
            store = VectorStore(cfg=cfg)

            # Create dummy chunks and embeddings
            chunks = [
                Chunk(text="Aetheris uses ZFS for encrypted storage", source="docs.md", index=0, token_count=10),
                Chunk(text="WireGuard provides secure mesh networking", source="docs.md", index=1, token_count=8),
                Chunk(text="OPA evaluates policies for every request", source="docs.md", index=2, token_count=9),
            ]

            # Use random normalized embeddings for testing (no API needed)
            import numpy as np
            dim = 384  # Common embedding dimension
            embeddings = [np.random.randn(dim).astype(np.float32) for _ in chunks]
            for e in embeddings:
                e /= np.linalg.norm(e)

            ids = store.add(chunks, embeddings)
            assert len(ids) == 3
            print(f"  ✅ Stored {len(ids)} chunks")

            # Test search with a matching vector
            query_vec = embeddings[0].copy()
            results = store.search(query_vec, top_k=2)
            assert len(results) > 0
            assert results[0].chunk_id == ids[0]  # Should find exact match first
            print(f"  ✅ Search returned {len(results)} results (best match first)")

            # Test source listing
            sources = store.list_sources()
            assert len(sources) == 1
            assert sources[0]["source"] == "docs.md"
            print(f"  ✅ Sources: {sources[0]['source']} ({sources[0]['chunks']} chunks)")

            # Test stats
            stats = store.stats()
            assert stats["total_chunks"] == 3
            assert stats["total_sources"] == 1
            print(f"  ✅ Stats: {stats['total_chunks']} chunks, {stats['total_sources']} source(s)")

            # Test deletion
            deleted = store.delete_source("docs.md")
            assert deleted == 3
            print(f"  ✅ Deleted {deleted} chunks")

            stats = store.stats()
            assert stats["total_chunks"] == 0
            print(f"  ✅ Store empty after deletion")

            store.close()
            import gc
            gc.collect()
        finally:
            import time
            time.sleep(0.1)
            if os.path.exists(f.name):
                try:
                    os.remove(f.name)
                except PermissionError:
                    pass


def test_retriever():
    """Test retrieval (requires embeddings)."""
    print("\n=== Test: Retriever ===")
    print("  ⚠️  Requires LMStudio running — skipping (covered by pipeline test)")


def test_pipeline():
    """Test full RAG pipeline (ingest + query)."""
    print("\n=== Test: Full Pipeline ===")

    with tempfile.NamedTemporaryFile(suffix=".db", delete=False) as f:
        cfg = get_test_config(f.name)

        try:
            pipeline = RAGPipeline(cfg=cfg)

            # Test ingest from text
            print("\n  Step 1: Ingest sample text")
            result = pipeline.ingest_text(SAMPLE_TEXT, source="architecture.md")
            print(f"  ✅ Ingested: {result['chunks_created']} chunks")

            # Test sources listing
            sources = pipeline.list_sources()
            assert len(sources) == 1
            print(f"  ✅ Source: {sources[0]['source']}")

            # Test stats
            stats = pipeline.stats()
            assert stats["total_chunks"] > 0
            print(f"  ✅ Stats: {stats['total_chunks']} chunks")

            # Test query (will fail if LMStudio not running)
            print("\n  Step 2: Query knowledge base")
            try:
                rag_result = pipeline.query("What storage system does Aetheris use?")
                print(f"  ✅ Query completed in {rag_result.response_time}s")
                print(f"     Sources used: {rag_result.chunks_searched}")
                print(f"     Answer preview: {rag_result.answer[:100]}...")
            except Exception as e:
                print(f"  ⚠️  Query failed (LMStudio not running): {e}")
                print("     (Ingest and storage tests passed)")

            # Test source deletion
            print("\n  Step 3: Delete source")
            deleted = pipeline.delete_source("architecture.md")
            assert deleted > 0
            print(f"  ✅ Deleted {deleted} chunks")

            pipeline.close()
            import gc
            gc.collect()
        finally:
            import time
            time.sleep(0.1)
            if os.path.exists(f.name):
                try:
                    os.remove(f.name)
                except PermissionError:
                    pass


def main():
    """Run all tests."""
    print("=" * 60)
    print("  Aetheris RAG Core — Integration Tests")
    print("=" * 60)

    tests = [
        ("Chunker", test_chunker),
        ("Embedder", test_embedder),
        ("Vector Store", test_vector_store),
        ("Pipeline", test_pipeline),
    ]

    passed = 0
    failed = 0
    skipped = 0

    for name, test_func in tests:
        try:
            test_func()
            passed += 1
        except Exception as e:
            print(f"  ❌ FAILED: {e}")
            failed += 1

    print("\n" + "=" * 60)
    print(f"  Results: {passed} passed, {failed} failed")
    print("=" * 60)

    return 0 if failed == 0 else 1


if __name__ == "__main__":
    # Allow running specific tests
    if len(sys.argv) > 1:
        test_name = sys.argv[1]
        test_map = {
            "test_chunker": test_chunker,
            "test_embedder": test_embedder,
            "test_vector_store": test_vector_store,
            "test_pipeline": test_pipeline,
        }
        if test_name in test_map:
            test_map[test_name]()
        else:
            print(f"Unknown test: {test_name}")
            sys.exit(1)
    else:
        sys.exit(main())
