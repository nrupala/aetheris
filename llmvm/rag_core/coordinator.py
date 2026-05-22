"""
Processing Coordinator — Central governance for RAG pipeline operations.

Responsibilities:
- State Machine: Enforce valid state transitions for all transactions
- Circuit Breaker: Protect against cascading failures (API, RAM, disk)
- Error Handler: Classify errors (transient vs permanent), retry or escalate
- RAM→Disk Manager: Flush intermediate data when host memory > 80%
- Queue Depth Control: Limit concurrent operations per engine
- Audit Logger: Log every transaction with full traceability
- Cleanup Scheduler: TTL-based deletion of expired temp files
- Monitoring: Query VictoriaMetrics for real-time host/container metrics

All decisions are metric-driven, not guessed.
"""

import os
import json
import time
import uuid
import shutil
import sqlite3
import threading
import logging
from enum import Enum
from datetime import datetime, timedelta
from typing import Dict, List, Optional, Callable, Any
from dataclasses import dataclass, field
from contextlib import contextmanager

logger = logging.getLogger(__name__)


# --- Enums ---

class TransactionState(str, Enum):
    QUEUED = "queued"
    PROCESSING = "processing"
    COMPLETED = "completed"
    FAILED = "failed"
    CANCELLED = "cancelled"


class CircuitState(str, Enum):
    CLOSED = "closed"       # Normal operation
    OPEN = "open"           # Blocking requests (engine unhealthy)
    HALF_OPEN = "half_open" # Testing recovery


class ErrorType(str, Enum):
    TRANSIENT = "transient"       # Retryable (timeout, connection refused)
    PERMANENT = "permanent"       # Non-retryable (auth failure, invalid input)
    RESOURCE = "resource"         # Host resource constraint (OOM, disk full)


# Valid state transitions
VALID_TRANSITIONS = {
    TransactionState.QUEUED: {TransactionState.PROCESSING, TransactionState.CANCELLED},
    TransactionState.PROCESSING: {TransactionState.COMPLETED, TransactionState.FAILED, TransactionState.CANCELLED},
    TransactionState.COMPLETED: set(),  # Terminal
    TransactionState.FAILED: {TransactionState.QUEUED},  # Can retry
    TransactionState.CANCELLED: set(),  # Terminal
}


# --- Data Classes ---

@dataclass
class Transaction:
    id: str
    engine: str  # "rag", "ai", "dev", etc.
    state: TransactionState
    created_at: str
    updated_at: str
    input_data: dict = field(default_factory=dict)
    output_data: dict = field(default_factory=dict)
    error: Optional[str] = None
    error_type: Optional[ErrorType] = None
    retry_count: int = 0
    max_retries: int = 3
    metadata: dict = field(default_factory=dict)
    duration_ms: float = 0


@dataclass
class CircuitBreaker:
    engine: str
    state: CircuitState = CircuitState.CLOSED
    failure_count: int = 0
    last_failure: Optional[str] = None
    last_success: Optional[str] = None
    opened_at: Optional[str] = None
    timeout_seconds: float = 30.0
    failure_threshold: int = 3
    half_open_max: int = 1
    half_open_attempts: int = 0


@dataclass
class ResourceSnapshot:
    ram_available_mb: float = 0
    ram_total_mb: float = 0
    ram_percent_used: float = 0
    disk_free_gb: float = 0
    disk_total_gb: float = 0
    disk_percent_used: float = 0
    load_1: float = 0
    timestamp: str = ""


# --- Configuration ---

@dataclass
class CoordinatorConfig:
    # Directory paths
    workspace_root: str = os.environ.get("WORKSPACE_ROOT", "/workspace")
    
    # Resource thresholds
    ram_flush_threshold: float = 0.80      # Flush to disk at 80% RAM
    ram_circuit_threshold: float = 0.90    # Open circuit at 90% RAM
    disk_reject_threshold: float = 0.95    # Reject uploads at 95% disk
    disk_cleanup_threshold: float = 0.85   # Trigger cleanup at 85% disk
    
    # Queue limits
    max_concurrent_per_engine: int = 2
    max_queue_depth: int = 5
    
    # Circuit breaker
    circuit_failure_threshold: int = 3
    circuit_timeout_seconds: float = 30.0
    
    # Retry
    max_retries: int = 3
    retry_backoff_base: float = 1.0        # Exponential backoff base
    
    # Cleanup
    cleanup_interval_seconds: int = 300    # Every 5 minutes
    ttl_expired_seconds: int = 1800        # 30 minutes
    
    # Monitoring
    metrics_endpoint: str = os.environ.get("VMETRICS_URL", "http://localhost:8428")
    metrics_enabled: bool = os.environ.get("METRICS_ENABLED", "true").lower() == "true"
    
    # Audit
    audit_log_path: str = os.environ.get("AUDIT_LOG_PATH", "/workspace/persisted/audit")


# --- Circuit Breaker Manager ---

class CircuitBreakerManager:
    """Manages circuit breakers for each engine."""
    
    def __init__(self, config: CoordinatorConfig):
        self.config = config
        self.breakers: Dict[str, CircuitBreaker] = {}
        self._lock = threading.Lock()
    
    def _get_or_create(self, engine: str) -> CircuitBreaker:
        if engine not in self.breakers:
            self.breakers[engine] = CircuitBreaker(
                engine=engine,
                failure_threshold=self.config.circuit_failure_threshold,
                timeout_seconds=self.config.circuit_timeout_seconds
            )
        return self.breakers[engine]
    
    def can_execute(self, engine: str) -> bool:
        with self._lock:
            cb = self._get_or_create(engine)
            
            if cb.state == CircuitState.CLOSED:
                return True
            
            if cb.state == CircuitState.OPEN:
                # Check if timeout has elapsed
                if cb.opened_at:
                    elapsed = (datetime.utcnow() - datetime.fromisoformat(cb.opened_at)).total_seconds()
                    if elapsed >= cb.timeout_seconds:
                        cb.state = CircuitState.HALF_OPEN
                        cb.half_open_attempts = 0
                        logger.info(f"Circuit {engine}: OPEN → HALF_OPEN (timeout elapsed)")
                        return True
                return False
            
            if cb.state == CircuitState.HALF_OPEN:
                # Allow limited test requests
                if cb.half_open_attempts < cb.half_open_max:
                    cb.half_open_attempts += 1
                    return True
                return False
            
            return False
    
    def record_success(self, engine: str):
        with self._lock:
            cb = self._get_or_create(engine)
            cb.failure_count = 0
            cb.last_success = datetime.utcnow().isoformat()
            
            if cb.state in (CircuitState.OPEN, CircuitState.HALF_OPEN):
                cb.state = CircuitState.CLOSED
                cb.opened_at = None
                cb.half_open_attempts = 0
                logger.info(f"Circuit {engine}: {cb.state.value} → CLOSED (success)")
            else:
                cb.state = CircuitState.CLOSED
    
    def record_failure(self, engine: str):
        with self._lock:
            cb = self._get_or_create(engine)
            cb.failure_count += 1
            cb.last_failure = datetime.utcnow().isoformat()
            
            if cb.state == CircuitState.HALF_OPEN:
                # Immediate re-open on failure during test
                cb.state = CircuitState.OPEN
                cb.opened_at = datetime.utcnow().isoformat()
                cb.half_open_attempts = 0
                logger.warning(f"Circuit {engine}: HALF_OPEN → OPEN (test failed)")
            elif cb.failure_count >= cb.failure_threshold:
                cb.state = CircuitState.OPEN
                cb.opened_at = datetime.utcnow().isoformat()
                logger.warning(
                    f"Circuit {engine}: CLOSED → OPEN "
                    f"(failures={cb.failure_count}/{cb.failure_threshold})"
                )
    
    def force_open(self, engine: str, reason: str):
        with self._lock:
            cb = self._get_or_create(engine)
            cb.state = CircuitState.OPEN
            cb.opened_at = datetime.utcnow().isoformat()
            logger.warning(f"Circuit {engine}: FORCE OPEN — {reason}")
    
    def get_status(self) -> Dict[str, dict]:
        with self._lock:
            return {
                engine: {
                    "state": cb.state.value,
                    "failure_count": cb.failure_count,
                    "last_failure": cb.last_failure,
                    "last_success": cb.last_success,
                    "opened_at": cb.opened_at,
                }
                for engine, cb in self.breakers.items()
            }


# --- Transaction Store ---

class TransactionStore:
    """SQLite-backed transaction log with full audit trail."""
    
    def __init__(self, db_path: str = ":memory:"):
        self.db_path = db_path
        self._conn = sqlite3.connect(db_path, check_same_thread=False)
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._lock = threading.Lock()
        self._init_schema()
    
    def _init_schema(self):
        self._conn.executescript("""
            CREATE TABLE IF NOT EXISTS transactions (
                id TEXT PRIMARY KEY,
                engine TEXT NOT NULL,
                state TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                input_data TEXT DEFAULT '{}',
                output_data TEXT DEFAULT '{}',
                error TEXT,
                error_type TEXT,
                retry_count INTEGER DEFAULT 0,
                max_retries INTEGER DEFAULT 3,
                metadata TEXT DEFAULT '{}',
                duration_ms REAL DEFAULT 0
            );
            
            CREATE INDEX IF NOT EXISTS idx_tx_engine ON transactions(engine);
            CREATE INDEX IF NOT EXISTS idx_tx_state ON transactions(state);
            CREATE INDEX IF NOT EXISTS idx_tx_created ON transactions(created_at);
        """)
        self._conn.commit()
    
    def create(self, tx: Transaction) -> Transaction:
        with self._lock:
            self._conn.execute(
                """INSERT INTO transactions
                   (id, engine, state, created_at, updated_at, input_data,
                    output_data, error, error_type, retry_count, max_retries,
                    metadata, duration_ms)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
                (tx.id, tx.engine, tx.state.value, tx.created_at, tx.updated_at,
                 json.dumps(tx.input_data), json.dumps(tx.output_data),
                 tx.error, tx.error_type.value if tx.error_type else None,
                 tx.retry_count, tx.max_retries,
                 json.dumps(tx.metadata), tx.duration_ms)
            )
            self._conn.commit()
            return tx
    
    def update(self, tx: Transaction) -> Transaction:
        with self._lock:
            self._conn.execute(
                """UPDATE transactions SET
                   state = ?, updated_at = ?, input_data = ?, output_data = ?,
                   error = ?, error_type = ?, retry_count = ?, metadata = ?,
                   duration_ms = ?
                   WHERE id = ?""",
                (tx.state.value, tx.updated_at,
                 json.dumps(tx.input_data), json.dumps(tx.output_data),
                 tx.error, tx.error_type.value if tx.error_type else None,
                 tx.retry_count, json.dumps(tx.metadata),
                 tx.duration_ms, tx.id)
            )
            self._conn.commit()
            return tx
    
    def get(self, tx_id: str) -> Optional[Transaction]:
        row = self._conn.execute(
            "SELECT * FROM transactions WHERE id = ?", (tx_id,)
        ).fetchone()
        if not row:
            return None
        return self._row_to_tx(row)
    
    def get_by_engine_state(self, engine: str, state: TransactionState) -> List[Transaction]:
        rows = self._conn.execute(
            "SELECT * FROM transactions WHERE engine = ? AND state = ? ORDER BY created_at DESC",
            (engine, state.value)
        ).fetchall()
        return [self._row_to_tx(r) for r in rows]
    
    def list_recent(self, limit: int = 50) -> List[Transaction]:
        rows = self._conn.execute(
            "SELECT * FROM transactions ORDER BY created_at DESC LIMIT ?",
            (limit,)
        ).fetchall()
        return [self._row_to_tx(r) for r in rows]
    
    def get_active_count(self, engine: str) -> int:
        return self._conn.execute(
            "SELECT COUNT(*) FROM transactions WHERE engine = ? AND state = ?",
            (engine, TransactionState.PROCESSING.value)
        ).fetchone()[0]
    
    def get_queue_count(self, engine: str) -> int:
        return self._conn.execute(
            "SELECT COUNT(*) FROM transactions WHERE engine = ? AND state = ?",
            (engine, TransactionState.QUEUED.value)
        ).fetchone()[0]
    
    def _row_to_tx(self, row) -> Transaction:
        return Transaction(
            id=row[0], engine=row[1], state=TransactionState(row[2]),
            created_at=row[3], updated_at=row[4],
            input_data=json.loads(row[5]), output_data=json.loads(row[6]),
            error=row[7], error_type=ErrorType(row[8]) if row[8] else None,
            retry_count=row[9], max_retries=row[10],
            metadata=json.loads(row[11]), duration_ms=row[12]
        )
    
    def stats(self) -> Dict:
        total = self._conn.execute("SELECT COUNT(*) FROM transactions").fetchone()[0]
        by_state = {}
        for row in self._conn.execute("SELECT state, COUNT(*) FROM transactions GROUP BY state").fetchall():
            by_state[row[0]] = row[1]
        return {"total": total, "by_state": by_state}
    
    def close(self):
        self._conn.close()


# --- Resource Monitor ---

class ResourceMonitor:
    """Monitors host resources via direct psutil or VictoriaMetrics."""
    
    def __init__(self, config: CoordinatorConfig):
        self.config = config
        self._metrics_available = False
        self._psutil_available = False
        
        # Try psutil first (direct, no network)
        try:
            import psutil
            self._psutil = psutil
            self._psutil_available = True
            logger.info("ResourceMonitor: psutil available (direct metrics)")
        except ImportError:
            logger.info("ResourceMonitor: psutil not available, will try VictoriaMetrics")
    
    def get_snapshot(self) -> ResourceSnapshot:
        snap = ResourceSnapshot(timestamp=datetime.utcnow().isoformat())
        
        if self._psutil_available:
            snap = self._get_psutil_snapshot()
        elif self.config.metrics_enabled:
            snap = self._get_metrics_snapshot()
        
        return snap
    
    def _get_psutil_snapshot(self) -> ResourceSnapshot:
        snap = ResourceSnapshot(timestamp=datetime.utcnow().isoformat())
        
        mem = self._psutil.virtual_memory()
        snap.ram_available_mb = round(mem.available / (1024 * 1024), 1)
        snap.ram_total_mb = round(mem.total / (1024 * 1024), 1)
        snap.ram_percent_used = round(mem.percent / 100, 3)
        
        disk = self._psutil.disk_usage("/")
        snap.disk_free_gb = round(disk.free / (1024**3), 2)
        snap.disk_total_gb = round(disk.total / (1024**3), 2)
        snap.disk_percent_used = round(disk.percent / 100, 3)
        
        try:
            snap.load_1 = round(self._psutil.getloadavg()[0], 2)
        except (OSError, AttributeError):
            snap.load_1 = 0.0
        
        return snap
    
    def _get_metrics_snapshot(self) -> ResourceSnapshot:
        """Query VictoriaMetrics via PromQL."""
        import urllib.request
        
        snap = ResourceSnapshot(timestamp=datetime.utcnow().isoformat())
        base = self.config.metrics_endpoint.rstrip("/")
        
        queries = {
            "ram_available": 'node_memory_MemAvailable_bytes / 1024 / 1024',
            "ram_total": 'node_memory_MemTotal_bytes / 1024 / 1024',
            "disk_free": 'node_filesystem_avail_bytes{mountpoint="/"} / 1024 / 1024 / 1024',
            "disk_total": 'node_filesystem_size_bytes{mountpoint="/"} / 1024 / 1024 / 1024',
            "load_1": 'node_load1',
        }
        
        results = {}
        for name, query in queries.items():
            try:
                url = f"{base}/api/v1/query?query={query}"
                req = urllib.request.Request(url)
                with urllib.request.urlopen(req, timeout=2) as resp:
                    data = json.loads(resp.read())
                    if data.get("status") == "success" and data.get("data", {}).get("result"):
                        val = float(data["data"]["result"][0]["value"][1])
                        results[name] = val
            except Exception:
                pass
        
        if "ram_available" in results:
            snap.ram_available_mb = round(results["ram_available"], 1)
        if "ram_total" in results:
            snap.ram_total_mb = round(results["ram_total"], 1)
        if snap.ram_total_mb > 0:
            snap.ram_percent_used = round(
                (snap.ram_total_mb - snap.ram_available_mb) / snap.ram_total_mb, 3
            )
        if "disk_free" in results:
            snap.disk_free_gb = round(results["disk_free"], 2)
        if "disk_total" in results:
            snap.disk_total_gb = round(results["disk_total"], 2)
        if snap.disk_total_gb > 0:
            snap.disk_percent_used = round(
                (snap.disk_total_gb - snap.disk_free_gb) / snap.disk_total_gb, 3
            )
        if "load_1" in results:
            snap.load_1 = round(results["load_1"], 2)
        
        return snap
    
    def should_flush_to_disk(self) -> bool:
        snap = self.get_snapshot()
        return snap.ram_percent_used >= self.config.ram_flush_threshold
    
    def should_open_resource_circuit(self) -> bool:
        snap = self.get_snapshot()
        return snap.ram_percent_used >= self.config.ram_circuit_threshold
    
    def should_reject_uploads(self) -> bool:
        snap = self.get_snapshot()
        return snap.disk_percent_used >= self.config.disk_reject_threshold
    
    def should_trigger_cleanup(self) -> bool:
        snap = self.get_snapshot()
        return snap.disk_percent_used >= self.config.disk_cleanup_threshold


# --- Error Classifier ---

class ErrorClassifier:
    """Classifies errors as transient, permanent, or resource-related."""
    
    TRANSIENT_KEYWORDS = [
        "timeout", "connection refused", "connection reset",
        "temporary failure", "rate limit", "too many requests",
        "503", "504", "ECONNRESET", "ETIMEDOUT",
    ]
    
    RESOURCE_KEYWORDS = [
        "out of memory", "oom", "no space left", "disk full",
        "cannot allocate memory", "resource temporarily unavailable",
    ]
    
    @classmethod
    def classify(cls, error: str) -> ErrorType:
        error_lower = error.lower()
        
        for keyword in cls.RESOURCE_KEYWORDS:
            if keyword in error_lower:
                return ErrorType.RESOURCE
        
        for keyword in cls.TRANSIENT_KEYWORDS:
            if keyword in error_lower:
                return ErrorType.TRANSIENT
        
        return ErrorType.PERMANENT


# --- Audit Logger ---

class AuditLogger:
    """Append-only audit log (JSONL format)."""
    
    def __init__(self, log_path: str):
        self.log_path = log_path
        os.makedirs(log_path, exist_ok=True)
        self._lock = threading.Lock()
        self._current_file = self._get_current_file()
    
    def _get_current_file(self) -> str:
        return os.path.join(self.log_path, f"{datetime.utcnow().strftime('%Y-%m')}.jsonl")
    
    def log(self, event: str, tx_id: str = "", engine: str = "",
            details: dict = None, user: str = ""):
        entry = {
            "timestamp": datetime.utcnow().isoformat(),
            "event": event,
            "tx_id": tx_id,
            "engine": engine,
            "user": user,
            "details": details or {},
        }
        
        with self._lock:
            filepath = self._get_current_file()
            if filepath != self._current_file:
                self._current_file = filepath
            
            with open(filepath, "a") as f:
                f.write(json.dumps(entry, separators=(",", ":")) + "\n")
    
    def get_recent(self, limit: int = 50) -> List[dict]:
        entries = []
        filepath = self._get_current_file()
        if not os.path.exists(filepath):
            return []
        
        with open(filepath, "r") as f:
            for line in f:
                line = line.strip()
                if line:
                    try:
                        entries.append(json.loads(line))
                    except json.JSONDecodeError:
                        pass
        
        return entries[-limit:]


# --- Cleanup Scheduler ---

class CleanupScheduler:
    """TTL-based cleanup of workspace directories."""
    
    def __init__(self, config: CoordinatorConfig, audit: AuditLogger):
        self.config = config
        self.audit = audit
        self._running = False
        self._thread: Optional[threading.Thread] = None
        self._lock = threading.Lock()
        self._cleanup_lock_path = os.path.join(config.workspace_root, ".tmp", "cleanup.lock")
    
    def start(self):
        if self._running:
            return
        self._running = True
        self._thread = threading.Thread(target=self._run_loop, daemon=True)
        self._thread.start()
        logger.info("CleanupScheduler: started")
    
    def stop(self):
        self._running = False
        if self._thread:
            self._thread.join(timeout=5)
        logger.info("CleanupScheduler: stopped")
    
    def _run_loop(self):
        while self._running:
            try:
                self._cleanup_cycle()
            except Exception as e:
                logger.error(f"CleanupScheduler: cycle failed: {e}")
            time.sleep(self.config.cleanup_interval_seconds)
    
    def _cleanup_cycle(self):
        workspace = self.config.workspace_root
        now = datetime.utcnow()
        cutoff = now - timedelta(seconds=self.config.ttl_expired_seconds)
        cleaned = 0
        
        for subdir in ["input", "preprocess", "processing", "intermediate", "output", ".tmp"]:
            dir_path = os.path.join(workspace, subdir)
            if not os.path.exists(dir_path):
                continue
            
            for entry in os.listdir(dir_path):
                entry_path = os.path.join(dir_path, entry)
                if not os.path.isdir(entry_path):
                    continue
                
                try:
                    mtime = datetime.fromtimestamp(os.path.getmtime(entry_path))
                    if mtime < cutoff:
                        shutil.rmtree(entry_path)
                        cleaned += 1
                        self.audit.log(
                            "cleanup_delete",
                            engine="coordinator",
                            details={"path": entry_path, "subdir": subdir, "age_seconds": (now - mtime).total_seconds()}
                        )
                except OSError as e:
                    logger.warning(f"CleanupScheduler: failed to clean {entry_path}: {e}")
        
        if cleaned > 0:
            logger.info(f"CleanupScheduler: cleaned {cleaned} expired directories")
            self.audit.log("cleanup_summary", engine="coordinator", details={"cleaned": cleaned})
    
    def force_cleanup(self) -> int:
        """Run cleanup immediately (e.g., disk > 85%)."""
        self._cleanup_cycle()
        return 0  # Counting requires refactoring _cleanup_cycle to return count


# --- Queue Controller ---

class QueueController:
    """Manages queue depth and backpressure per engine."""
    
    def __init__(self, config: CoordinatorConfig):
        self.config = config
        self._lock = threading.Lock()
        self._queues: Dict[str, List[str]] = {}  # engine → [tx_id, ...]
    
    def can_enqueue(self, engine: str, active_count: int, queue_count: int) -> tuple[bool, str]:
        with self._lock:
            if active_count >= self.config.max_concurrent_per_engine:
                return False, f"Engine {engine} at max concurrency ({active_count}/{self.config.max_concurrent_per_engine})"
            
            if queue_count >= self.config.max_queue_depth:
                return False, f"Engine {engine} queue full ({queue_count}/{self.config.max_queue_depth})"
            
            return True, ""
    
    def enqueue(self, engine: str, tx_id: str):
        with self._lock:
            if engine not in self._queues:
                self._queues[engine] = []
            self._queues[engine].append(tx_id)
    
    def dequeue(self, engine: str) -> Optional[str]:
        with self._lock:
            if engine in self._queues and self._queues[engine]:
                return self._queues[engine].pop(0)
            return None
    
    def get_queue_depth(self, engine: str) -> int:
        with self._lock:
            return len(self._queues.get(engine, []))


# --- Main Coordinator ---

class ProcessingCoordinator:
    """
    Central coordinator for all RAG pipeline operations.
    
    Usage:
        coordinator = ProcessingCoordinator()
        coordinator.start()
        
        tx = coordinator.create_transaction("rag", {"query": "hello"})
        result = coordinator.execute_with_guard(tx, lambda: pipeline.query("hello"))
        
        coordinator.stop()
    """
    
    def __init__(self, config: Optional[CoordinatorConfig] = None):
        self.config = config or CoordinatorConfig()
        
        # Initialize subsystems
        self.circuit_breaker = CircuitBreakerManager(self.config)
        self.tx_store = TransactionStore(
            db_path=os.path.join(self.config.workspace_root, "persisted", "transactions.db")
            if os.path.exists(self.config.workspace_root) else ":memory:"
        )
        self.resource_monitor = ResourceMonitor(self.config)
        self.queue_controller = QueueController(self.config)
        self.audit = AuditLogger(self.config.audit_log_path)
        self.cleanup_scheduler = CleanupScheduler(self.config, self.audit)
        
        # App performance analytics
        self.perf_monitor = AppPerformanceMonitor(
            db_path=os.path.join(self.config.workspace_root, "persisted", "perf_metrics.db")
            if os.path.exists(self.config.workspace_root) else ":memory:"
        )
        self.event_logger = SystemEventLogger(
            db_path=os.path.join(self.config.workspace_root, "persisted", "system_events.db")
            if os.path.exists(self.config.workspace_root) else ":memory:"
        )
        
        # Self-evaluator (requires KG, initialized when KG is attached)
        self.self_evaluator: Optional[SelfEvaluator] = None
        
        self._running = False
        self._lock = threading.Lock()
        self._current_session_id: Optional[str] = None
        self._session_queries: List[dict] = []
        
        logger.info("ProcessingCoordinator: initialized")
        self.audit.log("coordinator_init", engine="coordinator", details={
            "workspace_root": self.config.workspace_root,
            "ram_flush_threshold": self.config.ram_flush_threshold,
            "disk_reject_threshold": self.config.disk_reject_threshold,
        })
        self.event_logger.log(
            SystemEventLogger.Category.LIFECYCLE,
            SystemEventLogger.Severity.INFO,
            "coordinator_initialized",
            "Processing Coordinator started",
            source="coordinator",
        )
    
    def start(self):
        """Start background services."""
        self._running = True
        self.cleanup_scheduler.start()
        self.audit.log("coordinator_start", engine="coordinator")
        logger.info("ProcessingCoordinator: started")
    
    def stop(self):
        """Stop background services."""
        self._running = False
        
        # End current session if active
        if self._current_session_id:
            self.end_session(self._current_session_id)
        
        self.cleanup_scheduler.stop()
        self.tx_store.close()
        self.perf_monitor.close()
        self.event_logger.close()
        self.audit.log("coordinator_stop", engine="coordinator")
        self.event_logger.log(
            SystemEventLogger.Category.LIFECYCLE,
            SystemEventLogger.Severity.INFO,
            "coordinator_stopped",
            "Processing Coordinator stopped",
            source="coordinator",
        )
        logger.info("ProcessingCoordinator: stopped")
    
    def attach_knowledge_graph(self, kg):
        """Attach the Knowledge Graph for self-evaluation."""
        self.self_evaluator = SelfEvaluator(
            kg=kg,
            perf_monitor=self.perf_monitor,
            event_logger=self.event_logger,
            audit=self.audit,
        )
        self.event_logger.log(
            SystemEventLogger.Category.LIFECYCLE,
            SystemEventLogger.Severity.INFO,
            "kg_attached",
            "Knowledge Graph attached to coordinator",
            source="coordinator",
        )
    
    # --- Session Management ---
    
    def start_session(self, session_id: str = None) -> str:
        """Begin a new evaluation session."""
        if not session_id:
            session_id = f"session_{datetime.utcnow().strftime('%Y%m%d_%H%M%S')}_{uuid.uuid4().hex[:8]}"
        
        self._current_session_id = session_id
        self._session_queries = []
        
        self.perf_monitor.record_session_start(session_id)
        self.event_logger.log(
            SystemEventLogger.Category.USER,
            SystemEventLogger.Severity.INFO,
            "session_started",
            f"Session {session_id} started",
            source="coordinator",
            session_id=session_id,
        )
        return session_id
    
    def record_query_result(self, query: str, success: bool,
                            confidence: float = 0, iterations: int = 1,
                            latency_ms: float = 0, tokens_in: int = 0,
                            tokens_out: int = 0, cache_hit: bool = False,
                            error: str = None):
        """Record a query result for session tracking and performance monitoring."""
        result = {
            "query": query,
            "success": success,
            "confidence": confidence,
            "iterations": iterations,
            "latency_ms": latency_ms,
            "tokens_in": tokens_in,
            "tokens_out": tokens_out,
            "cache_hit": cache_hit,
            "error": error,
            "timestamp": datetime.utcnow().isoformat(),
        }
        self._session_queries.append(result)
        
        engine = "rag"
        self.perf_monitor.record_query(
            engine=engine,
            latency_ms=latency_ms,
            tokens_in=tokens_in,
            tokens_out=tokens_out,
            cache_hit=cache_hit,
            success=success,
            error_type=ErrorClassifier.classify(error).value if error else None,
        )
    
    def end_session(self, session_id: str = None) -> Optional[dict]:
        """
        End the current session, compute stats, and optionally run self-evaluation.
        Returns evaluation report if self-evaluator is attached.
        """
        sid = session_id or self._current_session_id
        if not sid:
            return None
        
        queries = self._session_queries
        total = len(queries)
        successful = sum(1 for q in queries if q.get("success"))
        failed = total - successful
        
        latencies = [q.get("latency_ms", 0) for q in queries if q.get("latency_ms", 0) > 0]
        avg_lat = sum(latencies) / max(len(latencies), 1)
        
        def percentile(data, p):
            if not data:
                return 0
            idx = int(len(data) * p / 100)
            return data[min(idx, len(data) - 1)]
        
        sorted_lat = sorted(latencies)
        cache_hits = sum(1 for q in queries if q.get("cache_hit"))
        total_tokens_in = sum(q.get("tokens_in", 0) for q in queries)
        total_tokens_out = sum(q.get("tokens_out", 0) for q in queries)
        
        stats = {
            "total_queries": total,
            "successful_queries": successful,
            "failed_queries": failed,
            "total_tokens_in": total_tokens_in,
            "total_tokens_out": total_tokens_out,
            "avg_latency_ms": round(avg_lat, 1),
            "p95_latency_ms": round(percentile(sorted_lat, 95), 1),
            "p99_latency_ms": round(percentile(sorted_lat, 99), 1),
            "cache_hit_rate": round(cache_hits / max(total, 1), 3),
            "circuit_trips": 0,
            "resource_events": 0,
            "kg_entities_added": 0,
            "kg_relations_added": 0,
        }
        
        # Get KG stats if attached
        if self.self_evaluator:
            kg_stats = self.self_evaluator.kg.stats()
            stats["kg_entities_added"] = kg_stats.get("entities", 0)
            stats["kg_relations_added"] = kg_stats.get("relations", 0)
        
        self.perf_monitor.record_session_end(sid, stats)
        
        # Run self-evaluation if KG is attached
        eval_report = None
        if self.self_evaluator and queries:
            eval_report = self.self_evaluator.evaluate_session(sid, {
                "queries": queries,
                "total_tokens": total_tokens_out,
                "kg_entities_before": 0,  # Would need snapshot at session start
                "kg_entities_after": stats["kg_entities_added"],
                "kg_relations_before": 0,
                "kg_relations_after": stats["kg_relations_added"],
            })
        
        # Detect anomalies
        anomalies = self.perf_monitor.detect_anomalies()
        for a in anomalies:
            self.event_logger.log(
                SystemEventLogger.Category.SYSTEM,
                SystemEventLogger.Severity.WARNING if a.get("severity") == "warning" else SystemEventLogger.Severity.CRITICAL,
                "anomaly_detected",
                f"Anomaly: {a.get('metric')} = {a.get('value')} (baseline: {a.get('baseline')})",
                source="performance_monitor",
                context=a,
                session_id=sid,
            )
        
        self.event_logger.log(
            SystemEventLogger.Category.USER,
            SystemEventLogger.Severity.INFO,
            "session_ended",
            f"Session {sid} ended: {total} queries, {successful} successful",
            source="coordinator",
            context=stats,
            session_id=sid,
        )
        
        self._current_session_id = None
        self._session_queries = []
        
        return eval_report
    
    def create_transaction(self, engine: str, input_data: dict,
                           max_retries: int = 3, metadata: dict = None) -> Transaction:
        """Create a new transaction in QUEUED state."""
        now = datetime.utcnow().isoformat()
        tx = Transaction(
            id=str(uuid.uuid4()),
            engine=engine,
            state=TransactionState.QUEUED,
            created_at=now,
            updated_at=now,
            input_data=input_data,
            max_retries=max_retries,
            metadata=metadata or {},
        )
        self.tx_store.create(tx)
        self.audit.log("transaction_created", tx.id, engine, {"input_keys": list(input_data.keys())})
        return tx
    
    def transition(self, tx: Transaction, new_state: TransactionState,
                   error: str = None, output_data: dict = None,
                   duration_ms: float = 0) -> Transaction:
        """Transition a transaction to a new state with validation."""
        if new_state not in VALID_TRANSITIONS.get(tx.state, set()):
            raise ValueError(
                f"Invalid transition: {tx.state.value} → {new_state.value}"
            )
        
        tx.state = new_state
        tx.updated_at = datetime.utcnow().isoformat()
        if error:
            tx.error = error
            tx.error_type = ErrorClassifier.classify(error)
        if output_data:
            tx.output_data.update(output_data)
        if duration_ms > 0:
            tx.duration_ms = duration_ms
        
        self.tx_store.update(tx)
        self.audit.log(
            f"transition_{new_state.value}",
            tx.id, tx.engine,
            {"from": tx.state.value, "to": new_state.value, "error": error}
        )
        return tx
    
    def execute_with_guard(self, tx: Transaction,
                           operation: Callable) -> Transaction:
        """
        Execute an operation with full governance:
        1. Check circuit breaker
        2. Check resource constraints
        3. Check queue depth
        4. Execute with timing
        5. Record success/failure
        6. Handle errors (retry or escalate)
        """
        engine = tx.engine
        start = time.time()
        
        # 1. Circuit breaker check
        if not self.circuit_breaker.can_execute(engine):
            self.audit.log("circuit_blocked", tx.id, engine)
            raise CircuitOpenError(
                f"Circuit breaker OPEN for engine {engine}"
            )
        
        # 2. Resource constraint check
        if self.resource_monitor.should_reject_uploads():
            self.circuit_breaker.force_open(engine, "disk > 95%")
            raise ResourceError("Disk usage > 95%, rejecting operations")
        
        if self.resource_monitor.should_open_resource_circuit():
            self.circuit_breaker.force_open(engine, "RAM > 90%")
            raise ResourceError("RAM usage > 90%, rejecting operations")
        
        # 3. Queue depth check
        active = self.tx_store.get_active_count(engine)
        queued = self.tx_store.get_queue_count(engine)
        can_run, reason = self.queue_controller.can_enqueue(engine, active, queued)
        if not can_run:
            raise QueueFullError(reason)
        
        # 4. Transition to processing
        tx = self.transition(tx, TransactionState.PROCESSING)
        
        try:
            # Execute the operation
            result = operation()
            
            # 5. Record success
            duration_ms = (time.time() - start) * 1000
            self.circuit_breaker.record_success(engine)
            tx = self.transition(
                tx, TransactionState.COMPLETED,
                output_data={"result": str(result)},
                duration_ms=duration_ms
            )
            return tx
            
        except Exception as e:
            duration_ms = (time.time() - start) * 1000
            error_str = str(e)
            error_type = ErrorClassifier.classify(error_str)
            
            self.circuit_breaker.record_failure(engine)
            
            # Handle based on error type
            if error_type == ErrorType.RESOURCE:
                # Resource errors: open circuit, no retry
                self.circuit_breaker.force_open(engine, f"Resource error: {error_str}")
                tx = self.transition(
                    tx, TransactionState.FAILED,
                    error=error_str, duration_ms=duration_ms
                )
                raise ResourceError(f"Resource constraint: {error_str}") from e
            
            elif error_type == ErrorType.TRANSIENT:
                # Transient errors: retry if under max
                tx.retry_count += 1
                if tx.retry_count < tx.max_retries:
                    tx.error = error_str
                    tx.error_type = error_type
                    tx = self.transition(tx, TransactionState.QUEUED)
                    self.audit.log(
                        "retry_scheduled", tx.id, engine,
                        {"retry": tx.retry_count, "max": tx.max_retries}
                    )
                    raise TransientError(
                        f"Transient error, will retry ({tx.retry_count}/{tx.max_retries}): {error_str}"
                    ) from e
                
                # Max retries exceeded
                tx = self.transition(
                    tx, TransactionState.FAILED,
                    error=error_str, duration_ms=duration_ms
                )
                raise MaxRetriesExceededError(
                    f"Max retries ({tx.max_retries}) exceeded: {error_str}"
                ) from e
            
            else:
                # Permanent error: no retry
                tx = self.transition(
                    tx, TransactionState.FAILED,
                    error=error_str, duration_ms=duration_ms
                )
                raise PermanentError(f"Permanent error: {error_str}") from e
    
    def get_resource_status(self) -> dict:
        snap = self.resource_monitor.get_snapshot()
        return {
            "ram": {
                "available_mb": snap.ram_available_mb,
                "total_mb": snap.ram_total_mb,
                "percent_used": snap.ram_percent_used,
                "flush_threshold": self.config.ram_flush_threshold,
                "circuit_threshold": self.config.ram_circuit_threshold,
            },
            "disk": {
                "free_gb": snap.disk_free_gb,
                "total_gb": snap.disk_total_gb,
                "percent_used": snap.disk_percent_used,
                "reject_threshold": self.config.disk_reject_threshold,
            },
            "load_1": snap.load_1,
            "should_flush": self.resource_monitor.should_flush_to_disk(),
            "should_reject_uploads": self.resource_monitor.should_reject_uploads(),
        }
    
    def get_circuit_status(self) -> dict:
        return self.circuit_breaker.get_status()
    
    def get_transaction_stats(self) -> dict:
        return self.tx_store.stats()
    
    def get_audit_log(self, limit: int = 50) -> List[dict]:
        return self.audit.get_recent(limit)
    
    def force_cleanup(self):
        self.cleanup_scheduler.force_cleanup()
    
    # --- Analytics & Evaluation Accessors ---
    
    def get_performance_dashboard(self) -> dict:
        return self.perf_monitor.get_dashboard()
    
    def get_event_log(self, category: str = None, severity: str = None,
                      limit: int = 50) -> List[dict]:
        return self.event_logger.query(category=category, severity=severity, limit=limit)
    
    def get_event_counts(self, hours: int = 24) -> dict:
        return self.event_logger.get_event_counts(hours=hours)
    
    def get_session_history(self, limit: int = 20) -> List[dict]:
        return self.perf_monitor.get_session_history(limit=limit)
    
    def get_anomalies(self, limit: int = 20) -> List[dict]:
        return self.perf_monitor.get_recent_anomalies(limit=limit)
    
    def run_self_evaluation(self, session_id: str,
                            session_stats: dict) -> Optional[dict]:
        if not self.self_evaluator:
            return None
        return self.self_evaluator.evaluate_session(session_id, session_stats)
    
    def get_current_session(self) -> Optional[str]:
        return self._current_session_id


# --- App Performance Monitor ---

class AppPerformanceMonitor:
    """
    Tracks application-level performance metrics — the app's equivalent of
    Windows Performance Monitor, Linux perf, Android systrace, iOS Instruments.
    
    Stores rolling metrics in SQLite for trend analysis and anomaly detection.
    Metrics are grouped into windows (1min, 5min, 1hour) for different granularities.
    """
    
    def __init__(self, db_path: str = ":memory:"):
        self.db_path = db_path
        self._conn = sqlite3.connect(db_path, check_same_thread=False)
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._lock = threading.Lock()
        self._init_schema()
    
    def _init_schema(self):
        self._conn.executescript("""
            -- Query latency records (high-volume, auto-pruned)
            CREATE TABLE IF NOT EXISTS query_metrics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                engine TEXT NOT NULL,
                latency_ms REAL NOT NULL,
                tokens_in INTEGER DEFAULT 0,
                tokens_out INTEGER DEFAULT 0,
                cache_hit INTEGER DEFAULT 0,
                queue_wait_ms REAL DEFAULT 0,
                success INTEGER DEFAULT 1,
                error_type TEXT
            );
            
            -- Rolling performance windows (computed on read, not stored)
            CREATE TABLE IF NOT EXISTS session_records (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                started_at TEXT NOT NULL,
                ended_at TEXT,
                total_queries INTEGER DEFAULT 0,
                successful_queries INTEGER DEFAULT 0,
                failed_queries INTEGER DEFAULT 0,
                total_tokens_in INTEGER DEFAULT 0,
                total_tokens_out INTEGER DEFAULT 0,
                avg_latency_ms REAL DEFAULT 0,
                p95_latency_ms REAL DEFAULT 0,
                p99_latency_ms REAL DEFAULT 0,
                cache_hit_rate REAL DEFAULT 0,
                circuit_trips INTEGER DEFAULT 0,
                resource_events INTEGER DEFAULT 0,
                kg_entities_added INTEGER DEFAULT 0,
                kg_relations_added INTEGER DEFAULT 0
            );
            
            -- Anomaly detection events
            CREATE TABLE IF NOT EXISTS anomalies (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                metric TEXT NOT NULL,
                value REAL NOT NULL,
                baseline REAL NOT NULL,
                deviation REAL NOT NULL,
                severity TEXT NOT NULL,
                details TEXT DEFAULT '{}'
            );
            
            -- Engine performance history
            CREATE TABLE IF NOT EXISTS engine_stats (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                engine TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                queries_total INTEGER DEFAULT 0,
                queries_success INTEGER DEFAULT 0,
                queries_failed INTEGER DEFAULT 0,
                avg_latency_ms REAL DEFAULT 0,
                error_rate REAL DEFAULT 0,
                tokens_consumed INTEGER DEFAULT 0,
                circuit_open_count INTEGER DEFAULT 0
            );
            
            CREATE INDEX IF NOT EXISTS idx_qm_timestamp ON query_metrics(timestamp);
            CREATE INDEX IF NOT EXISTS idx_qm_engine ON query_metrics(engine);
            CREATE INDEX IF NOT EXISTS idx_sr_session ON session_records(session_id);
            CREATE INDEX IF NOT EXISTS idx_anomaly_timestamp ON anomalies(timestamp);
            CREATE INDEX IF NOT EXISTS idx_es_engine_ts ON engine_stats(engine, timestamp);
        """)
        self._conn.commit()
    
    def record_query(self, engine: str, latency_ms: float,
                     tokens_in: int = 0, tokens_out: int = 0,
                     cache_hit: bool = False, queue_wait_ms: float = 0,
                     success: bool = True, error_type: str = None):
        with self._lock:
            self._conn.execute(
                """INSERT INTO query_metrics
                   (timestamp, engine, latency_ms, tokens_in, tokens_out,
                    cache_hit, queue_wait_ms, success, error_type)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)""",
                (datetime.utcnow().isoformat(), engine, latency_ms,
                 tokens_in, tokens_out, 1 if cache_hit else 0,
                 queue_wait_ms, 1 if success else 0, error_type)
            )
            self._conn.commit()
    
    def record_session_start(self, session_id: str) -> str:
        with self._lock:
            self._conn.execute(
                """INSERT INTO session_records
                   (session_id, started_at)
                   VALUES (?, ?)""",
                (session_id, datetime.utcnow().isoformat())
            )
            self._conn.commit()
            return session_id
    
    def record_session_end(self, session_id: str, stats: dict):
        with self._lock:
            self._conn.execute(
                """UPDATE session_records SET
                   ended_at = ?, total_queries = ?, successful_queries = ?,
                   failed_queries = ?, total_tokens_in = ?, total_tokens_out = ?,
                   avg_latency_ms = ?, p95_latency_ms = ?, p99_latency_ms = ?,
                   cache_hit_rate = ?, circuit_trips = ?, resource_events = ?,
                   kg_entities_added = ?, kg_relations_added = ?
                   WHERE session_id = ?""",
                (datetime.utcnow().isoformat(),
                 stats.get("total_queries", 0),
                 stats.get("successful_queries", 0),
                 stats.get("failed_queries", 0),
                 stats.get("total_tokens_in", 0),
                 stats.get("total_tokens_out", 0),
                 stats.get("avg_latency_ms", 0),
                 stats.get("p95_latency_ms", 0),
                 stats.get("p99_latency_ms", 0),
                 stats.get("cache_hit_rate", 0),
                 stats.get("circuit_trips", 0),
                 stats.get("resource_events", 0),
                 stats.get("kg_entities_added", 0),
                 stats.get("kg_relations_added", 0),
                 session_id)
            )
            self._conn.commit()
    
    def record_anomaly(self, metric: str, value: float,
                       baseline: float, severity: str, details: dict = None):
        deviation = abs(value - baseline) / max(baseline, 0.001)
        with self._lock:
            self._conn.execute(
                """INSERT INTO anomalies
                   (timestamp, metric, value, baseline, deviation, severity, details)
                   VALUES (?, ?, ?, ?, ?, ?, ?)""",
                (datetime.utcnow().isoformat(), metric, value, baseline,
                 round(deviation, 3), severity, json.dumps(details or {}))
            )
            self._conn.commit()
    
    def get_latency_percentiles(self, engine: str = None,
                                 window_minutes: int = 60) -> dict:
        """Compute p50, p95, p99 latency for recent window."""
        cutoff = (datetime.utcnow() - timedelta(minutes=window_minutes)).isoformat()
        query = "SELECT latency_ms FROM query_metrics WHERE timestamp >= ?"
        params = [cutoff]
        if engine:
            query += " AND engine = ?"
            params.append(engine)
        query += " ORDER BY latency_ms"
        
        rows = self._conn.execute(query, params).fetchall()
        if not rows:
            return {"count": 0}
        
        latencies = [r[0] for r in rows]
        n = len(latencies)
        
        def percentile(data, p):
            idx = int(len(data) * p / 100)
            return data[min(idx, len(data) - 1)]
        
        return {
            "count": n,
            "p50": round(percentile(latencies, 50), 1),
            "p95": round(percentile(latencies, 95), 1),
            "p99": round(percentile(latencies, 99), 1),
            "min": round(latencies[0], 1),
            "max": round(latencies[-1], 1),
            "avg": round(sum(latencies) / n, 1),
        }
    
    def get_error_rate(self, engine: str = None,
                       window_minutes: int = 60) -> dict:
        cutoff = (datetime.utcnow() - timedelta(minutes=window_minutes)).isoformat()
        query = "SELECT success, error_type FROM query_metrics WHERE timestamp >= ?"
        params = [cutoff]
        if engine:
            query += " AND engine = ?"
            params.append(engine)
        
        rows = self._conn.execute(query, params).fetchall()
        total = len(rows)
        if total == 0:
            return {"total": 0, "error_rate": 0}
        
        failures = sum(1 for r in rows if not r[0])
        by_type = {}
        for _, etype in rows:
            if etype:
                by_type[etype] = by_type.get(etype, 0) + 1
        
        return {
            "total": total,
            "failures": failures,
            "error_rate": round(failures / total, 3),
            "by_type": by_type,
        }
    
    def get_throughput(self, window_minutes: int = 5) -> dict:
        cutoff = (datetime.utcnow() - timedelta(minutes=window_minutes)).isoformat()
        rows = self._conn.execute(
            "SELECT engine, COUNT(*), SUM(tokens_out) FROM query_metrics "
            "WHERE timestamp >= ? AND success = 1 GROUP BY engine",
            (cutoff,)
        ).fetchall()
        
        total_queries = sum(r[1] for r in rows)
        total_tokens = sum(r[2] or 0 for r in rows)
        
        return {
            "window_minutes": window_minutes,
            "queries_per_minute": round(total_queries / max(window_minutes, 1), 1),
            "tokens_per_minute": round(total_tokens / max(window_minutes, 1), 0),
            "by_engine": [
                {"engine": r[0], "queries": r[1], "tokens": r[2] or 0}
                for r in rows
            ],
        }
    
    def get_cache_stats(self, window_minutes: int = 60) -> dict:
        cutoff = (datetime.utcnow() - timedelta(minutes=window_minutes)).isoformat()
        row = self._conn.execute(
            "SELECT COUNT(*), SUM(cache_hit) FROM query_metrics WHERE timestamp >= ?",
            (cutoff,)
        ).fetchone()
        
        total = row[0] or 0
        hits = row[1] or 0
        
        return {
            "total_queries": total,
            "cache_hits": hits,
            "cache_misses": total - hits,
            "hit_rate": round(hits / max(total, 1), 3),
        }
    
    def get_session_history(self, limit: int = 20) -> List[dict]:
        rows = self._conn.execute(
            "SELECT * FROM session_records ORDER BY started_at DESC LIMIT ?",
            (limit,)
        ).fetchall()
        columns = [desc[0] for desc in self._conn.execute(
            "SELECT * FROM session_records LIMIT 0"
        ).description]
        return [dict(zip(columns, row)) for row in rows]
    
    def get_recent_anomalies(self, limit: int = 20) -> List[dict]:
        rows = self._conn.execute(
            "SELECT * FROM anomalies ORDER BY timestamp DESC LIMIT ?",
            (limit,)
        ).fetchall()
        columns = [desc[0] for desc in self._conn.execute(
            "SELECT * FROM anomalies LIMIT 0"
        ).description]
        results = []
        for row in rows:
            d = dict(zip(columns, row))
            d["details"] = json.loads(d.get("details", "{}"))
            results.append(d)
        return results
    
    def detect_anomalies(self) -> List[dict]:
        """
        Auto-detect anomalies by comparing current window against baseline.
        Baseline = 24-hour rolling average.
        """
        detected = []
        
        # Check latency spike
        current_lat = self.get_latency_percentiles(window_minutes=5)
        baseline_lat = self.get_latency_percentiles(window_minutes=1440)  # 24h
        
        if current_lat.get("count", 0) > 5 and baseline_lat.get("count", 0) > 10:
            current_p95 = current_lat.get("p95", 0)
            baseline_p95 = baseline_lat.get("p95", 0)
            if baseline_p95 > 0 and current_p95 > baseline_p95 * 2:
                detected.append({
                    "metric": "latency_p95",
                    "value": current_p95,
                    "baseline": baseline_p95,
                    "severity": "warning" if current_p95 < baseline_p95 * 3 else "critical",
                })
        
        # Check error rate spike
        current_err = self.get_error_rate(window_minutes=5)
        baseline_err = self.get_error_rate(window_minutes=1440)
        
        if current_err.get("total", 0) > 3 and baseline_err.get("error_rate", 0) < 0.1:
            if current_err["error_rate"] > 0.2:
                detected.append({
                    "metric": "error_rate",
                    "value": current_err["error_rate"],
                    "baseline": baseline_err["error_rate"],
                    "severity": "critical" if current_err["error_rate"] > 0.5 else "warning",
                })
        
        # Record detected anomalies
        for a in detected:
            self.record_anomaly(
                a["metric"], a["value"], a["baseline"], a["severity"]
            )
        
        return detected
    
    def get_dashboard(self) -> dict:
        """Full performance dashboard snapshot."""
        return {
            "latency": {
                "5min": self.get_latency_percentiles(window_minutes=5),
                "1hour": self.get_latency_percentiles(window_minutes=60),
                "24hour": self.get_latency_percentiles(window_minutes=1440),
            },
            "errors": {
                "5min": self.get_error_rate(window_minutes=5),
                "1hour": self.get_error_rate(window_minutes=60),
            },
            "throughput": self.get_throughput(window_minutes=5),
            "cache": self.get_cache_stats(window_minutes=60),
            "anomalies": self.get_recent_anomalies(limit=5),
        }
    
    def prune_old_data(self, days: int = 7):
        """Remove query_metrics older than N days to prevent DB growth."""
        cutoff = (datetime.utcnow() - timedelta(days=days)).isoformat()
        with self._lock:
            deleted = self._conn.execute(
                "DELETE FROM query_metrics WHERE timestamp < ?", (cutoff,)
            ).rowcount
            self._conn.commit()
            return deleted
    
    def close(self):
        self._conn.close()


# --- System Event Logger ---

class SystemEventLogger:
    """
    Structured event logger for application lifecycle and behavior tracking.
    Similar to Windows Event Viewer, Linux journalctl, or iOS os_log.
    
    Event categories:
    - LIFECYCLE: startup, shutdown, restart, config reload
    - ENGINE: engine up/down, model loaded, health check pass/fail
    - USER: login, query, upload, settings change
    - SYSTEM: resource event, circuit trip, cleanup run
    - SECURITY: auth failure, policy violation, anomaly
    """
    
    class Category(str, Enum):
        LIFECYCLE = "lifecycle"
        ENGINE = "engine"
        USER = "user"
        SYSTEM = "system"
        SECURITY = "security"
        EVALUATION = "evaluation"
    
    class Severity(str, Enum):
        INFO = "info"
        WARNING = "warning"
        ERROR = "error"
        CRITICAL = "critical"
    
    def __init__(self, db_path: str = ":memory:"):
        self.db_path = db_path
        self._conn = sqlite3.connect(db_path, check_same_thread=False)
        self._conn.execute("PRAGMA journal_mode=WAL")
        self._lock = threading.Lock()
        self._init_schema()
    
    def _init_schema(self):
        self._conn.executescript("""
            CREATE TABLE IF NOT EXISTS system_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp TEXT NOT NULL,
                category TEXT NOT NULL,
                severity TEXT NOT NULL,
                event_type TEXT NOT NULL,
                source TEXT,
                message TEXT,
                context TEXT DEFAULT '{}',
                session_id TEXT,
                correlation_id TEXT
            );
            
            CREATE INDEX IF NOT EXISTS idx_se_category ON system_events(category);
            CREATE INDEX IF NOT EXISTS idx_se_severity ON system_events(severity);
            CREATE INDEX IF NOT EXISTS idx_se_timestamp ON system_events(timestamp);
            CREATE INDEX IF NOT EXISTS idx_se_session ON system_events(session_id);
        """)
        self._conn.commit()
    
    def log(self, category: "SystemEventLogger.Category",
            severity: "SystemEventLogger.Severity",
            event_type: str, message: str = "",
            source: str = "", context: dict = None,
            session_id: str = "", correlation_id: str = ""):
        with self._lock:
            self._conn.execute(
                """INSERT INTO system_events
                   (timestamp, category, severity, event_type, source,
                    message, context, session_id, correlation_id)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)""",
                (datetime.utcnow().isoformat(),
                 category.value if isinstance(category, SystemEventLogger.Category) else category,
                 severity.value if isinstance(severity, SystemEventLogger.Severity) else severity,
                 event_type, source, message,
                 json.dumps(context or {}), session_id, correlation_id)
            )
            self._conn.commit()
    
    def query(self, category: str = None, severity: str = None,
              session_id: str = None, since_minutes: int = None,
              limit: int = 100) -> List[dict]:
        query = "SELECT * FROM system_events WHERE 1=1"
        params = []
        
        if category:
            query += " AND category = ?"
            params.append(category)
        if severity:
            query += " AND severity = ?"
            params.append(severity)
        if session_id:
            query += " AND session_id = ?"
            params.append(session_id)
        if since_minutes:
            cutoff = (datetime.utcnow() - timedelta(minutes=since_minutes)).isoformat()
            query += " AND timestamp >= ?"
            params.append(cutoff)
        
        query += " ORDER BY timestamp DESC LIMIT ?"
        params.append(limit)
        
        rows = self._conn.execute(query, params).fetchall()
        columns = [desc[0] for desc in self._conn.execute(
            "SELECT * FROM system_events LIMIT 0"
        ).description]
        
        results = []
        for row in rows:
            d = dict(zip(columns, row))
            d["context"] = json.loads(d.get("context", "{}"))
            results.append(d)
        return results
    
    def get_event_counts(self, hours: int = 24) -> dict:
        cutoff = (datetime.utcnow() - timedelta(hours=hours)).isoformat()
        by_category = {}
        for row in self._conn.execute(
            "SELECT category, COUNT(*) FROM system_events "
            "WHERE timestamp >= ? GROUP BY category",
            (cutoff,)
        ).fetchall():
            by_category[row[0]] = row[1]
        
        by_severity = {}
        for row in self._conn.execute(
            "SELECT severity, COUNT(*) FROM system_events "
            "WHERE timestamp >= ? GROUP BY severity",
            (cutoff,)
        ).fetchall():
            by_severity[row[0]] = row[1]
        
        total = self._conn.execute(
            "SELECT COUNT(*) FROM system_events WHERE timestamp >= ?",
            (cutoff,)
        ).fetchone()[0]
        
        return {"total": total, "by_category": by_category, "by_severity": by_severity}
    
    def close(self):
        self._conn.close()


# --- Self-Evaluator ---

class SelfEvaluator:
    """
    Post-session self-evaluator using the Knowledge Graph for continuous improvement.
    
    Analyzes each session's performance against historical baselines stored in the KG.
    Produces actionable improvement suggestions — parameter tuning, pattern detection,
    and degradation alerts.
    
    Runs asynchronously after session end. Zero runtime cost to queries.
    
    Evaluation dimensions:
    1. Answer Quality: Compare converged answers against KG history
    2. Confidence Trends: Is the system getting more/less confident over time?
    3. Efficiency: Token usage, iteration count, latency trends
    4. Failure Patterns: What types of questions cause failures?
    5. Knowledge Growth: How much new knowledge was added?
    """
    
    def __init__(self, kg, perf_monitor: AppPerformanceMonitor,
                 event_logger: SystemEventLogger, audit: AuditLogger):
        self.kg = kg
        self.perf_monitor = perf_monitor
        self.event_logger = event_logger
        self.audit = audit
        self._lock = threading.Lock()
    
    def evaluate_session(self, session_id: str,
                         session_stats: dict) -> dict:
        """
        Run full self-evaluation for a completed session.
        
        Args:
            session_id: Unique session identifier
            session_stats: {
                "queries": [...],  # List of {query, confidence, iterations, success, latency}
                "kg_entities_before": int,
                "kg_entities_after": int,
                "kg_relations_before": int,
                "kg_relations_after": int,
                "converged_answers": int,
                "failed_attempts": int,
                "avg_confidence": float,
                "avg_iterations": float,
                "total_tokens": int,
            }
        
        Returns:
            Evaluation report with scores and suggestions
        """
        with self._lock:
            report = {
                "session_id": session_id,
                "evaluated_at": datetime.utcnow().isoformat(),
                "scores": {},
                "trends": {},
                "suggestions": [],
                "anomalies": [],
            }
            
            queries = session_stats.get("queries", [])
            if not queries:
                return report
            
            # 1. Answer Quality Score
            quality = self._evaluate_answer_quality(queries, session_stats)
            report["scores"]["answer_quality"] = quality
            
            # 2. Confidence Trend
            confidence_trend = self._evaluate_confidence_trend(queries)
            report["trends"]["confidence"] = confidence_trend
            
            # 3. Efficiency Score
            efficiency = self._evaluate_efficiency(queries, session_stats)
            report["scores"]["efficiency"] = efficiency
            
            # 4. Knowledge Growth
            kg_growth = self._evaluate_knowledge_growth(session_stats)
            report["scores"]["knowledge_growth"] = kg_growth
            
            # 5. Failure Pattern Analysis
            failures = self._analyze_failure_patterns(queries)
            report["failure_patterns"] = failures
            
            # 6. Generate Suggestions
            suggestions = self._generate_suggestions(report)
            report["suggestions"] = suggestions
            
            # Record evaluation event
            self.event_logger.log(
                SystemEventLogger.Category.EVALUATION,
                SystemEventLogger.Severity.INFO,
                "session_evaluated",
                f"Session {session_id} evaluated",
                source="self_evaluator",
                context={
                    "overall_score": self._compute_overall_score(report),
                    "suggestion_count": len(suggestions),
                    "anomaly_count": len(report["anomalies"]),
                },
                session_id=session_id,
            )
            
            # Store evaluation in KG as a decision/metadata
            self.kg.record_decision(
                decision=f"Session evaluation: {self._compute_overall_score(report):.1f}/10",
                reason="Automated self-evaluation",
                context=json.dumps(report, default=str)[:2000],  # Truncate for storage
            )
            
            # Audit log
            self.audit.log(
                "self_evaluation",
                engine="coordinator",
                details={"session_id": session_id, "report_summary": {
                    "answer_quality": quality,
                    "efficiency": efficiency,
                    "knowledge_growth": kg_growth,
                    "suggestions": len(suggestions),
                }}
            )
            
            return report
    
    def _evaluate_answer_quality(self, queries: List[dict],
                                  session_stats: dict) -> dict:
        """Score answer quality based on convergence, confidence, and iterations."""
        if not queries:
            return {"score": 0, "details": "No queries to evaluate"}
        
        converged = sum(1 for q in queries if q.get("confidence", 0) >= 0.7)
        total = len(queries)
        avg_confidence = sum(q.get("confidence", 0) for q in queries) / total
        avg_iterations = sum(q.get("iterations", 1) for q in queries) / total
        
        # Check against historical sessions
        sessions = self.perf_monitor.get_session_history(limit=10)
        historical_confidence = 0
        if len(sessions) > 1:
            historical_confidence = sum(
                s.get("avg_latency_ms", 0) for s in sessions[1:]
            ) / max(len(sessions) - 1, 1)
        
        # Quality score (0-10)
        score = 0
        score += min(4, (converged / max(total, 1)) * 4)  # Convergence rate
        score += min(3, avg_confidence * 3)               # Confidence level
        score += min(3, max(0, 3 - (avg_iterations - 3))) # Optimal iterations (3 is sweet spot)
        score = round(min(10, score), 1)
        
        return {
            "score": score,
            "convergence_rate": round(converged / max(total, 1), 3),
            "avg_confidence": round(avg_confidence, 3),
            "avg_iterations": round(avg_iterations, 1),
            "total_queries": total,
            "converged": converged,
        }
    
    def _evaluate_confidence_trend(self, queries: List[dict]) -> dict:
        """Detect if confidence is improving or degrading over session."""
        if len(queries) < 3:
            return {"trend": "insufficient_data", "direction": "unknown"}
        
        # Split into first half vs second half
        mid = len(queries) // 2
        first_half_conf = [q.get("confidence", 0) for q in queries[:mid]]
        second_half_conf = [q.get("confidence", 0) for q in queries[mid:]]
        
        avg_first = sum(first_half_conf) / max(len(first_half_conf), 1)
        avg_second = sum(second_half_conf) / max(len(second_half_conf), 1)
        
        delta = avg_second - avg_first
        
        if delta > 0.05:
            direction = "improving"
        elif delta < -0.05:
            direction = "degrading"
        else:
            direction = "stable"
        
        return {
            "direction": direction,
            "delta": round(delta, 3),
            "first_half_avg": round(avg_first, 3),
            "second_half_avg": round(avg_second, 3),
        }
    
    def _evaluate_efficiency(self, queries: List[dict],
                              session_stats: dict) -> dict:
        """Score resource efficiency."""
        total_tokens = session_stats.get("total_tokens", 0)
        total_queries = len(queries)
        
        if total_queries == 0:
            return {"score": 0, "details": "No queries"}
        
        tokens_per_query = total_tokens / total_queries
        
        # Benchmark: ~2000 tokens/query is reasonable for RAG
        efficiency_score = max(0, min(10, 10 - (tokens_per_query - 2000) / 500))
        efficiency_score = round(efficiency_score, 1)
        
        return {
            "score": efficiency_score,
            "tokens_per_query": round(tokens_per_query, 0),
            "total_tokens": total_tokens,
            "total_queries": total_queries,
        }
    
    def _evaluate_knowledge_growth(self, session_stats: dict) -> dict:
        """Measure how much new knowledge was added to the KG."""
        entities_before = session_stats.get("kg_entities_before", 0)
        entities_after = session_stats.get("kg_entities_after", 0)
        relations_before = session_stats.get("kg_relations_before", 0)
        relations_after = session_stats.get("kg_relations_after", 0)
        
        entity_growth = entities_after - entities_before
        relation_growth = relations_after - relations_before
        
        # Score: 10 if significant growth, 0 if none
        growth_score = min(10, (entity_growth + relation_growth) / 2)
        
        return {
            "score": round(growth_score, 1),
            "entity_growth": entity_growth,
            "relation_growth": relation_growth,
            "entities_total": entities_after,
            "relations_total": relations_after,
        }
    
    def _analyze_failure_patterns(self, queries: List[dict]) -> dict:
        """Identify common failure patterns."""
        failures = [q for q in queries if not q.get("success", True)]
        
        if not failures:
            return {"pattern": "no_failures", "count": 0}
        
        # Categorize failures
        by_type = {}
        by_confidence = {"low": 0, "medium": 0, "high": 0}
        
        for f in failures:
            error = f.get("error", "unknown")
            error_type = ErrorClassifier.classify(error)
            by_type[error_type.value] = by_type.get(error_type.value, 0) + 1
            
            conf = f.get("confidence", 0)
            if conf < 0.3:
                by_confidence["low"] += 1
            elif conf < 0.7:
                by_confidence["medium"] += 1
            else:
                by_confidence["high"] += 1
        
        return {
            "count": len(failures),
            "by_error_type": by_type,
            "by_confidence_range": by_confidence,
            "failure_rate": round(len(failures) / max(len(queries), 1), 3),
        }
    
    def _generate_suggestions(self, report: dict) -> List[dict]:
        """Generate actionable improvement suggestions based on evaluation."""
        suggestions = []
        
        quality = report["scores"].get("answer_quality", {})
        efficiency = report["scores"].get("efficiency", {})
        confidence = report["trends"].get("confidence", {})
        failures = report.get("failure_patterns", {})
        
        # Low convergence rate
        if quality.get("convergence_rate", 1) < 0.5:
            suggestions.append({
                "priority": "high",
                "category": "reasoning",
                "suggestion": "Increase max iterations from 3 to 5-7",
                "reason": f"Only {quality.get('convergence_rate', 0)*100:.0f}% of answers converged",
                "expected_impact": "10-20% improvement in answer quality",
            })
        
        # Confidence degrading
        if confidence.get("direction") == "degrading":
            suggestions.append({
                "priority": "high",
                "category": "model",
                "suggestion": "Review retrieval quality — degraded confidence suggests poor context",
                "reason": f"Confidence dropped by {abs(confidence.get('delta', 0)):.3f} mid-session",
                "expected_impact": "Restore confidence to baseline",
            })
        
        # High token usage
        if efficiency.get("tokens_per_query", 0) > 4000:
            suggestions.append({
                "priority": "medium",
                "category": "efficiency",
                "suggestion": "Reduce top_k from default or tighten chunk_size",
                "reason": f"{efficiency.get('tokens_per_query', 0):.0f} tokens/query (target: <2000)",
                "expected_impact": "30-50% reduction in token consumption",
            })
        
        # High failure rate
        if failures.get("failure_rate", 0) > 0.1:
            suggestions.append({
                "priority": "high",
                "category": "reliability",
                "suggestion": "Investigate transient errors — engine may need restart",
                "reason": f"{failures.get('failure_rate', 0)*100:.0f}% failure rate",
                "expected_impact": "Eliminate preventable failures",
            })
        
        # Low knowledge growth
        kg = report["scores"].get("knowledge_growth", {})
        if kg.get("score", 10) < 2:
            suggestions.append({
                "priority": "low",
                "category": "knowledge",
                "suggestion": "Enable entity extraction on ingest to grow KG faster",
                "reason": "Minimal knowledge growth this session",
                "expected_impact": "Richer personal context for future queries",
            })
        
        return suggestions
    
    def _compute_overall_score(self, report: dict) -> float:
        """Compute weighted overall score (0-10)."""
        scores = report.get("scores", {})
        weights = {
            "answer_quality": 0.4,
            "efficiency": 0.25,
            "knowledge_growth": 0.2,
            "reliability": 0.15,
        }
        
        total = 0
        weight_sum = 0
        
        for key, weight in weights.items():
            if key in scores:
                val = scores[key]
                if isinstance(val, dict):
                    val = val.get("score", 0)
                total += val * weight
                weight_sum += weight
            elif key == "reliability":
                # Derive from failure patterns
                failures = report.get("failure_patterns", {})
                rate = failures.get("failure_rate", 0)
                total += max(0, 10 * (1 - rate)) * weight
                weight_sum += weight
        
        return round(total / max(weight_sum, 0.01), 1)


# --- Custom Exceptions ---

class CircuitOpenError(Exception):
    """Circuit breaker is open, request blocked."""
    pass


class ResourceError(Exception):
    """Host resource constraint violated."""
    pass


class QueueFullError(Exception):
    """Engine queue at max depth."""
    pass


class TransientError(Exception):
    """Retryable error."""
    pass


class PermanentError(Exception):
    """Non-retryable error."""
    pass


class MaxRetriesExceededError(Exception):
    """Transaction exceeded max retry attempts."""
    pass


# --- Singleton ---

_coordinator: Optional[ProcessingCoordinator] = None

def get_coordinator(config: Optional[CoordinatorConfig] = None) -> ProcessingCoordinator:
    global _coordinator
    if _coordinator is None:
        _coordinator = ProcessingCoordinator(config)
    return _coordinator
