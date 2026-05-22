"""
A2A Integration — Wire A2A protocol into the Agent Orchestrator.

Provides:
- A2AGateway: Routes messages between agents via A2A protocol
- OPA-gated message delivery
- Conversation-scoped message bus
- TTL-based message cleanup
"""

import os
import json
import time
import uuid
import logging
from typing import Any, Dict, List, Optional
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)


@dataclass
class A2AMessage:
    """A2A message between agents."""
    id: str
    conversation_id: str
    from_agent: str
    to_agent: str
    message_type: str  # request, response, notification
    content: Dict
    timestamp: float = field(default_factory=time.time)
    ttl: int = 300  # seconds
    policy_approved: bool = False


class OPAPolicyGate:
    """OPA policy enforcement for A2A messages."""

    def __init__(self, opa_endpoint: str = "http://localhost:8181", policy: str = "aetheris/agent_policy"):
        self.opa_endpoint = opa_endpoint
        self.policy = policy
    
    async def evaluate(self, message: A2AMessage) -> bool:
        """Evaluate if a message should be delivered based on OPA policy."""
        # In production: POST to OPA endpoint with message context
        # For now: default allow with basic validation
        if not message.from_agent or not message.to_agent:
            return False
        if not message.content:
            return False
        return True


class A2AGateway:
    """Routes messages between agents via A2A protocol."""

    def __init__(self, workspace_root: str = "/workspace", ttl: int = 300):
        self.workspace_root = workspace_root
        self.intermediate_dir = os.path.join(workspace_root, "intermediate")
        self.ttl = ttl
        self.policy_gate = OPAPolicyGate()
        self._message_log: List[A2AMessage] = []
    
    def _conversation_dir(self, conversation_id: str) -> str:
        d = os.path.join(self.intermediate_dir, conversation_id)
        os.makedirs(d, exist_ok=True)
        return d
    
    async def send(self, message: A2AMessage) -> bool:
        """Send a message via A2A protocol."""
        # Policy check
        approved = await self.policy_gate.evaluate(message)
        message.policy_approved = approved
        
        if not approved:
            logger.warning(f"Message {message.id} blocked by OPA policy")
            return False
        
        # Write to conversation directory
        conv_dir = self._conversation_dir(message.conversation_id)
        msg_file = os.path.join(conv_dir, f"{message.id}.json")
        
        msg_data = {
            "id": message.id,
            "conversation_id": message.conversation_id,
            "from_agent": message.from_agent,
            "to_agent": message.to_agent,
            "message_type": message.message_type,
            "content": message.content,
            "timestamp": message.timestamp,
            "ttl": message.ttl,
            "policy_approved": message.policy_approved,
        }
        
        try:
            with open(msg_file, "w") as f:
                json.dump(msg_data, f, indent=2)
            self._message_log.append(message)
            logger.info(f"A2A message sent: {message.from_agent} → {message.to_agent}")
            return True
        except Exception as e:
            logger.error(f"Failed to write A2A message: {e}")
            return False
    
    async def receive(self, conversation_id: str, agent_id: str) -> List[A2AMessage]:
        """Receive pending messages for an agent."""
        conv_dir = self._conversation_dir(conversation_id)
        messages = []
        
        try:
            for fname in os.listdir(conv_dir):
                if not fname.endswith(".json"):
                    continue
                fpath = os.path.join(conv_dir, fname)
                with open(fpath, "r") as f:
                    data = json.load(f)
                
                # Check TTL
                if time.time() - data.get("timestamp", 0) > self.ttl:
                    os.remove(fpath)  # Expired
                    continue
                
                # Filter for this agent
                if data.get("to_agent") == agent_id:
                    messages.append(A2AMessage(
                        id=data["id"],
                        conversation_id=data["conversation_id"],
                        from_agent=data["from_agent"],
                        to_agent=data["to_agent"],
                        message_type=data["message_type"],
                        content=data["content"],
                        timestamp=data["timestamp"],
                        ttl=data["ttl"],
                        policy_approved=data.get("policy_approved", False),
                    ))
        except Exception as e:
            logger.error(f"Failed to read A2A messages: {e}")
        
        return messages
    
    def cleanup_expired(self) -> int:
        """Remove expired messages."""
        count = 0
        try:
            for conv_id in os.listdir(self.intermediate_dir):
                conv_dir = os.path.join(self.intermediate_dir, conv_id)
                if not os.path.isdir(conv_dir):
                    continue
                for fname in os.listdir(conv_dir):
                    if not fname.endswith(".json"):
                        continue
                    fpath = os.path.join(conv_dir, fname)
                    try:
                        with open(fpath, "r") as f:
                            data = json.load(f)
                        if time.time() - data.get("timestamp", 0) > self.ttl:
                            os.remove(fpath)
                            count += 1
                    except (json.JSONDecodeError, IOError):
                        os.remove(fpath)  # Corrupt file
                        count += 1
        except Exception as e:
            logger.error(f"Cleanup failed: {e}")
        return count
    
    def get_message_log(self, limit: int = 50) -> List[Dict]:
        """Get recent message log."""
        return [
            {
                "id": m.id,
                "from_agent": m.from_agent,
                "to_agent": m.to_agent,
                "message_type": m.message_type,
                "timestamp": m.timestamp,
                "approved": m.policy_approved,
            }
            for m in self._message_log[-limit:]
        ]
