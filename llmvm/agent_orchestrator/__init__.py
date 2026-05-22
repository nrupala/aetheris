"""
Agent Orchestrator — Phase 2: Multi-agent system with MCP server and A2A protocol.

Architecture:
    ┌─────────────────────────────────────────────────────────┐
    │                    Agent Orchestrator                    │
    ├─────────────┬─────────────┬─────────────┬──────────────┤
    │  MCP Server │  Agent Pool │  A2A Router │  OPA Gate    │
    │  (FastAPI)  │ (Pydantic)  │ (File Bus)  │ (Policy)     │
    └─────────────┴─────────────┴─────────────┴──────────────┘
    
    Agent Roles:
    - Researcher: Gathers information, queries RAG, analyzes sources
    - Coder: Generates code, implements solutions, writes scripts
    - Reviewer: Evaluates outputs, checks quality, verifies correctness
    - Planner: Breaks down complex tasks, coordinates agents, manages flow

Usage:
    python -m agent_orchestrator.server --host 0.0.0.0 --port 9090
    
    # Or via rag_cli:
    python rag_cli.py orchestrator --port 9090
"""

__version__ = "2.0.0"
