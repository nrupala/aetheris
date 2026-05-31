"""
Agent base class and role implementations.

Full integration: RAG (with reasoning loop), KG read/write, OPA policy enforcement.

Agents:
- BaseAgent: Abstract base with OPA gate, RAG+KG context
- ResearcherAgent: Queries RAG with reasoning, extracts entities to KG
- CoderAgent: Uses RAG context + KG for code generation
- ReviewerAgent: Evaluates against KG history + RAG standards
- PlannerAgent: Decomposes tasks, coordinates multi-agent workflows
"""

import time
import uuid
import json
import logging
import os
import asyncio
from abc import ABC, abstractmethod
from typing import Any, Dict, List, Optional
from dataclasses import dataclass, field
from enum import Enum
from rag_core.model_router import ModelRouter

logger = logging.getLogger(__name__)


class AgentRole(Enum):
    RESEARCHER = "researcher"
    CODER = "coder"
    REVIEWER = "reviewer"
    PLANNER = "planner"


class AgentState(Enum):
    IDLE = "idle"
    THINKING = "thinking"
    EXECUTING = "executing"
    WAITING = "waiting"
    COMPLETE = "complete"
    FAILED = "failed"


@dataclass
class AgentResult:
    """Result from an agent execution."""
    agent_id: str
    role: str
    task: str
    output: str
    metadata: Dict = field(default_factory=dict)
    duration_ms: float = 0
    tokens_used: int = 0
    success: bool = True
    error: Optional[str] = None


class OPAPolicyGate:
    """OPA policy enforcement for agent actions."""

    def __init__(self, opa_endpoint: str = "http://localhost:8181"):
        self.opa_endpoint = opa_endpoint
        self._cache: Dict[str, bool] = {}
    
    async def check(self, agent_role: str, action: str, context: Dict) -> bool:
        """Check if an agent action is allowed by OPA policy."""
        cache_key = f"{agent_role}:{action}"
        if cache_key in self._cache:
            return self._cache[cache_key]
        
        try:
            input_data = {
                "agent": {"role": agent_role, "action": action},
                "context": context,
                "timestamp": time.time(),
            }
            
            import aiohttp
            async with aiohttp.ClientSession() as session:
                async with session.post(
                    f"{self.opa_endpoint}/v1/data/aetheris/agent_policy",
                    json={"input": input_data},
                    timeout=aiohttp.ClientTimeout(total=5),
                ) as resp:
                    if resp.status == 200:
                        result = await resp.json()
                        allowed = result.get("result", {}).get("allow", False)
                        self._cache[cache_key] = allowed
                        return allowed
                    return False
        except Exception as e:
            logger.warning(f"OPA check failed (default deny): {e}")
            return False
    
    def check_local(self, agent_role: str, action: str) -> bool:
        """Local policy check when OPA is unavailable."""
        allowed_actions = {
            "researcher": {"query", "read", "extract_entities", "list_sources"},
            "coder": {"write", "read", "execute_readonly", "list_directory"},
            "reviewer": {"read", "evaluate", "query_kg", "list_sources"},
            "planner": {"read", "query", "query_kg", "list_agents", "coordinate"},
        }
        return action in allowed_actions.get(agent_role, set())


class BaseAgent(ABC):
    """Abstract base for all agents with OPA gate + RAG + KG + LLM."""

    def __init__(self, role: str, model: str, system_prompt: str, opa_gate: Optional[OPAPolicyGate] = None, router: Optional[ModelRouter] = None):
        self.id = f"{role}_{uuid.uuid4().hex[:8]}"
        self.role = role
        self.model = model
        self.system_prompt = system_prompt
        self.opa_gate = opa_gate or OPAPolicyGate()
        self.router = router
        self.state = AgentState.IDLE
        self.task_history: List[Dict] = []
        self._policies_checked = 0
        self._policies_allowed = 0
    
    async def check_policy(self, action: str, context: Dict) -> bool:
        """Check OPA policy before executing action."""
        self._policies_checked += 1
        allowed = await self.opa_gate.check(self.role, action, context)
        if not allowed:
            allowed = self.opa_gate.check_local(self.role, action)
        if allowed:
            self._policies_allowed += 1
        return allowed

    async def _call_llm(self, messages: List[Dict[str, str]], temperature: float = 0.1, max_tokens: int = 4096) -> str:
        """Call LLM via ModelRouter. Falls back to stub if no router."""
        if self.router:
            try:
                resp = await asyncio.to_thread(
                    self.router.chat,
                    messages=messages,
                    temperature=temperature,
                    max_tokens=max_tokens,
                )
                return resp.text
            except Exception as e:
                logger.error(f"LLM call failed for {self.role}/{self.model}: {e}")
                return f"# Error: LLM call failed — {e}"
        return f"# Note: LLM router not configured — agent {self.id} needs a ModelRouter"

    async def get_kg_context(self, kg_client, query: str) -> str:
        if not kg_client:
            return ""
        try:
            return await kg_client.get_personal_context(query)
        except Exception as e:
            logger.warning(f"KG context failed: {e}")
            return ""
    
    async def query_rag(self, rag_client, query: str, top_k: int = 5, reasoning: bool = False) -> Optional[Any]:
        if not rag_client:
            return None
        try:
            return await rag_client.query(query, top_k=top_k, reasoning=reasoning)
        except Exception as e:
            logger.warning(f"RAG query failed: {e}")
            return None
    
    async def extract_entities_to_kg(self, kg_client, text: str, source: str) -> Dict:
        if not kg_client:
            return {}
        try:
            await kg_client.add_entity(
                name=f"entity_{uuid.uuid4().hex[:8]}",
                entity_type="concept",
                properties={"text_snippet": text[:200], "source": source},
            )
            return {"entities_added": 1, "source": source}
        except Exception as e:
            logger.warning(f"KG entity extraction failed: {e}")
            return {"error": str(e)}
    
    @abstractmethod
    async def execute(self, task: str, context: Dict) -> AgentResult:
        """Execute a task and return results."""
        pass
    
    def get_status(self) -> Dict:
        return {
            "id": self.id,
            "role": self.role,
            "model": self.model,
            "state": self.state.value,
            "tasks_completed": len([t for t in self.task_history if t.get("success")]),
            "policy_checks": self._policies_checked,
            "policy_allowed": self._policies_allowed,
        }


class ResearcherAgent(BaseAgent):
    """Research agent — queries RAG with reasoning, extracts entities to KG."""

    async def execute(self, task: str, context: Dict) -> AgentResult:
        start = time.time()
        self.state = AgentState.EXECUTING
        
        try:
            rag_pipeline = context.get("rag_pipeline")
            kg = context.get("kg")
            use_reasoning = context.get("use_reasoning", True)
            
            # Policy check
            if not await self.check_policy("query", {"task": task}):
                return AgentResult(
                    agent_id=self.id, role=self.role, task=task, output="",
                    error="Policy denied: query", duration_ms=(time.time()-start)*1000, success=False,
                )
            
            findings = []
            sources_used = []
            reasoning_traces = []
            
            # Step 1: Get KG context
            kg_context = self.get_kg_context(kg, task)
            
            # Step 2: Query RAG with reasoning loop
            rag_result = self.query_rag(rag_pipeline, task, top_k=context.get("top_k", 5), reasoning=use_reasoning)
            if rag_result:
                findings.append(rag_result.answer)
                sources_used.extend(rag_result.sources)
                if hasattr(rag_result, 'reasoning_trace') and rag_result.reasoning_trace:
                    reasoning_traces = rag_result.reasoning_trace
            else:
                # Fall back to direct LLM call when RAG unavailable
                research_prompt = f"""Research the following topic thoroughly:

Task: {task}

KG Context: {kg_context[:1000] if kg_context else 'none'}

Provide findings with key insights, sources, and analysis."""
                llm_result = await self._call_llm([
                    {"role": "system", "content": "You are a research agent. Gather information and synthesize findings."},
                    {"role": "user", "content": research_prompt}
                ])
                findings.append(llm_result)
                sources_used.append("direct_llm")
            
            # Step 3: Extract new entities from findings
            kg_stats = {}
            if kg and findings:
                if not await self.check_policy("extract_entities", {"source": task}):
                    logger.warning("Policy denied: entity extraction")
                else:
                    combined_text = "\n".join(findings)
                    kg_stats = self.extract_entities_to_kg(kg, combined_text, f"agent_research_{self.id}")
            
            duration_ms = (time.time() - start) * 1000
            output = "\n\n".join(findings) if findings else "No findings available"
            
            self.task_history.append({"task": task, "success": True, "duration_ms": duration_ms})
            self.state = AgentState.COMPLETE
            
            return AgentResult(
                agent_id=self.id, role=self.role, task=task, output=output,
                metadata={
                    "sources": sources_used,
                    "kg_context": kg_context[:500] if kg_context else "",
                    "kg_stats": kg_stats,
                    "reasoning_traces": reasoning_traces,
                    "findings_count": len(findings),
                },
                duration_ms=duration_ms, success=True,
            )
        except Exception as e:
            self.state = AgentState.FAILED
            logger.error(f"Researcher agent failed: {e}")
            return AgentResult(
                agent_id=self.id, role=self.role, task=task, output="",
                error=str(e), duration_ms=(time.time()-start)*1000, success=False,
            )


class CoderAgent(BaseAgent):
    """Coding agent — uses RAG context + KG for code generation."""

    async def execute(self, task: str, context: Dict) -> AgentResult:
        start = time.time()
        self.state = AgentState.EXECUTING
        
        try:
            rag_pipeline = context.get("rag_pipeline")
            kg = context.get("kg")
            workspace = context.get("workspace", "/workspace")
            
            # Policy check
            if not await self.check_policy("read", {"task": task}):
                return AgentResult(
                    agent_id=self.id, role=self.role, task=task, output="",
                    error="Policy denied: read", duration_ms=(time.time()-start)*1000, success=False,
                )
            
            # Step 1: Query RAG for relevant code patterns
            code_context = ""
            if rag_pipeline:
                rag_result = self.query_rag(rag_pipeline, f"code examples for: {task}", top_k=3)
                if rag_result:
                    code_context = rag_result.answer
            
            # Step 2: Get KG context for tech stack info
            kg_context = self.get_kg_context(kg, task)
            
            # Step 3: Build prompt with all context
            prompt = f"""{self.system_prompt}

## Task
{task}

## Context from Knowledge Base
{code_context}

## Personal Context (Knowledge Graph)
{kg_context}

## Workspace
{workspace}

Provide a complete, working implementation. Include:
1. Code with comments
2. Error handling
3. Usage examples"""
            
            code_output = await self._call_llm([
                {"role": "system", "content": self.system_prompt},
                {"role": "user", "content": prompt}
            ])
            
            # Step 4: Write to workspace if path specified
            output_path = context.get("output_path")
            if output_path and code_output:
                if await self.check_policy("write", {"path": output_path}):
                    os.makedirs(os.path.dirname(output_path) or ".", exist_ok=True)
                    with open(output_path, "w", encoding="utf-8") as f:
                        f.write(code_output)
            
            duration_ms = (time.time() - start) * 1000
            
            self.task_history.append({"task": task, "success": True, "duration_ms": duration_ms})
            self.state = AgentState.COMPLETE
            
            return AgentResult(
                agent_id=self.id, role=self.role, task=task, output=code_output,
                metadata={
                    "workspace": workspace,
                    "output_path": output_path,
                    "kg_context_used": bool(kg_context),
                    "rag_context_used": bool(code_context),
                },
                duration_ms=duration_ms, success=True,
            )
        except Exception as e:
            self.state = AgentState.FAILED
            logger.error(f"Coder agent failed: {e}")
            return AgentResult(
                agent_id=self.id, role=self.role, task=task, output="",
                error=str(e), duration_ms=(time.time()-start)*1000, success=False,
            )
    
class ReviewerAgent(BaseAgent):
    """Review agent — evaluates against KG history + RAG standards."""

    async def execute(self, task: str, context: Dict) -> AgentResult:
        start = time.time()
        self.state = AgentState.EXECUTING
        
        try:
            kg = context.get("kg")
            rag_pipeline = context.get("rag_pipeline")
            content = context.get("content", "")
            criteria = context.get("criteria", ["correctness", "quality", "security"])
            
            # Policy check
            if not await self.check_policy("evaluate", {"task": task}):
                return AgentResult(
                    agent_id=self.id, role=self.role, task=task, output="",
                    error="Policy denied: evaluate", duration_ms=(time.time()-start)*1000, success=False,
                )
            
            # Step 1: Query RAG for best practices
            standards_context = ""
            if rag_pipeline:
                standards_result = self.query_rag(rag_pipeline, f"best practices for {task}", top_k=3)
                if standards_result:
                    standards_context = standards_result.answer
            
            # Step 2: Get KG history for past decisions
            kg_history = ""
            if kg:
                kg_context = self.get_kg_context(kg, task)
                kg_history = kg_context[:500] if kg_context else ""
            
            # Step 3: Evaluate content via LLM
            review_prompt = f"""Evaluate the following content against these criteria: {criteria}

Content:
{content}

Best practices context:
{standards_context}

Past decisions (KG history):
{kg_history}

Provide a structured review as JSON with fields: score (1-10), criteria_scores (object), strengths (list), issues (list), suggestions (list), verdict ("approve"/"comment"/"request_changes"/"reject")."""
            review_result = await self._call_llm([
                {"role": "system", "content": "You are a code review agent. Evaluate content for correctness, quality, and security."},
                {"role": "user", "content": review_prompt}
            ])
            try:
                review = json.loads(review_result)
            except json.JSONDecodeError:
                review = {"score": 5, "criteria_scores": {}, "strengths": [], "issues": ["Could not parse structured review"], "suggestions": [], "verdict": "pending", "raw_review": review_result}
            
            duration_ms = (time.time() - start) * 1000
            
            self.task_history.append({"task": task, "success": True, "duration_ms": duration_ms})
            self.state = AgentState.COMPLETE
            
            return AgentResult(
                agent_id=self.id, role=self.role, task=task, output=json.dumps(review, indent=2),
                metadata=review,
                duration_ms=duration_ms, success=True,
            )
        except Exception as e:
            self.state = AgentState.FAILED
            logger.error(f"Reviewer agent failed: {e}")
            return AgentResult(
                agent_id=self.id, role=self.role, task=task, output="",
                error=str(e), duration_ms=(time.time()-start)*1000, success=False,
            )
    
    def _evaluate_content_fallback(self, content: str, criteria: List[str], standards: str, kg_history: str) -> Dict:
        """Fallback evaluation when LLM is unavailable."""
        review = {
            "score": 0,
            "criteria_scores": {},
            "strengths": [],
            "issues": [],
            "suggestions": [],
            "verdict": "pending",
        }
        
        if not content:
            review["verdict"] = "reject"
            review["issues"].append("No content to review")
            return review
        
        # Basic heuristic evaluation
        lines = content.split("\n")
        has_error_handling = any("except" in l or "try" in l or "if" in l for l in lines)
        has_comments = any("#" in l or "//" in l for l in lines)
        has_docstrings = any('"""' in l or "'''" in l for l in lines)
        
        if "correctness" in criteria:
            score = 5
            if has_error_handling: score += 2
            if has_comments: score += 1
            review["criteria_scores"]["correctness"] = min(score, 10)
        
        if "quality" in criteria:
            score = 5
            if has_docstrings: score += 2
            if has_comments: score += 1
            if len(lines) > 5: score += 1
            review["criteria_scores"]["quality"] = min(score, 10)
        
        if "security" in criteria:
            score = 5
            if has_error_handling: score += 2
            review["criteria_scores"]["security"] = min(score, 10)
        
        review["score"] = int(sum(review["criteria_scores"].values()) / max(len(review["criteria_scores"]), 1))
        
        if review["score"] >= 8:
            review["verdict"] = "approve"
        elif review["score"] >= 5:
            review["verdict"] = "comment"
        else:
            review["verdict"] = "request_changes"
        
        if has_error_handling:
            review["strengths"].append("Includes error handling")
        if has_comments:
            review["strengths"].append("Well commented")
        if has_docstrings:
            review["strengths"].append("Includes docstrings")
        
        return review


class PlannerAgent(BaseAgent):
    """Planning agent — decomposes tasks, coordinates multi-agent workflows via LLM."""

    async def execute(self, task: str, context: Dict) -> AgentResult:
        start = time.time()
        self.state = AgentState.EXECUTING
        
        try:
            available_agents = context.get("available_agents", [])
            kg = context.get("kg")
            
            # Policy check
            if not await self.check_policy("coordinate", {"task": task}):
                return AgentResult(
                    agent_id=self.id, role=self.role, task=task, output="",
                    error="Policy denied: coordinate", duration_ms=(time.time()-start)*1000, success=False,
                )
            
            # Step 1: Get KG context for similar past tasks
            kg_context = self.get_kg_context(kg, task)
            
            # Step 2: Decompose task into steps via LLM
            plan_prompt = f"""Decompose this task into steps and assign each to the most appropriate agent.

Task: {task}
Available agents: {available_agents}
Knowledge Graph context: {kg_context}

Return a JSON plan with: original_task, steps (array with id, description, agent, depends_on, estimated_ms), total_steps."""
            plan_result = await self._call_llm([
                {"role": "system", "content": "You are a planning agent. Break down complex tasks into actionable steps with agent assignments."},
                {"role": "user", "content": plan_prompt}
            ])
            try:
                plan = json.loads(plan_result)
            except json.JSONDecodeError:
                plan = {"original_task": task, "steps": [], "kg_context_used": bool(kg_context), "error": "Parse failed", "raw_plan": plan_result}
            if "original_task" not in plan:
                plan["original_task"] = task
            if "steps" not in plan:
                plan["steps"] = [{"id": 1, "description": f"Process: {task}", "agent": available_agents[0] if available_agents else "researcher", "depends_on": [], "estimated_ms": 30000}]
            plan["kg_context_used"] = bool(kg_context)
            
            duration_ms = (time.time() - start) * 1000
            
            self.task_history.append({"task": task, "success": True, "duration_ms": duration_ms})
            self.state = AgentState.COMPLETE
            
            return AgentResult(
                agent_id=self.id, role=self.role, task=task, output=json.dumps(plan, indent=2),
                metadata=plan,
                duration_ms=duration_ms, success=True,
            )
        except Exception as e:
            self.state = AgentState.FAILED
            logger.error(f"Planner agent failed: {e}")
            return AgentResult(
                agent_id=self.id, role=self.role, task=task, output="",
                error=str(e), duration_ms=(time.time()-start)*1000, success=False,
            )
    
    def _decompose_task_fallback(self, task: str, available_agents: List[str], kg_context: str) -> Dict:
        """Fallback plan decomposition when LLM is unavailable."""
        task_lower = task.lower()
        
        steps = []
        step_id = 0
        
        if any(kw in task_lower for kw in ["how", "what", "explain", "research", "find", "analyze"]):
            step_id += 1
            steps.append({
                "id": step_id,
                "description": f"Research: {task}",
                "agent": "researcher",
                "depends_on": [],
                "estimated_ms": 30000,
            })
        
        if any(kw in task_lower for kw in ["code", "implement", "write", "create", "build", "script"]):
            step_id += 1
            steps.append({
                "id": step_id,
                "description": f"Implement: {task}",
                "agent": "coder",
                "depends_on": [s["id"] for s in steps] if steps else [],
                "estimated_ms": 60000,
            })
        
        if steps:
            step_id += 1
            steps.append({
                "id": step_id,
                "description": f"Review output for: {task}",
                "agent": "reviewer",
                "depends_on": [steps[-1]["id"]],
                "estimated_ms": 15000,
            })
        
        if not steps:
            steps = [
                {"id": 1, "description": f"Analyze: {task}", "agent": "researcher", "depends_on": [], "estimated_ms": 30000},
                {"id": 2, "description": f"Review findings", "agent": "reviewer", "depends_on": [1], "estimated_ms": 15000},
            ]
        
        return {
            "original_task": task,
            "steps": steps,
            "total_steps": len(steps),
            "estimated_duration_ms": sum(s["estimated_ms"] for s in steps),
            "kg_context_used": bool(kg_context),
        }


def create_agent(role: str, model: str, system_prompt: str, opa_gate: Optional[OPAPolicyGate] = None, router: Optional[ModelRouter] = None) -> BaseAgent:
    """Factory function to create agents by role."""
    agents = {
        "researcher": ResearcherAgent,
        "coder": CoderAgent,
        "reviewer": ReviewerAgent,
        "planner": PlannerAgent,
    }
    agent_class = agents.get(role)
    if not agent_class:
        raise ValueError(f"Unknown agent role: {role}")
    return agent_class(role, model, system_prompt, opa_gate, router=router)
