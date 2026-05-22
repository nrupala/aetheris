"""
Agent Orchestrator Server — FastAPI server with MCP, A2A, multi-agent, and Phase 3 cross-system orchestration.

Endpoints:
    POST /task/submit          — Submit task (single agent or multi-agent workflow)
    GET  /task/{id}            — Get task status and results
    GET  /tasks                — List recent tasks
    POST /workflow/run         — Run multi-agent workflow (planner → researcher → coder → reviewer)
    GET  /agents               — List all agents
    GET  /agents/status        — Agent status
    POST /mcp/request          — MCP protocol endpoint
    GET  /mcp/tools            — List MCP tools
    GET  /mcp/prompts          — List MCP prompts
    GET  /orchestrator/state   — Cross-engine state dashboard
    GET  /orchestrator/forecast — Resource spread forecast
    GET  /orchestrator/kg-hub   — Shared KG dashboard
    POST /orchestrator/kg-hub/* — Shared KG operations
    GET  /a2a/messages         — A2A message log
    GET  /health               — Health check

Usage:
    python -m agent_orchestrator.server --host 0.0.0.0 --port 9090
"""

import os
import sys
import uuid
import time
import asyncio
import logging
import json
from typing import Dict, List, Optional
from datetime import datetime

from fastapi import FastAPI, HTTPException, Query
from pydantic import BaseModel

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from agent_orchestrator.config import config
from agent_orchestrator.mcp import MCPServer, register_default_tools, register_default_prompts
from agent_orchestrator.agents import create_agent, BaseAgent, AgentResult, OPAPolicyGate
from agent_orchestrator.a2a_gateway import A2AGateway, A2AMessage
from agent_orchestrator.cross_system import (
    CrossEngineState, EngineType, ResourceForecaster,
    SharedKGHub, SpreadForecaster, CrossSystemOrchestrator,
)
from agent_orchestrator.rag_client import RAGClient, KGClient

logger = logging.getLogger(__name__)

app = FastAPI(title="Aetheris Agent Orchestrator", version="2.0.0")

# Global state
mcp_server = MCPServer(name=config.mcp_server_name, version=config.mcp_server_version)
agents: Dict[str, BaseAgent] = {}
tasks: Dict[str, Dict] = {}
a2a_gateway = A2AGateway(workspace_root=config.workspace_root, ttl=config.message_ttl_seconds)
opa_gate = OPAPolicyGate(opa_endpoint=config.opa_endpoint)
cross_orchestrator: Optional[CrossSystemOrchestrator] = None
rag_client: Optional[RAGClient] = None
kg_client: Optional[KGClient] = None


# --- Models ---

class TaskRequest(BaseModel):
    task: str
    role: Optional[str] = None
    context: Dict = {}
    max_iterations: int = 3
    use_reasoning: bool = True


class WorkflowRequest(BaseModel):
    task: str
    max_iterations: int = 3
    use_reasoning: bool = True
    skip_review: bool = False


class MCPRequest(BaseModel):
    method: str
    params: Optional[Dict] = {}


class KGWriteRequest(BaseModel):
    engine: str
    action: str
    data: Dict


# --- Startup ---

@app.on_event("startup")
async def startup():
    global cross_orchestrator, rag_client, kg_client
    
    rag_client = RAGClient(endpoint=os.environ.get("RAG_ENDPOINT", "http://rag-service:8080"))
    kg_client = KGClient(endpoint=os.environ.get("RAG_ENDPOINT", "http://rag-service:8080"))
    
    register_default_tools(mcp_server.tools, rag_pipeline=None, kg=None)
    register_default_prompts(mcp_server.prompts)
    
    state_dir = os.path.join(config.workspace_root, "orchestrator_state")
    os.makedirs(state_dir, exist_ok=True)
    
    cross_orchestrator = CrossSystemOrchestrator(
        state_dir=state_dir,
        host_memory_mb=15360,
        kg_instance=None,
    )
    
    for engine in EngineType:
        cross_orchestrator.state.update_engine(engine, status="healthy")
    
    for role in config.agent_roles:
        model = config.role_models.get(role, config.default_model)
        prompt = config.role_prompts.get(role, f"You are a {role} agent.")
        agent = create_agent(role, model, prompt, opa_gate)
        agents[agent.id] = agent
        logger.info(f"Created agent: {agent.id} ({role}) using model {model}")
    
    rag_healthy = await rag_client.health()
    logger.info(f"Agent Orchestrator started: {len(agents)} agents, RAG health={rag_healthy}")


# --- Workflow Engine ---

async def run_workflow(task: str, max_iterations: int = 3, use_reasoning: bool = True, skip_review: bool = False) -> Dict:
    workflow_id = f"wf_{uuid.uuid4().hex[:12]}"
    conversation_id = f"conv_{uuid.uuid4().hex[:12]}"
    
    steps_executed = []
    final_output = ""
    start_time = time.time()
    
    if cross_orchestrator:
        cross_orchestrator.state.update_engine(EngineType.AGENTS, status="busy", active_tasks=1)
    
    planner = None
    for agent in agents.values():
        if agent.role == "planner":
            planner = agent
            break
    
    if planner:
        plan_result = await planner.execute(task, {
            "available_agents": [a.role for a in agents.values()],
            "kg_client": kg_client,
        })
        
        steps_executed.append({
            "step": 1,
            "agent": "planner",
            "agent_id": planner.id,
            "success": plan_result.success,
            "duration_ms": plan_result.duration_ms,
            "output_preview": plan_result.output[:200] if plan_result.output else "",
        })
        
        await a2a_gateway.send(A2AMessage(
            id=f"msg_{uuid.uuid4().hex[:8]}",
            conversation_id=conversation_id,
            from_agent=planner.id,
            to_agent="researcher",
            message_type="request",
            content={"plan": plan_result.output, "original_task": task},
        ))
    
    researcher = None
    for agent in agents.values():
        if agent.role == "researcher":
            researcher = agent
            break
    
    if researcher:
        research_result = await researcher.execute(task, {
            "rag_client": rag_client,
            "kg_client": kg_client,
            "use_reasoning": use_reasoning,
            "top_k": 5,
        })
        
        steps_executed.append({
            "step": 2,
            "agent": "researcher",
            "agent_id": researcher.id,
            "success": research_result.success,
            "duration_ms": research_result.duration_ms,
            "output_preview": research_result.output[:200] if research_result.output else "",
            "kg_stats": research_result.metadata.get("kg_stats", {}),
        })
        
        await a2a_gateway.send(A2AMessage(
            id=f"msg_{uuid.uuid4().hex[:8]}",
            conversation_id=conversation_id,
            from_agent=researcher.id,
            to_agent="coder",
            message_type="request",
            content={"findings": research_result.output, "original_task": task},
        ))
    
    coder = None
    for agent in agents.values():
        if agent.role == "coder":
            coder = agent
            break
    
    if coder:
        code_result = await coder.execute(task, {
            "rag_client": rag_client,
            "kg_client": kg_client,
            "workspace": config.workspace_root,
        })
        
        steps_executed.append({
            "step": 3,
            "agent": "coder",
            "agent_id": coder.id,
            "success": code_result.success,
            "duration_ms": code_result.duration_ms,
            "output_preview": code_result.output[:200] if code_result.output else "",
        })
        
        final_output = code_result.output
        
        # A2A: Send code to reviewer
        if not skip_review:
            await a2a_gateway.send(A2AMessage(
                id=f"msg_{uuid.uuid4().hex[:8]}",
                conversation_id=conversation_id,
                from_agent=coder.id,
                to_agent="reviewer",
                message_type="request",
                content={"code": code_result.output, "task": task},
            ))
    
    # Step 4: Reviewer evaluates (optional)
    if not skip_review:
        reviewer = None
        for agent in agents.values():
            if agent.role == "reviewer":
                reviewer = agent
                break
        
        if reviewer:
            review_result = await reviewer.execute(task, {
                "content": final_output,
                "kg_client": kg_client,
                "rag_client": rag_client,
                "criteria": ["correctness", "quality", "security"],
            })
            
            steps_executed.append({
                "step": 4,
                "agent": "reviewer",
                "agent_id": reviewer.id,
                "success": review_result.success,
                "duration_ms": review_result.duration_ms,
                "output_preview": review_result.output[:200] if review_result.output else "",
            })
            
            # Merge review into final output
            final_output = f"## Implementation\n\n{final_output}\n\n## Review\n\n{review_result.output}"
    
    # Record engine state
    if cross_orchestrator:
        cross_orchestrator.state.update_engine(EngineType.AGENTS, status="healthy", active_tasks=0)
    
    total_duration_ms = (time.time() - start_time) * 1000
    
    return {
        "workflow_id": workflow_id,
        "conversation_id": conversation_id,
        "task": task,
        "steps_executed": steps_executed,
        "total_steps": len(steps_executed),
        "total_duration_ms": round(total_duration_ms, 1),
        "final_output": final_output,
        "success": all(s["success"] for s in steps_executed),
    }


# --- Task Endpoints ---

@app.post("/task/submit")
async def submit_task(req: TaskRequest):
    """Submit a task for agent execution."""
    task_id = f"task_{uuid.uuid4().hex[:12]}"
    
    if req.role:
        selected = None
        for agent in agents.values():
            if agent.role == req.role and agent.state.value == "idle":
                selected = agent
                break
        if not selected:
            model = config.role_models.get(req.role, config.default_model)
            prompt = config.role_prompts.get(req.role, "")
            selected = create_agent(req.role, model, prompt, opa_gate)
            agents[selected.id] = selected
    else:
        selected = None
        for agent in agents.values():
            if agent.role == "planner" and agent.state.value == "idle":
                selected = agent
                break
        if not selected:
            selected = list(agents.values())[0]
    
    tasks[task_id] = {
        "task_id": task_id,
        "task": req.task,
        "role": req.role or "auto",
        "agent_id": selected.id,
        "status": "queued",
        "created_at": datetime.utcnow().isoformat(),
        "context": req.context,
        "max_iterations": req.max_iterations,
        "use_reasoning": req.use_reasoning,
    }
    
    return {"task_id": task_id, "status": "queued", "agent_id": selected.id}


@app.get("/task/{task_id}")
async def get_task(task_id: str):
    task = tasks.get(task_id)
    if not task:
        raise HTTPException(404, f"Task not found: {task_id}")
    return task


@app.get("/tasks")
async def list_tasks(limit: int = Query(default=20)):
    sorted_tasks = sorted(tasks.values(), key=lambda t: t["created_at"], reverse=True)
    return sorted_tasks[:limit]


# --- Workflow Endpoints ---

@app.post("/workflow/run")
async def run_workflow_endpoint(req: WorkflowRequest):
    """Run a full multi-agent workflow."""
    result = await run_workflow(
        task=req.task,
        max_iterations=req.max_iterations,
        use_reasoning=req.use_reasoning,
        skip_review=req.skip_review,
    )
    return result


# --- Agent Endpoints ---

@app.get("/agents")
async def list_agents():
    return [agent.get_status() for agent in agents.values()]


@app.get("/agents/status")
async def agents_status():
    return {
        "total": len(agents),
        "idle": sum(1 for a in agents.values() if a.state.value == "idle"),
        "busy": sum(1 for a in agents.values() if a.state.value != "idle"),
        "agents": [agent.get_status() for agent in agents.values()],
    }


# --- MCP Endpoints ---

@app.post("/mcp/request")
async def mcp_request(req: MCPRequest):
    result = await mcp_server.handle_request(req.method, req.params)
    return result


@app.get("/mcp/tools")
async def list_mcp_tools():
    return {"tools": mcp_server.tools.list_tools()}


@app.get("/mcp/prompts")
async def list_mcp_prompts():
    return {"prompts": mcp_server.prompts.list_prompts()}


@app.get("/mcp/resources")
async def list_mcp_resources():
    return {"resources": mcp_server.resources.list_resources()}


# --- Phase 3: Cross-System Endpoints ---

@app.get("/orchestrator/state")
async def orchestrator_state():
    """Cross-engine state dashboard."""
    if not cross_orchestrator:
        raise HTTPException(503, "Cross-system orchestrator not initialized")
    return cross_orchestrator.state.get_dashboard()


@app.post("/orchestrator/state/engine/{engine_name}")
async def update_engine_state(engine_name: str, updates: Dict):
    """Update an engine's state."""
    if not cross_orchestrator:
        raise HTTPException(503, "Cross-system orchestrator not initialized")
    try:
        engine = EngineType(engine_name)
        updated = cross_orchestrator.state.update_engine(engine, **updates)
        return {"engine": engine_name, "status": updated.status}
    except ValueError:
        raise HTTPException(400, f"Unknown engine: {engine_name}")


@app.get("/orchestrator/forecast")
async def orchestrator_forecast():
    """Resource spread forecast."""
    if not cross_orchestrator:
        raise HTTPException(503, "Cross-system orchestrator not initialized")
    return cross_orchestrator.spread_forecaster.get_dashboard()


@app.get("/orchestrator/kg-hub")
async def kg_hub_dashboard():
    """Shared KG dashboard."""
    if not cross_orchestrator:
        raise HTTPException(503, "Cross-system orchestrator not initialized")
    return cross_orchestrator.kg_hub.get_dashboard("agents")


@app.post("/orchestrator/kg-hub/write")
async def kg_hub_write(req: KGWriteRequest):
    """Write to shared KG."""
    if not cross_orchestrator:
        raise HTTPException(503, "Cross-system orchestrator not initialized")
    
    action = req.data.get("action", "")
    if action == "add_entity":
        return cross_orchestrator.kg_hub.add_entity(
            req.engine, req.data["name"], req.data["entity_type"], req.data.get("properties", {}),
        )
    elif action == "add_relation":
        return cross_orchestrator.kg_hub.add_relation(
            req.engine, req.data["from"], req.data["type"], req.data["to"],
        )
    elif action == "record_interaction":
        return cross_orchestrator.kg_hub.record_interaction(req.engine, req.data["type"], req.data.get("details", {}))
    else:
        raise HTTPException(400, f"Unknown action: {action}")


@app.get("/orchestrator/snapshot")
async def create_snapshot():
    """Create a state snapshot."""
    if not cross_orchestrator:
        raise HTTPException(503, "Cross-system orchestrator not initialized")
    return cross_orchestrator.state.snapshot()


@app.get("/a2a/messages")
async def a2a_messages(limit: int = Query(default=50)):
    """Get A2A message log."""
    return {"messages": a2a_gateway.get_message_log(limit)}


# --- Health ---

@app.get("/health")
async def health():
    forecast = cross_orchestrator.spread_forecaster.forecast() if cross_orchestrator else None
    return {
        "status": "healthy",
        "agents": len(agents),
        "tasks": len(tasks),
        "tools": len(mcp_server.tools.list_tools()),
        "prompts": len(mcp_server.prompts.list_prompts()),
        "cross_system": cross_orchestrator is not None,
        "spread_forecast": {
            "total_memory_mb": forecast.total_memory_mb if forecast else 0,
            "confidence": forecast.confidence if forecast else 0,
            "bottleneck": forecast.bottleneck if forecast else None,
        },
        "uptime": datetime.utcnow().isoformat(),
    }


# --- CLI Entry Point ---

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Aetheris Agent Orchestrator")
    parser.add_argument("--host", default=config.host, help="Bind address")
    parser.add_argument("--port", type=int, default=config.port, help="Port number")
    parser.add_argument("--log-level", default="info", help="Logging level")
    args = parser.parse_args()
    
    logging.basicConfig(
        level=getattr(logging, args.log_level.upper()),
        format="%(asctime)s [%(name)s] %(levelname)s: %(message)s",
    )
    
    import uvicorn
    uvicorn.run(app, host=args.host, port=args.port, log_level=args.log_level.lower())


if __name__ == "__main__":
    main()
