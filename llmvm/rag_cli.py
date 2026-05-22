"""
RAG CLI — Command-line interface for Aetheris RAG operations.

Usage:
    python rag_cli.py ingest docs/              # Index a directory
    python rag_cli.py ingest docs/guide.md      # Index a file
    python rag_cli.py query "How to configure?" # Ask a question
    python rag_cli.py query "What is X?" --no-rag  # Pure LLM
    python rag_cli.py stats                     # Show index stats
    python rag_cli.py sources                   # List indexed sources
    python rag_cli.py delete source_name        # Remove a source
    python rag_cli.py reset                     # Clear all data
"""

import argparse
import sys
import os
import time
import json
import uuid
import threading
import signal
import atexit
from datetime import datetime
from typing import Dict, Optional

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from rag_core.pipeline import RAGPipeline
from rag_core.config import config

# Coordinator imports (optional, only needed for --server mode)
try:
    from rag_core.coordinator import (
        get_coordinator, ProcessingCoordinator,
        CoordinatorConfig, CircuitOpenError, ResourceError,
        QueueFullError, TransientError, PermanentError, MaxRetriesExceededError,
    )
    HAS_COORDINATOR = True
except ImportError:
    HAS_COORDINATOR = False

# Knowledge Graph imports
try:
    from rag_core.knowledge_graph import KnowledgeGraph
    HAS_KG = True
except ImportError:
    HAS_KG = False

# HTTP server imports (optional, only needed for --server mode)
try:
    from fastapi import FastAPI, UploadFile, File, HTTPException, BackgroundTasks, Query
    from fastapi.responses import JSONResponse
    from pydantic import BaseModel
    import uvicorn
    HAS_SERVER = True
except ImportError:
    HAS_SERVER = False

# Global instances for server mode
_pipeline = None
_coordinator = None
_kg = None
_session_id = None

def get_pipeline():
    global _pipeline
    if _pipeline is None:
        _pipeline = RAGPipeline()
    return _pipeline

def get_coordinator_instance():
    global _coordinator
    if _coordinator is None and HAS_COORDINATOR:
        coord_config = CoordinatorConfig(
            workspace_root=os.environ.get("WORKSPACE_ROOT", "/workspace"),
            metrics_endpoint=os.environ.get("VMETRICS_URL", "http://localhost:8428"),
            metrics_enabled=os.environ.get("METRICS_ENABLED", "true").lower() == "true",
            audit_log_path=os.environ.get("AUDIT_LOG_PATH", "/workspace/persisted/audit"),
        )
        _coordinator = get_coordinator(coord_config)
        _coordinator.start()

        # Attach KG if available
        if HAS_KG:
            global _kg
            _kg = KnowledgeGraph(
                db_path=os.environ.get("RAG_GRAPH_DB_PATH", "/app/rag_data/knowledge_graph.db"),
                cfg=config,
            )
            _coordinator.attach_knowledge_graph(_kg)

        # Start session
        global _session_id
        _session_id = _coordinator.start_session()

    return _coordinator


# --- Background Job Tracking ---
_jobs: Dict[str, dict] = {}
_jobs_lock = threading.Lock()


def _update_job(job_id: str, status: str, **kwargs):
    with _jobs_lock:
        if job_id in _jobs:
            _jobs[job_id].update({"status": status, "updated_at": datetime.utcnow().isoformat(), **kwargs})


def _ingest_background(job_id: str, tmp_path: str, source: str, verbose: bool, extract_entities: bool = False):
    """Run ingest in background thread with progress tracking."""
    try:
        def progress(step, count):
            _update_job(job_id, "processing",
                       current_step=step,
                       current_count=count,
                       verbose_log=f"[{step}] {count} items" if verbose else None)

        pipeline = get_pipeline()
        _update_job(job_id, "processing", started_ingest=True)

        result = pipeline.ingest_file(
            tmp_path,
            metadata={"source": source},
            progress_callback=progress,
            extract_entities=extract_entities,
        )

        kg_stats = result.get("kg_stats", {})

        # Move from staging to permanent storage
        storage_dir = os.path.join(config.storage_dir, datetime.utcnow().strftime("%Y/%m"))
        os.makedirs(storage_dir, exist_ok=True)
        safe_name = f"{job_id[:8]}_{os.path.basename(tmp_path)}"
        final_path = os.path.join(storage_dir, safe_name)
        if os.path.exists(tmp_path):
            os.rename(tmp_path, final_path)

        _update_job(job_id, "completed",
                   chunks_created=result["chunks_created"],
                   time_seconds=result["time_seconds"],
                   chunks_per_second=result["chunks_per_second"],
                   stored_path=final_path,
                   entities_extracted=kg_stats.get("entities_added", 0))
    except Exception as e:
        _update_job(job_id, "failed", error=str(e))
        if os.path.exists(tmp_path):
            os.remove(tmp_path)


# --- API Models ---
class QueryRequest(BaseModel):
    query: str
    use_rag: bool = True
    top_k: int = 3
    threshold: float = 0.7
    include_history: bool = True
    reasoning: bool = False
    max_iterations: int = 3
    confidence_threshold: float = 0.7

class IngestResponse(BaseModel):
    status: str
    files_processed: int = 0
    total_chunks: int = 0
    time_seconds: float = 0

class QueryResponse(BaseModel):
    answer: str
    model: str
    chunks_searched: int
    response_time: float
    tokens_used: int
    sources: list = []
    confidence: float = 0.0
    iterations_used: int = 1
    reasoning_trace: list = []
    converged: bool = True
    verification: dict = {}


# --- HTTP Server ---
def start_server(host="0.0.0.0", port=8080):
    if not HAS_SERVER:
        print("Error: fastapi and uvicorn required for server mode")
        print("Install: pip install fastapi uvicorn")
        sys.exit(1)

    # Initialize coordinator on startup
    coord = get_coordinator_instance()
    if coord:
        print(f"Processing Coordinator: started (session: {_session_id})")
    else:
        print("Processing Coordinator: not available (run without coordinator)")

    app = FastAPI(title="Aetheris RAG API", version="2.0.0")

    def _shutdown():
        """Graceful shutdown: end session, stop coordinator."""
        global _session_id
        if coord and _session_id:
            report = coord.end_session(_session_id)
            if report:
                print(f"\nSession Evaluation (score: {report.get('scores', {}).get('answer_quality', {}).get('score', 'N/A')}/10)")
                for s in report.get('suggestions', []):
                    print(f"  [{s['priority']}] {s['suggestion']}")
            _session_id = None
        if coord:
            coord.stop()
            print("Processing Coordinator: stopped")

    @app.on_event("shutdown")
    def on_shutdown():
        _shutdown()

    # --- Core Endpoints ---

    @app.get("/health")
    def health():
        status = {"status": "ok", "service": "aetheris-rag", "version": "2.0.0"}
        if coord:
            status["coordinator"] = "active"
            status["session"] = _session_id
            status["resource_status"] = coord.get_resource_status()
        return status

    @app.post("/query", response_model=QueryResponse)
    def api_query(req: QueryRequest):
        pipeline = get_pipeline()
        start_time = time.time()

        try:
            result = pipeline.query(
                req.query,
                use_rag=req.use_rag,
                top_k=req.top_k,
                threshold=req.threshold,
                include_history=req.include_history,
                reasoning=req.reasoning,
                max_iterations=req.max_iterations,
                confidence_threshold=req.confidence_threshold,
            )

            latency_ms = (time.time() - start_time) * 1000

            if coord:
                coord.record_query_result(
                    query=req.query,
                    success=True,
                    confidence=result.confidence or 0.85,
                    iterations=result.iterations_used,
                    latency_ms=latency_ms,
                    tokens_in=result.tokens_used,
                    tokens_out=result.tokens_used,
                    cache_hit=False,
                )

            return QueryResponse(
                answer=result.answer,
                model=result.model,
                chunks_searched=result.chunks_searched,
                response_time=result.response_time,
                tokens_used=result.tokens_used,
                sources=result.sources or [],
                confidence=result.confidence,
                iterations_used=result.iterations_used,
                reasoning_trace=result.reasoning_trace,
                converged=result.converged,
                verification=result.verification,
            )

        except Exception as e:
            latency_ms = (time.time() - start_time) * 1000
            if coord:
                coord.record_query_result(
                    query=req.query,
                    success=False,
                    latency_ms=latency_ms,
                    error=str(e),
                )
            raise HTTPException(500, str(e))

    @app.post("/ingest/directory")
    def api_ingest_dir(path: str):
        pipeline = get_pipeline()
        if not os.path.exists(path):
            raise HTTPException(404, f"Path not found: {path}")

        # Check resource constraints
        if coord and coord.resource_monitor.should_reject_uploads():
            raise HTTPException(507, "Disk usage > 95%, uploads rejected")

        result = pipeline.ingest_directory(path)
        return {
            "status": "ok",
            "files_processed": result["files_processed"],
            "total_chunks": result["total_chunks"],
            "time_seconds": result["time_seconds"]
        }

    @app.post("/ingest/file")
    def api_ingest_file(
        background_tasks: BackgroundTasks,
        file: UploadFile = File(...),
        source: str = Query(default=None),
        verbose: bool = Query(default=False),
        wait: bool = Query(default=False),
        extract_entities: bool = Query(default=True),
    ):
        # Size limit
        content = file.file.read()
        if len(content) > config.max_upload_size:
            limit_mb = config.max_upload_size / (1024 * 1024)
            raise HTTPException(413, f"File too large. Max: {limit_mb:.0f}MB")

        if not content:
            raise HTTPException(400, "Empty file")

        # Check resource constraints
        if coord and coord.resource_monitor.should_reject_uploads():
            raise HTTPException(507, "Disk usage > 95%, uploads rejected")

        # UUID-isolated staging path
        safe_name = os.path.basename(file.filename) if file.filename else "upload"
        source_name = source or safe_name
        job_id = str(uuid.uuid4())
        staging_dir = os.path.join(config.upload_dir, job_id[:8])
        os.makedirs(staging_dir, exist_ok=True)
        tmp_path = os.path.join(staging_dir, safe_name)

        with open(tmp_path, "wb") as f:
            f.write(content)

        # Create job record
        with _jobs_lock:
            _jobs[job_id] = {
                "job_id": job_id,
                "status": "queued",
                "filename": safe_name,
                "source": source_name,
                "size_bytes": len(content),
                "created_at": datetime.utcnow().isoformat(),
                "updated_at": datetime.utcnow().isoformat(),
            }

        if wait:
            _ingest_background(job_id, tmp_path, source_name, verbose, extract_entities)
            return JSONResponse(content=_jobs[job_id])

        background_tasks.add_task(_ingest_background, job_id, tmp_path, source_name, verbose, extract_entities)

        return {
            "status": "queued",
            "job_id": job_id,
            "filename": safe_name,
            "size_bytes": len(content),
            "poll_url": f"/jobs/{job_id}",
        }

    @app.get("/jobs/{job_id}")
    def api_get_job(job_id: str):
        with _jobs_lock:
            job = _jobs.get(job_id)
        if not job:
            raise HTTPException(404, f"Job not found: {job_id}")
        return job

    @app.get("/jobs")
    def api_list_jobs(limit: int = Query(default=20)):
        with _jobs_lock:
            recent = sorted(_jobs.values(), key=lambda j: j["created_at"], reverse=True)[:limit]
        return recent

    @app.get("/ingest/stats")
    def api_ingest_stats():
        with _jobs_lock:
            total = len(_jobs)
            completed = sum(1 for j in _jobs.values() if j["status"] == "completed")
            failed = sum(1 for j in _jobs.values() if j["status"] == "failed")
            processing = sum(1 for j in _jobs.values() if j["status"] == "processing")
        return {"total_jobs": total, "completed": completed, "failed": failed, "processing": processing}

    @app.get("/stats")
    def api_stats():
        pipeline = get_pipeline()
        return pipeline.stats()

    @app.get("/sources")
    def api_sources():
        pipeline = get_pipeline()
        return pipeline.list_sources()

    @app.delete("/sources/{source_path}")
    def api_delete_source(source_path: str):
        pipeline = get_pipeline()
        count = pipeline.delete_source(source_path)
        return {"deleted": count}

    @app.post("/reset")
    def api_reset():
        pipeline = get_pipeline()
        pipeline.reset()
        return {"status": "cleared"}

    # --- Coordinator & Observability Endpoints ---

    @app.get("/coordinator/dashboard")
    def api_dashboard():
        """Full performance dashboard snapshot."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        return coord.get_performance_dashboard()

    @app.get("/coordinator/events")
    def api_events(
        category: str = Query(default=None),
        severity: str = Query(default=None),
        limit: int = Query(default=50),
    ):
        """Query system event log."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        return coord.get_event_log(category=category, severity=severity, limit=limit)

    @app.get("/coordinator/events/counts")
    def api_event_counts(hours: int = Query(default=24)):
        """Event counts by category and severity."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        return coord.get_event_counts(hours=hours)

    @app.get("/coordinator/sessions")
    def api_sessions(limit: int = Query(default=20)):
        """Session history."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        return coord.get_session_history(limit=limit)

    @app.get("/coordinator/anomalies")
    def api_anomalies(limit: int = Query(default=20)):
        """Recent anomaly detections."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        return coord.get_anomalies(limit=limit)

    @app.get("/coordinator/circuits")
    def api_circuits():
        """Circuit breaker status for all engines."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        return coord.get_circuit_status()

    @app.get("/coordinator/resources")
    def api_resources():
        """Current host resource status."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        return coord.get_resource_status()

    @app.get("/coordinator/transactions")
    def api_transactions(limit: int = Query(default=50)):
        """Recent transactions."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        return [
            {
                "id": tx.id,
                "engine": tx.engine,
                "state": tx.state.value,
                "created_at": tx.created_at,
                "duration_ms": tx.duration_ms,
                "error": tx.error,
            }
            for tx in coord.tx_store.list_recent(limit)
        ]

    @app.get("/coordinator/audit")
    def api_audit(limit: int = Query(default=50)):
        """Recent audit log entries."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        return coord.get_audit_log(limit=limit)

    @app.post("/coordinator/evaluate")
    def api_evaluate_session(session_id: str = Query(default=None)):
        """Run self-evaluation for a session."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        sid = session_id or coord.get_current_session()
        if not sid:
            raise HTTPException(400, "No active session")

        if not coord.self_evaluator:
            raise HTTPException(503, "Knowledge Graph not attached")

        # Gather session stats for evaluation
        sessions = coord.get_session_history(limit=1)
        if not sessions:
            raise HTTPException(404, "Session not found")

        stats = sessions[0]
        report = coord.run_self_evaluation(sid, {
            "queries": coord._session_queries,
            "total_tokens": stats.get("total_tokens_out", 0),
            "kg_entities_before": 0,
            "kg_entities_after": stats.get("kg_entities_added", 0),
            "kg_relations_before": 0,
            "kg_relations_after": stats.get("kg_relations_added", 0),
        })
        return report

    @app.post("/coordinator/cleanup")
    def api_force_cleanup():
        """Trigger immediate workspace cleanup."""
        if not coord:
            raise HTTPException(503, "Coordinator not available")
        coord.force_cleanup()
        return {"status": "cleanup_triggered"}

    @app.get("/knowledge-graph/stats")
    def api_kg_stats():
        """Knowledge Graph statistics."""
        if not _kg:
            raise HTTPException(503, "Knowledge Graph not available")
        return _kg.stats()

    @app.get("/knowledge-graph/entities")
    def api_kg_entities(
        entity_type: str = Query(default=None),
        min_importance: float = Query(default=0.0),
        limit: int = Query(default=100),
    ):
        """List entities."""
        if not _kg:
            raise HTTPException(503, "Knowledge Graph not available")
        return _kg.list_entities(entity_type=entity_type, min_importance=min_importance, limit=limit)

    @app.get("/knowledge-graph/relations")
    def api_kg_relations(entity_name: str = Query(default=None)):
        """List relations."""
        if not _kg:
            raise HTTPException(503, "Knowledge Graph not available")
        return _kg.get_relations(entity_name=entity_name)

    @app.get("/knowledge-graph/profile")
    def api_kg_profile():
        """Get user profile."""
        if not _kg:
            raise HTTPException(503, "Knowledge Graph not available")
        return _kg.get_full_profile()

    @app.post("/knowledge-graph/export")
    def api_kg_export():
        """Export entire knowledge graph."""
        if not _kg:
            raise HTTPException(503, "Knowledge Graph not available")
        return _kg.export_graph()

    print(f"Starting RAG API server v2.0.0 on {host}:{port}")
    if coord:
        print(f"  Coordinator: active")
        print(f"  Session: {_session_id}")
        print(f"  Knowledge Graph: {'attached' if _kg else 'not attached'}")
        print(f"  Performance Monitor: active")
        print(f"  System Event Logger: active")
        print(f"  Self-Evaluator: {'active' if coord.self_evaluator else 'pending KG'}")
    uvicorn.run(app, host=host, port=port)


def cmd_ingest(args):
    """Index files into the knowledge base."""
    pipeline = RAGPipeline()

    path = args.path
    if not os.path.exists(path):
        print(f"Error: Path not found: {path}")
        sys.exit(1)

    def progress(file_path, status):
        print(f"  {status}: {file_path}")

    if os.path.isdir(path):
        print(f"\nIndexing directory: {path}")
        result = pipeline.ingest_directory(path, progress_callback=progress)
        print(f"\nDone: {result['files_processed']} files, {result['total_chunks']} chunks")
        print(f"Time: {result['time_seconds']}s")
    else:
        print(f"\nIndexing file: {path}")
        result = pipeline.ingest_file(path)
        print(f"\nDone: {result['chunks_created']} chunks in {result['time_seconds']}s")
        print(f"Speed: {result['chunks_per_second']} chunks/sec")

    pipeline.close()


def cmd_query(args):
    """Ask a question using RAG or pure LLM."""
    pipeline = RAGPipeline()

    query = args.query
    use_rag = not args.no_rag

    print(f"\nQuery: {query}")
    print(f"Mode: {'RAG' if use_rag else 'LLM'}")
    print("-" * 60)

    # Show typing indicator
    print("Thinking...", end="", flush=True)

    result = pipeline.query(
        query,
        use_rag=use_rag,
        top_k=args.top_k,
        threshold=args.threshold,
        include_history=not args.no_history
    )

    # Clear "Thinking..."
    print("\r" + " " * 20 + "\r")

    print(result.answer)
    print("-" * 60)
    print(f"Model: {result.model}")
    print(f"Sources: {result.chunks_searched} chunks")
    print(f"Time: {result.response_time}s")
    print(f"Tokens: {result.tokens_used}")

    if result.sources:
        print("\nSources used:")
        for src in result.sources:
            print(f"  - {src['source']} (score: {src['score']})")

    pipeline.close()


def cmd_stats(args):
    """Show knowledge base statistics."""
    pipeline = RAGPipeline()
    stats = pipeline.stats()

    print("\nKnowledge Base Statistics:")
    print(f"  Total chunks: {stats['total_chunks']}")
    print(f"  Total sources: {stats['total_sources']}")
    print(f"  Total tokens: {stats['total_tokens']:,}")
    print(f"  Embedding dim: {stats['embedding_dimension']}")
    print(f"  DB size: {stats['db_size_mb']} MB")
    print(f"  DB path: {stats['db_path']}")

    pipeline.close()


def cmd_sources(args):
    """List all indexed sources."""
    pipeline = RAGPipeline()
    sources = pipeline.list_sources()

    if not sources:
        print("\nNo sources indexed yet.")
    else:
        print(f"\nIndexed Sources ({len(sources)}):")
        print("-" * 60)
        for src in sources:
            print(f"  {src['source']}")
            print(f"    Chunks: {src['chunks']}")
            print(f"    Indexed: {src['last_seen']}")

    pipeline.close()


def cmd_delete(args):
    """Remove a source from the knowledge base."""
    pipeline = RAGPipeline()
    count = pipeline.delete_source(args.source)
    print(f"Deleted {count} chunks from source: {args.source}")
    pipeline.close()


def cmd_reset(args):
    """Delete all indexed data."""
    if not args.force:
        confirm = input("This will delete ALL indexed data. Continue? [y/N] ")
        if confirm.lower() != 'y':
            print("Aborted.")
            return

    pipeline = RAGPipeline()
    pipeline.reset()
    print("Knowledge base cleared.")
    pipeline.close()


def main():
    parser = argparse.ArgumentParser(
        description="Aetheris RAG — Command Line Interface",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  python rag_cli.py ingest docs/
  python rag_cli.py query "How do I set up WireGuard?"
  python rag_cli.py query "What's the weather?" --no-rag
  python rag_cli.py stats
  python rag_cli.py sources
  python rag_cli.py delete docs/manual.pdf
  python rag_cli.py reset --force
        """
    )

    subparsers = parser.add_subparsers(dest="command", help="Command to run")

    # Ingest
    ingest_parser = subparsers.add_parser("ingest", help="Index files into knowledge base")
    ingest_parser.add_argument("path", help="File or directory to index")
    ingest_parser.set_defaults(func=cmd_ingest)

    # Query
    query_parser = subparsers.add_parser("query", help="Ask a question")
    query_parser.add_argument("query", help="Your question")
    query_parser.add_argument("--top-k", type=int, default=None, help="Number of context chunks")
    query_parser.add_argument("--threshold", type=float, default=None, help="Min similarity score")
    query_parser.add_argument("--no-rag", action="store_true", help="Use pure LLM (no retrieval)")
    query_parser.add_argument("--no-history", action="store_true", help="Ignore conversation history")
    query_parser.set_defaults(func=cmd_query)

    # Stats
    stats_parser = subparsers.add_parser("stats", help="Show KB statistics")
    stats_parser.set_defaults(func=cmd_stats)

    # Sources
    sources_parser = subparsers.add_parser("sources", help="List indexed sources")
    sources_parser.set_defaults(func=cmd_sources)

    # Delete
    delete_parser = subparsers.add_parser("delete", help="Remove a source")
    delete_parser.add_argument("source", help="Source path to remove")
    delete_parser.set_defaults(func=cmd_delete)

    # Reset
    reset_parser = subparsers.add_parser("reset", help="Clear all data")
    reset_parser.add_argument("--force", action="store_true", help="Skip confirmation")
    reset_parser.set_defaults(func=cmd_reset)

    # Server
    if HAS_SERVER:
        server_parser = subparsers.add_parser("server", help="Start HTTP API server")
        server_parser.add_argument("--host", default="0.0.0.0", help="Bind address")
        server_parser.add_argument("--port", type=int, default=8080, help="Port number")
        server_parser.set_defaults(func=lambda args: start_server(args.host, args.port))

    args = parser.parse_args()

    # Direct server mode: if --server flag is passed directly
    if '--server' in sys.argv:
        host = '0.0.0.0'
        port = 8080
        if '--port' in sys.argv:
            port_idx = sys.argv.index('--port') + 1
            if port_idx < len(sys.argv):
                port = int(sys.argv[port_idx])
        if '--host' in sys.argv:
            host_idx = sys.argv.index('--host') + 1
            if host_idx < len(sys.argv):
                host = sys.argv[host_idx]
        start_server(host, port)
        return

    if not args.command:
        parser.print_help()
        sys.exit(1)

    args.func(args)


if __name__ == "__main__":
    main()
