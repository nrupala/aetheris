"""
Pregel Checkpointing — Deterministic execution with crash recovery.

Inspired by LangGraph's checkpointing pattern:
- Save state to disk after each iteration
- Resume from last checkpoint on crash
- Support branching (compare multiple reasoning paths)

Storage: JSON files in /workspace/processing/{task_id}/checkpoints/
"""

import os
import json
import hashlib
import time
import threading
from typing import Dict, List, Optional, Any
from dataclasses import dataclass, field, asdict
from datetime import datetime


@dataclass
class CheckpointState:
    """State captured at a checkpoint."""
    iteration: int
    answer: str
    confidence: float
    temperature: float
    reasoning: str
    verification: Dict[str, Any] = field(default_factory=dict)
    sources_used: List[str] = field(default_factory=list)
    tokens_used: int = 0
    timestamp: str = ""
    error: Optional[str] = None


@dataclass
class Checkpoint:
    """A checkpoint record with metadata."""
    checkpoint_id: str
    task_id: str
    state: CheckpointState
    parent_id: Optional[str] = None
    branch: Optional[str] = None
    is_final: bool = False
    created_at: str = ""


class PregelCheckpoint:
    """
    Manages Pregel-style checkpoints for reasoning loop execution.
    
    Usage:
        cp = PregelCheckpoint("/workspace/processing/task-123")
        
        # Save checkpoint after each iteration
        cp.save(state, parent_id=None)
        
        # Resume from last checkpoint on crash
        last = cp.get_latest()
        if last:
            resume_from = last.state.iteration + 1
    
    Guarantees:
    - Atomic writes (write to .tmp, then rename)
    - Crash recovery (partial writes are ignored)
    - Branching (multiple reasoning paths from same point)
    """
    
    def __init__(self, task_dir: str):
        self.task_dir = task_dir
        self.checkpoint_dir = os.path.join(task_dir, "checkpoints")
        os.makedirs(self.checkpoint_dir, exist_ok=True)
        self._lock = threading.Lock()
    
    def save(
        self,
        state: CheckpointState,
        task_id: str,
        parent_id: Optional[str] = None,
        branch: Optional[str] = None,
        is_final: bool = False,
    ) -> Checkpoint:
        """
        Save a checkpoint atomically.
        
        Returns the checkpoint record.
        """
        with self._lock:
            now = datetime.utcnow().isoformat()
            checkpoint_id = f"cp_{now.replace(':', '-').replace('.', '-')}_{state.iteration}"
            if branch:
                checkpoint_id += f"_{branch}"
            
            checkpoint = Checkpoint(
                checkpoint_id=checkpoint_id,
                task_id=task_id,
                state=state,
                parent_id=parent_id,
                branch=branch,
                is_final=is_final,
                created_at=now,
            )
            
            # Atomic write: write to .tmp, then rename
            filepath = os.path.join(self.checkpoint_dir, f"{checkpoint_id}.json")
            tmp_path = filepath + ".tmp"
            
            with open(tmp_path, "w") as f:
                json.dump(self._checkpoint_to_dict(checkpoint), f, indent=2)
                f.flush()
                os.fsync(f.fileno())
            
            os.rename(tmp_path, filepath)
            
            return checkpoint
    
    def get_latest(self, branch: Optional[str] = None) -> Optional[Checkpoint]:
        """
        Get the most recent checkpoint.
        If branch specified, get latest from that branch.
        """
        checkpoints = self.list_checkpoints(branch=branch)
        if not checkpoints:
            return None
        return checkpoints[-1]
    
    def list_checkpoints(self, branch: Optional[str] = None) -> List[Checkpoint]:
        """
        List all checkpoints, ordered by iteration.
        Filter by branch if specified.
        """
        checkpoints = []
        
        for filename in sorted(os.listdir(self.checkpoint_dir)):
            if not filename.endswith(".json"):
                continue
            if branch and f"_{branch}.json" not in filename and not filename.endswith(f"_{branch}.json"):
                continue
            
            filepath = os.path.join(self.checkpoint_dir, filename)
            try:
                with open(filepath, "r") as f:
                    data = json.load(f)
                checkpoints.append(self._dict_to_checkpoint(data))
            except (json.JSONDecodeError, KeyError, FileNotFoundError):
                # Corrupted or partial write — skip
                continue
        
        checkpoints.sort(key=lambda c: (c.state.iteration, c.created_at))
        return checkpoints
    
    def get_checkpoint(self, checkpoint_id: str) -> Optional[Checkpoint]:
        """Get a specific checkpoint by ID."""
        filepath = os.path.join(self.checkpoint_dir, f"{checkpoint_id}.json")
        if not os.path.exists(filepath):
            return None
        
        try:
            with open(filepath, "r") as f:
                data = json.load(f)
            return self._dict_to_checkpoint(data)
        except (json.JSONDecodeError, KeyError):
            return None
    
    def get_branches(self) -> List[str]:
        """Get all branch names."""
        branches = set()
        for cp in self.list_checkpoints():
            if cp.branch:
                branches.add(cp.branch)
        return sorted(branches)
    
    def get_branch_history(self, branch: str) -> List[Checkpoint]:
        """Get full checkpoint history for a branch."""
        return [cp for cp in self.list_checkpoints() if cp.branch == branch]
    
    def get_final(self) -> Optional[Checkpoint]:
        """Get the final (converged) checkpoint."""
        for cp in self.list_checkpoints():
            if cp.is_final:
                return cp
        return None
    
    def delete_branch(self, branch: str) -> int:
        """Delete all checkpoints for a branch."""
        deleted = 0
        for cp in self.list_checkpoints(branch=branch):
            filepath = os.path.join(self.checkpoint_dir, f"{cp.checkpoint_id}.json")
            if os.path.exists(filepath):
                os.remove(filepath)
                deleted += 1
        return deleted
    
    def cleanup_old(self, keep_last: int = 10) -> int:
        """Delete old checkpoints, keeping only the last N."""
        all_cps = self.list_checkpoints()
        if len(all_cps) <= keep_last:
            return 0
        
        to_delete = all_cps[:-keep_last]
        deleted = 0
        for cp in to_delete:
            if cp.is_final:
                continue  # Never delete final checkpoints
            filepath = os.path.join(self.checkpoint_dir, f"{cp.checkpoint_id}.json")
            if os.path.exists(filepath):
                os.remove(filepath)
                deleted += 1
        return deleted
    
    def get_state_summary(self) -> Dict:
        """Get summary of checkpoint state."""
        all_cps = self.list_checkpoints()
        if not all_cps:
            return {"total": 0}
        
        branches = set()
        final_exists = False
        for cp in all_cps:
            if cp.branch:
                branches.add(cp.branch)
            if cp.is_final:
                final_exists = True
        
        return {
            "total": len(all_cps),
            "branches": list(branches),
            "has_final": final_exists,
            "latest_iteration": all_cps[-1].state.iteration,
            "latest_confidence": all_cps[-1].state.confidence,
            "task_id": all_cps[0].task_id,
        }
    
    def _checkpoint_to_dict(self, cp: Checkpoint) -> dict:
        return {
            "checkpoint_id": cp.checkpoint_id,
            "task_id": cp.task_id,
            "parent_id": cp.parent_id,
            "branch": cp.branch,
            "is_final": cp.is_final,
            "created_at": cp.created_at,
            "state": {
                "iteration": cp.state.iteration,
                "answer": cp.state.answer,
                "confidence": cp.state.confidence,
                "temperature": cp.state.temperature,
                "reasoning": cp.state.reasoning,
                "verification": cp.state.verification,
                "sources_used": cp.state.sources_used,
                "tokens_used": cp.state.tokens_used,
                "timestamp": cp.state.timestamp,
                "error": cp.state.error,
            },
        }
    
    def _dict_to_checkpoint(self, data: dict) -> Checkpoint:
        state_data = data["state"]
        state = CheckpointState(
            iteration=state_data["iteration"],
            answer=state_data["answer"],
            confidence=state_data["confidence"],
            temperature=state_data["temperature"],
            reasoning=state_data.get("reasoning", ""),
            verification=state_data.get("verification", {}),
            sources_used=state_data.get("sources_used", []),
            tokens_used=state_data.get("tokens_used", 0),
            timestamp=state_data.get("timestamp", ""),
            error=state_data.get("error"),
        )
        return Checkpoint(
            checkpoint_id=data["checkpoint_id"],
            task_id=data["task_id"],
            state=state,
            parent_id=data.get("parent_id"),
            branch=data.get("branch"),
            is_final=data.get("is_final", False),
            created_at=data.get("created_at", ""),
        )


# --- Global Checkpoint Manager ---

class CheckpointManager:
    """
    Global manager for all Pregel checkpoints across tasks.
    Handles cleanup, recovery, and cross-task queries.
    """
    
    def __init__(self, workspace_root: str):
        self.workspace_root = workspace_root
        self.processing_dir = os.path.join(workspace_root, "processing")
    
    def get_checkpoint_for_task(self, task_id: str) -> PregelCheckpoint:
        """Get or create checkpoint store for a task."""
        task_dir = os.path.join(self.processing_dir, task_id)
        return PregelCheckpoint(task_dir)
    
    def find_incomplete_tasks(self) -> List[str]:
        """Find tasks with checkpoints but no final answer."""
        if not os.path.exists(self.processing_dir):
            return []
        
        incomplete = []
        for task_id in os.listdir(self.processing_dir):
            task_dir = os.path.join(self.processing_dir, task_id)
            if not os.path.isdir(task_dir):
                continue
            
            cp = PregelCheckpoint(task_dir)
            summary = cp.get_state_summary()
            if summary["total"] > 0 and not summary["has_final"]:
                incomplete.append(task_id)
        
        return incomplete
    
    def resume_incomplete_tasks(self) -> Dict[str, Checkpoint]:
        """Get latest checkpoint for each incomplete task."""
        results = {}
        for task_id in self.find_incomplete_tasks():
            cp = self.get_checkpoint_for_task(task_id)
            latest = cp.get_latest()
            if latest:
                results[task_id] = latest
        return results
