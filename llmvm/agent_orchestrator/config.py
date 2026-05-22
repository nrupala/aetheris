"""
Agent Orchestrator Configuration.
"""

import os
from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class OrchestratorConfig:
    """Configuration for the Agent Orchestrator."""
    
    # Server
    host: str = "0.0.0.0"
    port: int = 9090
    
    # Model routing
    lmstudio_endpoint: str = os.environ.get("LMSTUDIO_ENDPOINT", "http://localhost:1234")
    default_model: str = os.environ.get("DEFAULT_MODEL", "strand-rust-coder-14b-v1")
    fallback_model: str = os.environ.get("FALLBACK_MODEL", "ibm/granite-4-h-tiny")
    
    # A2A Protocol
    workspace_root: str = os.environ.get("WORKSPACE_ROOT", "/workspace")
    intermediate_dir: str = os.path.join(workspace_root, "intermediate")
    message_ttl_seconds: int = 300  # 5 minutes
    
    # OPA Policy Gate
    opa_endpoint: str = os.environ.get("OPA_ENDPOINT", "http://localhost:8181")
    opa_policy: str = "aetheris/agent_policy"
    
    # Agent limits
    max_concurrent_agents: int = 4
    max_iterations_per_task: int = 10
    task_timeout_seconds: int = 300
    
    # MCP Server
    mcp_server_name: str = "aetheris-orchestrator"
    mcp_server_version: str = "2.0.0"
    
    # Agent roles configuration
    agent_roles: List[str] = field(default_factory=lambda: [
        "researcher", "coder", "reviewer", "planner"
    ])
    
    # Model assignments per role
    role_models: dict = field(default_factory=lambda: {
        "researcher": "strand-rust-coder-14b-v1",
        "coder": "strand-rust-coder-14b-v1",
        "reviewer": "microsoft/phi-4-reasoning-plus",
        "planner": "microsoft/phi-4-reasoning-plus",
    })
    
    # System prompts per role
    role_prompts: dict = field(default_factory=lambda: {
        "researcher": (
            "You are a Research Agent. Your role is to gather information, "
            "query knowledge bases, analyze sources, and synthesize findings. "
            "Be thorough, cite sources, and identify gaps in information."
        ),
        "coder": (
            "You are a Coding Agent. Your role is to implement solutions, "
            "write clean code, follow best practices, and ensure correctness. "
            "Always explain your code and include error handling."
        ),
        "reviewer": (
            "You are a Review Agent. Your role is to evaluate outputs, "
            "check quality, verify correctness, and identify issues. "
            "Be thorough but constructive. Score outputs on a scale of 1-10."
        ),
        "planner": (
            "You are a Planning Agent. Your role is to break down complex tasks, "
            "create step-by-step plans, coordinate between agents, and manage flow. "
            "Think strategically and identify dependencies."
        ),
    })
    
    @property
    def message_bus_dir(self) -> str:
        return self.intermediate_dir
    
    def conversation_dir(self, conversation_id: str) -> str:
        return os.path.join(self.intermediate_dir, conversation_id)


config = OrchestratorConfig()
