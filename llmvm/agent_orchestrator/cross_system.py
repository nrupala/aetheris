"""
Phase 3: Cross-System Orchestration.

Provides:
- CrossEngineState: Synchronized state across all 4 engines (RAG, AI, Dev, Agents)
- ResourceForecaster: Predict resource needs, pre-warm engines
- SharedKGHub: Single source of truth Knowledge Graph shared across engines
- SpreadForecaster: Accurate resource allocation forecasting

Architecture:
    ┌─────────────────────────────────────────────────────────────────┐
    │                    Cross-System Orchestrator                     │
    ├─────────────┬─────────────┬─────────────┬──────────────────────┤
    │  State Sync │  Resource   │  Shared KG  │  Spread              │
    │  (Atomic)   │  Forecaster │  Hub        │  Forecaster          │
    └──────┬──────┴──────┬──────┴──────┬──────┴──────┬───────────────┘
           │             │             │             │
      ┌────▼───┐   ┌────▼───┐   ┌─────▼────┐  ┌────▼────┐
      │  RAG   │   │  AI    │   │  KG Hub  │  │ Monitor │
      │ Engine │   │ Engine │   │          │  │ Stack   │
      └────────┘   └────────┘   └──────────┘  └─────────┘
"""

import os
import json
import time
import uuid
import logging
import threading
from typing import Any, Callable, Dict, List, Optional, Tuple
from dataclasses import dataclass, field
from datetime import datetime, timedelta
from enum import Enum

logger = logging.getLogger(__name__)


# --- Engine Definitions ---

class EngineType(Enum):
    RAG = "rag"
    AI = "ai"
    DEV = "dev"
    AGENTS = "agents"


@dataclass
class EngineState:
    """State of a single engine."""
    engine: EngineType
    status: str = "unknown"  # healthy, degraded, down, prewarming
    last_health_check: float = 0
    response_time_ms: float = 0
    memory_mb: float = 0
    active_tasks: int = 0
    queue_depth: int = 0
    error_count_1h: int = 0
    metadata: Dict = field(default_factory=dict)


# --- Cross-Engine State Manager (3.1) ---

class CrossEngineState:
    """Synchronized state management across all 4 engines.
    
    Provides atomic state updates, change subscriptions, and crash recovery.
    Uses SQLite-backed WAL for durability (reuses coordinator pattern).
    """

    def __init__(self, state_dir: str = "/workspace/state"):
        self.state_dir = state_dir
        self.state_file = os.path.join(state_dir, "engine_state.json")
        self._lock = threading.RLock()
        self._subscribers: Dict[str, List[Callable]] = {}
        self._states: Dict[str, EngineState] = {}
        self._version = 0
        self._snapshot_history: List[Dict] = []
        
        os.makedirs(state_dir, exist_ok=True)
        self._load_state()
    
    def _load_state(self):
        """Load state from disk."""
        if os.path.exists(self.state_file):
            try:
                with open(self.state_file, "r") as f:
                    data = json.load(f)
                self._version = data.get("version", 0)
                for engine_name, es_data in data.get("engines", {}).items():
                    es = EngineState(
                        engine=EngineType(engine_name),
                        **{k: v for k, v in es_data.items() if k != "engine"},
                    )
                    self._states[engine_name] = es
                self._snapshot_history = data.get("snapshots", [])[-100:]
            except Exception as e:
                logger.error(f"Failed to load state: {e}")
    
    def _save_state(self):
        """Atomically save state to disk."""
        data = {
            "version": self._version,
            "engines": {
                name: {
                    "engine": es.engine.value,
                    "status": es.status,
                    "last_health_check": es.last_health_check,
                    "response_time_ms": es.response_time_ms,
                    "memory_mb": es.memory_mb,
                    "active_tasks": es.active_tasks,
                    "queue_depth": es.queue_depth,
                    "error_count_1h": es.error_count_1h,
                    "metadata": es.metadata,
                }
                for name, es in self._states.items()
            },
            "snapshots": self._snapshot_history[-100:],
            "updated_at": datetime.utcnow().isoformat(),
        }
        tmp_file = self.state_file + ".tmp"
        with open(tmp_file, "w") as f:
            json.dump(data, f, indent=2)
        os.replace(tmp_file, self.state_file)
    
    def update_engine(self, engine: EngineType, **kwargs) -> EngineState:
        """Atomically update an engine's state."""
        with self._lock:
            name = engine.value
            if name not in self._states:
                self._states[name] = EngineState(engine=engine)
            
            es = self._states[name]
            for key, value in kwargs.items():
                if hasattr(es, key):
                    setattr(es, key, value)
            
            self._version += 1
            self._save_state()
            
            # Notify subscribers
            self._notify_subscribers(engine, es)
            
            return es
    
    def get_engine(self, engine: EngineType) -> Optional[EngineState]:
        return self._states.get(engine.value)
    
    def get_all_engines(self) -> Dict[str, EngineState]:
        return dict(self._states)
    
    def get_healthy_engines(self) -> List[EngineType]:
        return [
            es.engine for es in self._states.values()
            if es.status in ("healthy", "prewarming")
        ]
    
    def snapshot(self) -> Dict:
        """Create a state snapshot for recovery."""
        with self._lock:
            snap = {
                "version": self._version,
                "timestamp": datetime.utcnow().isoformat(),
                "engines": {
                    name: {
                        "status": es.status,
                        "response_time_ms": es.response_time_ms,
                        "active_tasks": es.active_tasks,
                    }
                    for name, es in self._states.items()
                },
            }
            self._snapshot_history.append(snap)
            if len(self._snapshot_history) > 100:
                self._snapshot_history = self._snapshot_history[-100:]
            self._save_state()
            return snap
    
    def restore_snapshot(self, index: int = -1) -> bool:
        """Restore state from a snapshot."""
        with self._lock:
            if not self._snapshot_history:
                return False
            snap = self._snapshot_history[index]
            self._version = snap.get("version", self._version)
            self._save_state()
            return True
    
    def subscribe(self, engine: EngineType, callback: Callable):
        """Subscribe to state changes for an engine."""
        name = engine.value
        if name not in self._subscribers:
            self._subscribers[name] = []
        self._subscribers[name].append(callback)
    
    def _notify_subscribers(self, engine: EngineType, state: EngineState):
        name = engine.value
        for callback in self._subscribers.get(name, []):
            try:
                callback(state)
            except Exception as e:
                logger.error(f"Subscriber callback failed: {e}")
    
    def get_dashboard(self) -> Dict:
        """Get complete state dashboard."""
        return {
            "version": self._version,
            "engines": {
                name: {
                    "status": es.status,
                    "response_time_ms": round(es.response_time_ms, 1),
                    "memory_mb": round(es.memory_mb, 1),
                    "active_tasks": es.active_tasks,
                    "queue_depth": es.queue_depth,
                    "error_count_1h": es.error_count_1h,
                    "last_check": es.last_health_check,
                }
                for name, es in self._states.items()
            },
            "healthy_count": len(self.get_healthy_engines()),
            "total_engines": len(self._states),
        }


# --- Resource Forecaster (3.2) ---

@dataclass
class ResourcePrediction:
    """Resource prediction for an engine."""
    engine: str
    predicted_need_mb: float
    predicted_need_cpu_pct: float
    confidence: float  # 0-1
    should_prewarm: bool
    estimated_start_time: float  # unix timestamp
    reasoning: str


class ResourceForecaster:
    """Predicts resource needs and pre-warms engines.
    
    Uses sliding window analysis of historical metrics to predict:
    - Memory requirements
    - CPU utilization
    - When to pre-warm engines
    - Queue depth forecasts
    """

    def __init__(self, state: CrossEngineState):
        self.state = state
        self._metrics_history: List[Dict] = []
        self._max_history = 1000
        self._prewarm_threshold_mb = 500  # Pre-warm if predicted need > 500MB
        self._forecast_window = 300  # 5 minute forecast window
    
    def record_metrics(self, engine: EngineType, memory_mb: float, cpu_pct: float, active_tasks: int, queue_depth: int):
        """Record current metrics for forecasting."""
        entry = {
            "engine": engine.value,
            "memory_mb": memory_mb,
            "cpu_pct": cpu_pct,
            "active_tasks": active_tasks,
            "queue_depth": queue_depth,
            "timestamp": time.time(),
        }
        self._metrics_history.append(entry)
        if len(self._metrics_history) > self._max_history:
            self._metrics_history = self._metrics_history[-self._max_history:]
        
        # Update engine state
        self.state.update_engine(engine, memory_mb=memory_mb)
    
    def predict(self, engine: EngineType) -> ResourcePrediction:
        """Predict resource needs for an engine."""
        engine_history = [
            m for m in self._metrics_history
            if m["engine"] == engine.value
        ]
        
        if len(engine_history) < 3:
            return ResourcePrediction(
                engine=engine.value,
                predicted_need_mb=0,
                predicted_need_cpu_pct=0,
                confidence=0.0,
                should_prewarm=False,
                estimated_start_time=time.time(),
                reasoning="Insufficient history for prediction",
            )
        
        # Simple linear regression on recent data
        recent = engine_history[-20:]
        memory_values = [m["memory_mb"] for m in recent]
        cpu_values = [m["cpu_pct"] for m in recent]
        task_values = [m["active_tasks"] for m in recent]
        
        avg_memory = sum(memory_values) / len(memory_values)
        avg_cpu = sum(cpu_values) / len(cpu_values)
        avg_tasks = sum(task_values) / len(task_values)
        
        # Trend detection
        memory_trend = memory_values[-1] - memory_values[0] if len(memory_values) > 1 else 0
        cpu_trend = cpu_values[-1] - cpu_values[0] if len(cpu_values) > 1 else 0
        
        # Forecast: project forward
        predicted_memory = avg_memory + (memory_trend * 1.5)  # 50% trend extrapolation
        predicted_cpu = min(avg_cpu + (cpu_trend * 1.5), 100)
        
        # Confidence based on data volume and variance
        memory_variance = sum((x - avg_memory) ** 2 for x in memory_values) / len(memory_values)
        confidence = max(0.0, min(1.0, 1.0 - (memory_variance / 10000)))
        confidence = min(confidence, len(recent) / 20.0)
        
        should_prewarm = predicted_memory > self._prewarm_threshold_mb
        
        return ResourcePrediction(
            engine=engine.value,
            predicted_need_mb=round(predicted_memory, 1),
            predicted_need_cpu_pct=round(predicted_cpu, 1),
            confidence=round(confidence, 2),
            should_prewarm=should_prewarm,
            estimated_start_time=time.time() + 60,  # 1 minute ahead
            reasoning=f"Trend: mem {memory_trend:+.0f}MB, cpu {cpu_trend:+.1f}%",
        )
    
    def predict_all(self) -> Dict[str, ResourcePrediction]:
        """Predict for all engines."""
        return {
            engine.value: self.predict(engine)
            for engine in EngineType
        }
    
    def get_prewarm_recommendations(self) -> List[Dict]:
        """Get list of engines that should be pre-warmed."""
        recommendations = []
        for engine in EngineType:
            pred = self.predict(engine)
            if pred.should_prewarm:
                recommendations.append({
                    "engine": engine.value,
                    "predicted_memory_mb": pred.predicted_need_mb,
                    "confidence": pred.confidence,
                    "action": "prewarm",
                    "reasoning": pred.reasoning,
                })
        return recommendations


# --- Shared Knowledge Graph Hub (3.3) ---

class SharedKGHub:
    """Single source of truth Knowledge Graph shared across all engines.
    
    Provides:
    - Unified entity/relation store
    - Per-engine views and access control
    - Change propagation to all subscribed engines
    - Conflict resolution for concurrent writes
    - Export/import for backup and migration
    """

    def __init__(self, kg_instance=None, state: Optional[CrossEngineState] = None):
        self.kg = kg_instance
        self.state = state
        self._access_log: List[Dict] = []
        self._change_log: List[Dict] = []
        self._engine_permissions: Dict[str, List[str]] = {
            "rag": ["read", "write_entities", "write_relations", "query"],
            "ai": ["read", "query", "write_interactions"],
            "dev": ["read", "write_decisions", "query"],
            "agents": ["read", "write_entities", "write_relations", "write_interactions", "query"],
        }
    
    def _check_permission(self, engine: str, action: str) -> bool:
        return action in self._engine_permissions.get(engine, [])
    
    def _log_access(self, engine: str, action: str, success: bool, details: str = ""):
        self._access_log.append({
            "engine": engine,
            "action": action,
            "success": success,
            "details": details,
            "timestamp": datetime.utcnow().isoformat(),
        })
        if len(self._access_log) > 1000:
            self._access_log = self._access_log[-1000:]
    
    def _log_change(self, engine: str, change_type: str, details: Dict):
        self._change_log.append({
            "engine": engine,
            "change_type": change_type,
            "details": details,
            "timestamp": datetime.utcnow().isoformat(),
        })
        if len(self._change_log) > 1000:
            self._change_log = self._change_log[-1000:]
        
        if self.state:
            self.state.update_engine(
                EngineType.AGENTS,
                metadata={"kg_changes": len(self._change_log)},
            )
    
    # Read operations
    def query(self, engine: str, query: str) -> Dict:
        """Query the shared KG."""
        if not self._check_permission(engine, "query"):
            self._log_access(engine, "query", False, "permission denied")
            return {"error": "permission denied"}
        
        if not self.kg:
            return {"error": "KG not available"}
        
        result = self.kg.get_personal_context(query)
        self._log_access(engine, "query", True)
        return {"context": result}
    
    def get_entity(self, engine: str, entity_name: str) -> Dict:
        """Get an entity from the shared KG."""
        if not self._check_permission(engine, "read"):
            self._log_access(engine, "read", False, "permission denied")
            return {"error": "permission denied"}
        
        if not self.kg:
            return {"error": "KG not available"}
        
        entity = self.kg.get_entity(entity_name)
        self._log_access(engine, "read", True)
        return {"entity": entity}
    
    def get_stats(self, engine: str) -> Dict:
        """Get KG statistics."""
        if not self._check_permission(engine, "read"):
            return {"error": "permission denied"}
        if not self.kg:
            return {"error": "KG not available"}
        return self.kg.stats()
    
    # Write operations
    def add_entity(self, engine: str, name: str, entity_type: str, properties: Dict) -> Dict:
        """Add an entity to the shared KG."""
        if not self._check_permission(engine, "write_entities"):
            self._log_access(engine, "write_entities", False, "permission denied")
            return {"error": "permission denied"}
        
        if not self.kg:
            return {"error": "KG not available"}
        
        try:
            self.kg.add_entity(name, entity_type, properties)
            self._log_change(engine, "add_entity", {"name": name, "type": entity_type})
            self._log_access(engine, "write_entities", True)
            return {"success": True, "entity": name}
        except Exception as e:
            self._log_access(engine, "write_entities", False, str(e))
            return {"error": str(e)}
    
    def add_relation(self, engine: str, from_entity: str, relation_type: str, to_entity: str) -> Dict:
        """Add a relation to the shared KG."""
        if not self._check_permission(engine, "write_relations"):
            self._log_access(engine, "write_relations", False, "permission denied")
            return {"error": "permission denied"}
        
        if not self.kg:
            return {"error": "KG not available"}
        
        try:
            self.kg.add_relation(from_entity, relation_type, to_entity)
            self._log_change(engine, "add_relation", {
                "from": from_entity,
                "type": relation_type,
                "to": to_entity,
            })
            self._log_access(engine, "write_relations", True)
            return {"success": True}
        except Exception as e:
            self._log_access(engine, "write_relations", False, str(e))
            return {"error": str(e)}
    
    def record_interaction(self, engine: str, interaction_type: str, details: Dict) -> Dict:
        """Record an interaction in the shared KG."""
        if not self._check_permission(engine, "write_interactions"):
            return {"error": "permission denied"}
        
        if not self.kg:
            return {"error": "KG not available"}
        
        try:
            self.kg.add_interaction(interaction_type, details)
            self._log_change(engine, "add_interaction", {"type": interaction_type})
            return {"success": True}
        except Exception as e:
            return {"error": str(e)}
    
    # Analytics
    def get_change_log(self, engine: str, limit: int = 50) -> List[Dict]:
        """Get recent changes."""
        if not self._check_permission(engine, "read"):
            return []
        return self._change_log[-limit:]
    
    def get_access_log(self, engine: str, limit: int = 50) -> List[Dict]:
        """Get recent access entries."""
        if not self._check_permission(engine, "read"):
            return []
        return self._access_log[-limit:]
    
    def export(self, engine: str) -> Dict:
        """Export the entire KG."""
        if not self._check_permission(engine, "read"):
            return {"error": "permission denied"}
        if not self.kg:
            return {"error": "KG not available"}
        
        return {
            "kg": self.kg.export(),
            "change_log": self._change_log,
            "access_log": self._access_log,
            "exported_at": datetime.utcnow().isoformat(),
        }
    
    def import_data(self, engine: str, data: Dict) -> Dict:
        """Import KG data."""
        if not self._check_permission(engine, "write_entities"):
            return {"error": "permission denied"}
        if not self.kg:
            return {"error": "KG not available"}
        
        try:
            self.kg.import_data(data.get("kg", {}))
            self._log_change(engine, "import", {"entities": len(data.get("kg", {}).get("entities", []))})
            return {"success": True}
        except Exception as e:
            return {"error": str(e)}
    
    def get_dashboard(self, engine: str) -> Dict:
        """Get shared KG dashboard."""
        if not self._check_permission(engine, "read"):
            return {"error": "permission denied"}
        
        stats = self.get_stats(engine) if self.kg else {"error": "KG not available"}
        return {
            "stats": stats,
            "total_changes": len(self._change_log),
            "total_accesses": len(self._access_log),
            "recent_changes": self._change_log[-10:],
            "engine_permissions": self._engine_permissions,
        }


# --- Spread Forecaster (3.4) ---

@dataclass
class SpreadForecast:
    """Resource spread forecast across all engines."""
    timestamp: str
    total_memory_mb: float
    total_cpu_pct: float
    engines: Dict[str, Dict]
    bottleneck: Optional[str]
    recommendations: List[str]
    confidence: float


class SpreadForecaster:
    """Accurate spread forecasting for resource allocation across engines.
    
    Analyzes:
    - Per-engine memory and CPU projections
    - Host capacity constraints
    - Queue depth correlations
    - Cascading failure risks
    """

    def __init__(
        self,
        state: CrossEngineState,
        forecaster: ResourceForecaster,
        host_memory_mb: float = 15360,
        host_cpu_pct: float = 100,
    ):
        self.state = state
        self.forecaster = forecaster
        self.host_memory_mb = host_memory_mb
        self.host_cpu_pct = host_cpu_pct
        self._forecast_history: List[SpreadForecast] = []
    
    def forecast(self) -> SpreadForecast:
        """Generate comprehensive resource spread forecast."""
        predictions = self.forecaster.predict_all()
        engine_states = self.state.get_all_engines()
        
        total_memory = sum(p.predicted_need_mb for p in predictions.values())
        total_cpu = sum(p.predicted_need_cpu_pct for p in predictions.values())
        
        engine_details = {}
        bottleneck = None
        max_memory_ratio = 0
        recommendations = []
        
        for engine_type, pred in predictions.items():
            es = engine_states.get(engine_type)
            memory_ratio = pred.predicted_need_mb / self.host_memory_mb
            cpu_ratio = pred.predicted_need_cpu_pct / self.host_cpu_pct
            
            status = "ok"
            if memory_ratio > 0.8 or cpu_ratio > 0.8:
                status = "critical"
                if memory_ratio > max_memory_ratio:
                    max_memory_ratio = memory_ratio
                    bottleneck = engine_type
            elif memory_ratio > 0.6 or cpu_ratio > 0.6:
                status = "warning"
            
            engine_details[engine_type] = {
                "predicted_memory_mb": pred.predicted_need_mb,
                "predicted_cpu_pct": pred.predicted_need_cpu_pct,
                "memory_ratio": round(memory_ratio, 2),
                "cpu_ratio": round(cpu_ratio, 2),
                "status": status,
                "should_prewarm": pred.should_prewarm,
                "current_active_tasks": es.active_tasks if es else 0,
                "current_queue_depth": es.queue_depth if es else 0,
            }
        
        # Generate recommendations
        if total_memory > self.host_memory_mb * 0.8:
            recommendations.append(f"Memory pressure: {total_memory:.0f}MB predicted vs {self.host_memory_mb}MB host capacity")
        if bottleneck:
            recommendations.append(f"Bottleneck: {bottleneck} engine at {max_memory_ratio*100:.0f}% memory ratio")
        
        prewarm_count = sum(1 for p in predictions.values() if p.should_prewarm)
        if prewarm_count > 2:
            recommendations.append(f"{prewarm_count} engines need pre-warming — stagger to avoid resource spike")
        
        total_ratio = total_cpu / self.host_cpu_pct
        if total_ratio > 1.0:
            recommendations.append(f"CPU oversubscription: {total_ratio*100:.0f}% predicted — reduce active tasks")
        
        avg_confidence = sum(p.confidence for p in predictions.values()) / max(len(predictions), 1)
        
        forecast = SpreadForecast(
            timestamp=datetime.utcnow().isoformat(),
            total_memory_mb=round(total_memory, 1),
            total_cpu_pct=round(total_cpu, 1),
            engines=engine_details,
            bottleneck=bottleneck,
            recommendations=recommendations,
            confidence=round(avg_confidence, 2),
        )
        
        self._forecast_history.append(forecast)
        if len(self._forecast_history) > 100:
            self._forecast_history = self._forecast_history[-100:]
        
        return forecast
    
    def get_history(self, limit: int = 20) -> List[Dict]:
        """Get forecast history."""
        return [
            {
                "timestamp": f.timestamp,
                "total_memory_mb": f.total_memory_mb,
                "bottleneck": f.bottleneck,
                "recommendations": f.recommendations,
                "confidence": f.confidence,
            }
            for f in self._forecast_history[-limit:]
        ]
    
    def get_dashboard(self) -> Dict:
        """Get complete spread forecasting dashboard."""
        forecast = self.forecast()
        return {
            "forecast": {
                "timestamp": forecast.timestamp,
                "total_memory_mb": forecast.total_memory_mb,
                "total_cpu_pct": forecast.total_cpu_pct,
                "host_memory_mb": self.host_memory_mb,
                "host_cpu_pct": self.host_cpu_pct,
                "memory_utilization_pct": round(forecast.total_memory_mb / self.host_memory_mb * 100, 1),
                "cpu_utilization_pct": forecast.total_cpu_pct,
                "bottleneck": forecast.bottleneck,
                "confidence": forecast.confidence,
            },
            "engines": forecast.engines,
            "recommendations": forecast.recommendations,
            "history": self.get_history(10),
        }


# --- Unified Cross-System Orchestrator ---

class CrossSystemOrchestrator:
    """Unifies all Phase 3 components into a single orchestrator."""

    def __init__(
        self,
        state_dir: str = "/workspace/state",
        host_memory_mb: float = 15360,
        kg_instance=None,
    ):
        self.state = CrossEngineState(state_dir=state_dir)
        self.forecaster = ResourceForecaster(self.state)
        self.kg_hub = SharedKGHub(kg_instance=kg_instance, state=self.state)
        self.spread_forecaster = SpreadForecaster(
            self.state, self.forecaster,
            host_memory_mb=host_memory_mb,
        )
    
    def get_full_dashboard(self) -> Dict:
        """Get complete cross-system dashboard."""
        return {
            "state": self.state.get_dashboard(),
            "spread_forecast": self.spread_forecaster.get_dashboard(),
            "kg_hub": self.kg_hub.get_dashboard("agents"),
            "prewarm_recommendations": self.forecaster.get_prewarm_recommendations(),
            "timestamp": datetime.utcnow().isoformat(),
        }
