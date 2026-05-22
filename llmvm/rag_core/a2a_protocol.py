"""
A2A (Agent-to-Agent) Message Protocol — Lightweight inter-engine communication.

Protocol:
    RAG (Researcher) ↔ AI (Generator) ↔ Dev (Executor)
    Coordinator routes messages with OPA policy gate

Message format:
{
    "message_id": "uuid",
    "conversation_id": "uuid",
    "from_engine": "rag",
    "to_engine": "ai",
    "type": "query|response|error|status",
    "payload": {...},
    "timestamp": "iso8601",
    "ttl_seconds": 3600,
}

Storage: /workspace/intermediate/{conversation_id}/
OPA Gate: Every message checked against policy before delivery.
"""

import os
import json
import uuid
import time
import threading
import logging
from typing import Dict, List, Optional, Callable, Any
from dataclasses import dataclass, field, asdict
from datetime import datetime, timedelta
from enum import Enum

logger = logging.getLogger(__name__)


# --- Message Types ---

class EngineType(str, Enum):
    RAG = "rag"
    AI = "ai"
    DEV = "dev"
    COORDINATOR = "coordinator"


class MessageType(str, Enum):
    QUERY = "query"           # Request to another engine
    RESPONSE = "response"     # Answer back
    ERROR = "error"           # Error notification
    STATUS = "status"         # Status update
    CONTEXT = "context"       # Context sharing
    COMMAND = "command"       # Instruction


class MessagePriority(str, Enum):
    LOW = "low"
    NORMAL = "normal"
    HIGH = "high"
    CRITICAL = "critical"


@dataclass
class A2AMessage:
    """Agent-to-Agent message."""
    message_id: str
    conversation_id: str
    from_engine: str
    to_engine: str
    message_type: str
    payload: Dict[str, Any]
    priority: str = "normal"
    timestamp: str = ""
    ttl_seconds: int = 3600
    requires_ack: bool = False
    acked: bool = False
    delivered: bool = False
    error: Optional[str] = None
    
    def __post_init__(self):
        if not self.timestamp:
            self.timestamp = datetime.utcnow().isoformat()
        if not self.message_id:
            self.message_id = str(uuid.uuid4())
    
    def is_expired(self) -> bool:
        ts = datetime.fromisoformat(self.timestamp)
        return datetime.utcnow() > ts + timedelta(seconds=self.ttl_seconds)
    
    def to_dict(self) -> dict:
        return {
            "message_id": self.message_id,
            "conversation_id": self.conversation_id,
            "from_engine": self.from_engine,
            "to_engine": self.to_engine,
            "type": self.message_type,
            "payload": self.payload,
            "priority": self.priority,
            "timestamp": self.timestamp,
            "ttl_seconds": self.ttl_seconds,
            "requires_ack": self.requires_ack,
            "acked": self.acked,
            "delivered": self.delivered,
            "error": self.error,
        }
    
    @classmethod
    def from_dict(cls, data: dict) -> "A2AMessage":
        return cls(
            message_id=data["message_id"],
            conversation_id=data["conversation_id"],
            from_engine=data["from_engine"],
            to_engine=data["to_engine"],
            message_type=data["type"],
            payload=data["payload"],
            priority=data.get("priority", "normal"),
            timestamp=data.get("timestamp", ""),
            ttl_seconds=data.get("ttl_seconds", 3600),
            requires_ack=data.get("requires_ack", False),
            acked=data.get("acked", False),
            delivered=data.get("delivered", False),
            error=data.get("error"),
        )


# --- Message Factory ---

class MessageFactory:
    """Create typed A2A messages for common patterns."""
    
    @staticmethod
    def rag_to_ai_query(
        conversation_id: str,
        question: str,
        context: str,
        confidence_threshold: float = 0.7,
    ) -> A2AMessage:
        """RAG asks AI to generate an answer."""
        return A2AMessage(
            message_id=str(uuid.uuid4()),
            conversation_id=conversation_id,
            from_engine=EngineType.RAG.value,
            to_engine=EngineType.AI.value,
            message_type=MessageType.QUERY.value,
            payload={
                "question": question,
                "context": context,
                "confidence_threshold": confidence_threshold,
                "requires_reasoning": True,
            },
            priority=MessagePriority.HIGH.value,
            requires_ack=True,
        )
    
    @staticmethod
    def ai_to_rag_response(
        conversation_id: str,
        answer: str,
        confidence: float,
        sources: List[str],
        reasoning: str = "",
    ) -> A2AMessage:
        """AI responds to RAG with answer."""
        return A2AMessage(
            message_id=str(uuid.uuid4()),
            conversation_id=conversation_id,
            from_engine=EngineType.AI.value,
            to_engine=EngineType.RAG.value,
            message_type=MessageType.RESPONSE.value,
            payload={
                "answer": answer,
                "confidence": confidence,
                "sources": sources,
                "reasoning": reasoning,
            },
        )
    
    @staticmethod
    def ai_to_dev_command(
        conversation_id: str,
        command: str,
        context: str,
        expected_output: str = "",
    ) -> A2AMessage:
        """AI asks Dev to execute code/command."""
        return A2AMessage(
            message_id=str(uuid.uuid4()),
            conversation_id=conversation_id,
            from_engine=EngineType.AI.value,
            to_engine=EngineType.DEV.value,
            message_type=MessageType.COMMAND.value,
            payload={
                "command": command,
                "context": context,
                "expected_output": expected_output,
            },
            priority=MessagePriority.HIGH.value,
            requires_ack=True,
        )
    
    @staticmethod
    def dev_to_ai_response(
        conversation_id: str,
        output: str,
        exit_code: int = 0,
        error: str = "",
    ) -> A2AMessage:
        """Dev responds with command execution result."""
        return A2AMessage(
            message_id=str(uuid.uuid4()),
            conversation_id=conversation_id,
            from_engine=EngineType.DEV.value,
            to_engine=EngineType.AI.value,
            message_type=MessageType.RESPONSE.value,
            payload={
                "output": output,
                "exit_code": exit_code,
                "error": error,
            },
        )
    
    @staticmethod
    def status_update(
        conversation_id: str,
        from_engine: str,
        status: str,
        details: Dict = None,
    ) -> A2AMessage:
        """Broadcast status update."""
        return A2AMessage(
            message_id=str(uuid.uuid4()),
            conversation_id=conversation_id,
            from_engine=from_engine,
            to_engine=EngineType.COORDINATOR.value,
            message_type=MessageType.STATUS.value,
            payload={
                "status": status,
                "details": details or {},
            },
            priority=MessagePriority.NORMAL.value,
        )
    
    @staticmethod
    def error_notification(
        conversation_id: str,
        from_engine: str,
        to_engine: str,
        error: str,
        error_type: str = "unknown",
    ) -> A2AMessage:
        """Notify another engine of an error."""
        return A2AMessage(
            message_id=str(uuid.uuid4()),
            conversation_id=conversation_id,
            from_engine=from_engine,
            to_engine=to_engine,
            message_type=MessageType.ERROR.value,
            payload={
                "error": error,
                "error_type": error_type,
            },
            priority=MessagePriority.CRITICAL.value,
        )


# --- Message Bus (File-based) ---

class A2AMessageBus:
    """
    File-based message bus for inter-engine communication.
    Messages stored in /workspace/intermediate/{conversation_id}/
    
    Thread-safe, atomic writes, TTL-based cleanup.
    """
    
    def __init__(self, workspace_root: str = "/workspace"):
        self.workspace_root = workspace_root
        self.intermediate_dir = os.path.join(workspace_root, "intermediate")
        os.makedirs(self.intermediate_dir, exist_ok=True)
        self._lock = threading.Lock()
        self._handlers: Dict[str, Callable] = {}  # engine → handler
    
    def send(self, message: A2AMessage) -> str:
        """
        Send a message to another engine.
        Returns message_id.
        
        Messages are stored in the conversation's directory.
        """
        with self._lock:
            conv_dir = self._get_conversation_dir(message.conversation_id)
            
            # Filename: {timestamp}_{from}_{to}_{type}_{id}.json
            filename = (
                f"{message.timestamp.replace(':', '-').replace('.', '-')}_"
                f"{message.from_engine}_to_{message.to_engine}_"
                f"{message.message_type}_{message.message_id}.json"
            )
            filepath = os.path.join(conv_dir, filename)
            
            # Atomic write
            tmp_path = filepath + ".tmp"
            with open(tmp_path, "w") as f:
                json.dump(message.to_dict(), f, indent=2)
                f.flush()
                os.fsync(f.fileno())
            
            os.rename(tmp_path, filepath)
            
            logger.debug(f"A2A: sent {message.message_type} from {message.from_engine} → {message.to_engine}")
            
            return message.message_id
    
    def receive(
        self,
        conversation_id: str,
        for_engine: str,
        message_type: str = None,
        limit: int = 50,
    ) -> List[A2AMessage]:
        """
        Receive messages for a specific engine.
        Optionally filter by message type.
        """
        conv_dir = self._get_conversation_dir(conversation_id)
        if not os.path.exists(conv_dir):
            return []
        
        messages = []
        for filename in sorted(os.listdir(conv_dir)):
            if not filename.endswith(".json"):
                continue
            if f"_to_{for_engine}_" not in filename:
                continue
            
            filepath = os.path.join(conv_dir, filename)
            try:
                with open(filepath, "r") as f:
                    data = json.load(f)
                
                msg = A2AMessage.from_dict(data)
                
                # Skip expired
                if msg.is_expired():
                    continue
                
                # Filter by type
                if message_type and msg.message_type != message_type:
                    continue
                
                messages.append(msg)
            except (json.JSONDecodeError, KeyError):
                continue
        
        return messages[-limit:]
    
    def mark_delivered(self, conversation_id: str, message_id: str):
        """Mark a message as delivered."""
        conv_dir = self._get_conversation_dir(conversation_id)
        if not os.path.exists(conv_dir):
            return
        
        for filename in os.listdir(conv_dir):
            if message_id in filename and filename.endswith(".json"):
                filepath = os.path.join(conv_dir, filename)
                try:
                    with open(filepath, "r") as f:
                        data = json.load(f)
                    data["delivered"] = True
                    with open(filepath, "w") as f:
                        json.dump(data, f)
                except Exception:
                    pass
    
    def mark_acked(self, conversation_id: str, message_id: str):
        """Mark a message as acknowledged."""
        conv_dir = self._get_conversation_dir(conversation_id)
        if not os.path.exists(conv_dir):
            return
        
        for filename in os.listdir(conv_dir):
            if message_id in filename and filename.endswith(".json"):
                filepath = os.path.join(conv_dir, filename)
                try:
                    with open(filepath, "r") as f:
                        data = json.load(f)
                    data["acked"] = True
                    with open(filepath, "w") as f:
                        json.dump(data, f)
                except Exception:
                    pass
    
    def get_conversation_history(
        self,
        conversation_id: str,
        limit: int = 100,
    ) -> List[A2AMessage]:
        """Get full conversation history."""
        conv_dir = self._get_conversation_dir(conversation_id)
        if not os.path.exists(conv_dir):
            return []
        
        messages = []
        for filename in sorted(os.listdir(conv_dir)):
            if not filename.endswith(".json"):
                continue
            
            filepath = os.path.join(conv_dir, filename)
            try:
                with open(filepath, "r") as f:
                    data = json.load(f)
                messages.append(A2AMessage.from_dict(data))
            except (json.JSONDecodeError, KeyError):
                continue
        
        return messages[-limit:]
    
    def delete_conversation(self, conversation_id: str):
        """Delete all messages for a conversation."""
        conv_dir = self._get_conversation_dir(conversation_id)
        if os.path.exists(conv_dir):
            import shutil
            shutil.rmtree(conv_dir)
    
    def list_conversations(self) -> List[str]:
        """List all active conversations."""
        if not os.path.exists(self.intermediate_dir):
            return []
        return [
            d for d in os.listdir(self.intermediate_dir)
            if os.path.isdir(os.path.join(self.intermediate_dir, d))
        ]
    
    def cleanup_expired(self) -> int:
        """Remove expired messages."""
        cleaned = 0
        if not os.path.exists(self.intermediate_dir):
            return 0
        
        for conv_id in self.list_conversations():
            conv_dir = self._get_conversation_dir(conv_id)
            for filename in os.listdir(conv_dir):
                if not filename.endswith(".json"):
                    continue
                
                filepath = os.path.join(conv_dir, filename)
                try:
                    with open(filepath, "r") as f:
                        data = json.load(f)
                    msg = A2AMessage.from_dict(data)
                    if msg.is_expired():
                        os.remove(filepath)
                        cleaned += 1
                except Exception:
                    pass
        
        return cleaned
    
    def _get_conversation_dir(self, conversation_id: str) -> str:
        conv_dir = os.path.join(self.intermediate_dir, conversation_id)
        os.makedirs(conv_dir, exist_ok=True)
        return conv_dir


# --- OPA Policy Gate ---

class OPAPolicyGate:
    """
    OPA policy check before every A2A message delivery.
    Zero-trust: even internal engine calls need authorization.
    """
    
    def __init__(self, opa_endpoint: str = "http://localhost:8181"):
        self.opa_endpoint = opa_endpoint
    
    def check(self, message: A2AMessage) -> tuple[bool, str]:
        """
        Check if message is allowed by OPA policy.
        
        Returns (allowed, reason).
        """
        import requests
        
        input_data = {
            "input": {
                "from_engine": message.from_engine,
                "to_engine": message.to_engine,
                "message_type": message.message_type,
                "priority": message.priority,
                "conversation_id": message.conversation_id,
            }
        }
        
        try:
            resp = requests.post(
                f"{self.opa_endpoint}/v1/data/aetheris/allow",
                json=input_data,
                timeout=5,
            )
            
            if resp.status_code != 200:
                return False, f"OPA returned {resp.status_code}"
            
            data = resp.json()
            result = data.get("result", {})
            
            if result.get("allow", False):
                return True, "allowed"
            else:
                return False, result.get("reason", "policy denied")
        
        except requests.RequestException as e:
            # OPA unavailable — default deny for safety
            return False, f"OPA unavailable: {e}"
    
    def check_local(self, message: A2AMessage) -> tuple[bool, str]:
        """
        Local policy check (fallback when OPA unavailable).
        Basic rules for inter-engine communication.
        """
        from_engine = message.from_engine
        to_engine = message.to_engine
        
        # Allowed communication patterns
        allowed_patterns = {
            ("rag", "ai"),
            ("ai", "rag"),
            ("ai", "dev"),
            ("dev", "ai"),
            ("rag", "coordinator"),
            ("ai", "coordinator"),
            ("dev", "coordinator"),
            ("coordinator", "rag"),
            ("coordinator", "ai"),
            ("coordinator", "dev"),
        }
        
        if (from_engine, to_engine) not in allowed_patterns:
            return False, f"Communication {from_engine} → {to_engine} not allowed"
        
        return True, "allowed"


# --- A2A Router ---

class A2ARouter:
    """
    Routes messages between engines with OPA policy enforcement.
    
    Usage:
        router = A2ARouter(workspace_root="/workspace", opa_endpoint="http://opa:8181")
        
        # Send message
        msg = MessageFactory.rag_to_ai_query("conv-123", "question", "context")
        router.send(msg)
        
        # Receive messages for an engine
        messages = router.receive("conv-123", for_engine="ai")
    """
    
    def __init__(
        self,
        workspace_root: str = "/workspace",
        opa_endpoint: str = "http://localhost:8181",
    ):
        self.bus = A2AMessageBus(workspace_root)
        self.opa_gate = OPAPolicyGate(opa_endpoint)
    
    def send(self, message: A2AMessage) -> str:
        """Send message with OPA policy check."""
        # Try OPA first, fall back to local policy
        allowed, reason = self.opa_gate.check(message)
        if not allowed:
            allowed, reason = self.opa_gate.check_local(message)
        
        if not allowed:
            logger.warning(f"A2A: policy denied {message.from_engine} → {message.to_engine}: {reason}")
            raise PolicyDeniedError(f"Message denied: {reason}")
        
        return self.bus.send(message)
    
    def receive(self, conversation_id: str, for_engine: str,
                message_type: str = None, limit: int = 50) -> List[A2AMessage]:
        return self.bus.receive(conversation_id, for_engine, message_type, limit)
    
    def get_history(self, conversation_id: str, limit: int = 100) -> List[A2AMessage]:
        return self.bus.get_conversation_history(conversation_id, limit)
    
    def cleanup(self) -> int:
        return self.bus.cleanup_expired()


# --- Custom Exceptions ---

class PolicyDeniedError(Exception):
    """OPA policy denied the message."""
    pass
