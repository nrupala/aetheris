"""
Reasoning Loop — Iterative self-improving RAG with Pregel checkpointing.

Architecture:
    Question → Iterate N times → Self-Verify → Converge
    Temperature annealing: 0.8 (explore) → 0.5 (refine) → 0.1 (converge)
    Pregel checkpoints: save state after each iteration, resume on crash

Inspired by:
- MAgICoRe (multi-agent iterative coarse-to-fine refinement)
- Hegelian Dialectic (thesis → antithesis → synthesis)
- SCoRe (self-correction via multi-turn RL)
- LangGraph (Pregel-inspired checkpointing)
"""

import os
import time
import logging
from typing import List, Optional, Dict, Any, Tuple
from dataclasses import dataclass, field

from .config import RAGConfig, config
from .model_router import (
    ModelRouter, create_default_router, Agent,
    AnswerWithConfidence, SelfVerification, EntityExtract,
)
from .pregel_checkpoint import (
    PregelCheckpoint, CheckpointState, CheckpointManager,
)
from .retriever import RetrievalResult

logger = logging.getLogger(__name__)


@dataclass
class ReasoningResult:
    """Final result from the reasoning loop."""
    answer: str
    confidence: float
    iterations_used: int
    max_iterations: int
    temperature_schedule: List[float]
    reasoning_trace: List[Dict]  # Each iteration's reasoning
    sources_used: List[str]
    tokens_used: int
    model: str
    converged: bool
    checkpoint_id: Optional[str] = None
    verification: Dict[str, Any] = field(default_factory=dict)


# --- Temperature Annealing Schedule ---

def get_temperature_schedule(max_iterations: int, start_temp: float = 0.8) -> List[float]:
    """
    Generate temperature annealing schedule.
    High T for exploration → low T for convergence.
    
    Hegelian Dialectic:
    - Iteration 1 (thesis): High temperature, diverse approaches
    - Iteration 2 (antithesis): Medium temperature, refine direction
    - Iteration 3+ (synthesis): Low temperature, converge to answer
    """
    if max_iterations <= 1:
        return [0.1]
    
    temps = []
    for i in range(max_iterations):
        if i == 0:
            temps.append(start_temp)
        elif i == max_iterations - 1:
            temps.append(0.1)
        else:
            # Linear interpolation
            progress = i / (max_iterations - 1)
            temps.append(start_temp * (1 - progress) + 0.1 * progress)
    
    return temps


# --- Reasoning Loop ---

class ReasoningLoop:
    """
    Iterative reasoning with self-verification and Pregel checkpointing.
    
    Usage:
        loop = ReasoningLoop(agent, checkpoint_dir="/workspace/processing/task-123")
        result = loop.run(
            question="How do I configure WireGuard?",
            context=[retrieved_chunks],
            max_iterations=3,
            confidence_threshold=0.7,
        )
    """
    
    def __init__(
        self,
        agent: Agent,
        task_id: str,
        checkpoint_dir: Optional[str] = None,
        workspace_root: Optional[str] = None,
    ):
        self.agent = agent
        self.task_id = task_id
        
        # Initialize checkpointing
        if checkpoint_dir:
            self.checkpoint = PregelCheckpoint(checkpoint_dir)
        elif workspace_root:
            task_dir = os.path.join(workspace_root, "processing", task_id)
            self.checkpoint = PregelCheckpoint(task_dir)
        else:
            self.checkpoint = None
        
        self._system_prompt = """You are Aetheris, a sovereign AI assistant.
Think step by step. Consider multiple approaches.
Answer based on the provided context. If context is insufficient, say so clearly.
Never fabricate information."""
    
    def run(
        self,
        question: str,
        context: List[RetrievalResult],
        max_iterations: int = 3,
        confidence_threshold: float = 0.7,
        temperature_start: float = 0.8,
        parent_checkpoint_id: Optional[str] = None,
    ) -> ReasoningResult:
        """
        Run the reasoning loop.
        
        Returns the converged answer or best attempt after max iterations.
        """
        context_str = self._format_context(context)
        schedule = get_temperature_schedule(max_iterations, temperature_start)
        reasoning_trace = []
        total_tokens = 0
        last_checkpoint_id = parent_checkpoint_id
        
        # Check for existing checkpoint (crash recovery)
        start_iteration = 0
        if self.checkpoint and parent_checkpoint_id is None:
            existing = self.checkpoint.get_latest()
            if existing and not existing.is_final:
                logger.info(f"ReasoningLoop: resuming from iteration {existing.state.iteration}")
                start_iteration = existing.state.iteration
                last_checkpoint_id = existing.checkpoint_id
        
        current_answer = ""
        current_confidence = 0.0
        
        for iteration in range(start_iteration, max_iterations):
            temperature = schedule[iteration] if iteration < len(schedule) else 0.1
            
            # Generate answer for this iteration
            start_time = time.time()
            answer, confidence, reasoning, sources, tokens = self._generate_iteration(
                question=question,
                context=context_str,
                previous_answer=current_answer,
                previous_confidence=current_confidence,
                temperature=temperature,
                iteration=iteration,
                max_iterations=max_iterations,
            )
            latency = (time.time() - start_time) * 1000
            total_tokens += tokens
            
            # Self-verify
            verification = self._self_verify(question, answer, context_str)
            
            # Record iteration
            iteration_record = {
                "iteration": iteration + 1,
                "temperature": temperature,
                "answer": answer,
                "confidence": confidence,
                "reasoning": reasoning,
                "verification": {
                    "is_correct": verification.is_correct,
                    "confidence": verification.confidence,
                    "issues": verification.issues,
                },
                "tokens": tokens,
                "latency_ms": round(latency, 1),
            }
            reasoning_trace.append(iteration_record)
            
            current_answer = answer
            current_confidence = confidence
            
            # Save checkpoint
            if self.checkpoint:
                state = CheckpointState(
                    iteration=iteration + 1,
                    answer=answer,
                    confidence=confidence,
                    temperature=temperature,
                    reasoning=reasoning,
                    verification={
                        "is_correct": verification.is_correct,
                        "confidence": verification.confidence,
                        "issues": verification.issues,
                    },
                    sources_used=sources,
                    tokens_used=total_tokens,
                    timestamp="",
                )
                cp = self.checkpoint.save(
                    state=state,
                    task_id=self.task_id,
                    parent_id=last_checkpoint_id,
                    is_final=False,
                )
                last_checkpoint_id = cp.checkpoint_id
            
            # Check convergence
            if confidence >= confidence_threshold and verification.confidence >= confidence_threshold:
                # Converged — mark as final
                if self.checkpoint:
                    final_state = CheckpointState(
                        iteration=iteration + 1,
                        answer=answer,
                        confidence=confidence,
                        temperature=temperature,
                        reasoning=reasoning,
                        verification={
                            "is_correct": verification.is_correct,
                            "confidence": verification.confidence,
                            "issues": verification.issues,
                        },
                        sources_used=sources,
                        tokens_used=total_tokens,
                    )
                    self.checkpoint.save(
                        state=final_state,
                        task_id=self.task_id,
                        parent_id=last_checkpoint_id,
                        is_final=True,
                    )
                
                return ReasoningResult(
                    answer=answer,
                    confidence=confidence,
                    iterations_used=iteration + 1,
                    max_iterations=max_iterations,
                    temperature_schedule=schedule[:iteration + 1],
                    reasoning_trace=reasoning_trace,
                    sources_used=sources,
                    tokens_used=total_tokens,
                    model="phi-4-reasoning-plus",
                    converged=True,
                    checkpoint_id=last_checkpoint_id,
                    verification={
                        "is_correct": verification.is_correct,
                        "confidence": verification.confidence,
                        "issues": verification.issues,
                        "suggestions": verification.suggestions,
                    },
                )
        
        # Did not converge — return best attempt
        final_cp_id = None
        if self.checkpoint:
            final_state = CheckpointState(
                iteration=max_iterations,
                answer=current_answer,
                confidence=current_confidence,
                temperature=schedule[-1] if schedule else 0.1,
                reasoning=reasoning_trace[-1]["reasoning"] if reasoning_trace else "",
                verification={
                    "is_correct": verification.is_correct,
                    "confidence": verification.confidence,
                    "issues": verification.issues,
                },
                sources_used=reasoning_trace[-1].get("sources", []) if reasoning_trace else [],
                tokens_used=total_tokens,
            )
            cp = self.checkpoint.save(
                state=final_state,
                task_id=self.task_id,
                parent_id=last_checkpoint_id,
                is_final=True,
            )
            final_cp_id = cp.checkpoint_id
        
        return ReasoningResult(
            answer=current_answer,
            confidence=current_confidence,
            iterations_used=max_iterations,
            max_iterations=max_iterations,
            temperature_schedule=schedule,
            reasoning_trace=reasoning_trace,
            sources_used=reasoning_trace[-1].get("sources", []) if reasoning_trace else [],
            tokens_used=total_tokens,
            model="phi-4-reasoning-plus",
            converged=False,
            checkpoint_id=final_cp_id,
            verification={
                "is_correct": verification.is_correct,
                "confidence": verification.confidence,
                "issues": verification.issues,
                "suggestions": verification.suggestions,
            },
        )
    
    def _generate_iteration(
        self,
        question: str,
        context: str,
        previous_answer: str,
        previous_confidence: float,
        temperature: float,
        iteration: int,
        max_iterations: int,
    ) -> Tuple[str, float, str, List[str], int]:
        """Generate one iteration of reasoning."""
        prompt = self._build_iteration_prompt(
            question, context, previous_answer, previous_confidence, iteration, max_iterations
        )
        
        messages = [
            {"role": "system", "content": self._system_prompt},
            {"role": "user", "content": prompt},
        ]
        
        response = self.agent.chat(
            messages=messages,
            temperature=temperature,
            max_tokens=2048,
            tools=False,
        )
        
        # Parse response (expect structured format)
        answer, confidence, reasoning, sources = self._parse_iteration_response(response.text)
        
        return answer, confidence, reasoning, sources, response.tokens_out
    
    def _build_iteration_prompt(
        self,
        question: str,
        context: str,
        previous_answer: str,
        previous_confidence: float,
        iteration: int,
        max_iterations: int,
    ) -> str:
        """Build the prompt for one reasoning iteration."""
        if iteration == 0:
            return f"""Context:
{context}

Question: {question}

Provide your initial answer. Think step by step.
Include:
1. Your answer
2. Confidence level (0.0-1.0)
3. Your reasoning process
4. Sources you referenced"""
        else:
            return f"""Context:
{context}

Question: {question}

Previous Answer (iteration {iteration}, confidence: {previous_confidence:.2f}):
{previous_answer}

This is iteration {iteration + 1} of {max_iterations}.
Review your previous answer. Identify any gaps, inaccuracies, or areas for improvement.
Provide a refined answer.

Include:
1. Your refined answer
2. Updated confidence level (0.0-1.0)
3. What you changed and why
4. Sources you referenced"""
    
    def _parse_iteration_response(self, text: str) -> Tuple[str, float, str, List[str]]:
        """Parse the iteration response into components."""
        # Simple parsing — in production, use structured output
        import re
        
        # Try to extract confidence
        confidence = 0.5
        conf_match = re.search(r'[Cc]onfidence[:\s]+([0-9.]+)', text)
        if conf_match:
            try:
                confidence = float(conf_match.group(1))
                confidence = max(0.0, min(1.0, confidence))
            except ValueError:
                pass
        
        # Extract answer (everything before "Reasoning:" or "Sources:")
        answer = text
        reasoning = ""
        sources = []
        
        # Split by sections if present
        if "Reasoning:" in text or "reasoning:" in text:
            parts = re.split(r'[Rr]easoning:', text, 1)
            answer = parts[0].strip()
            if len(parts) > 1:
                remaining = parts[1]
                if "Sources:" in remaining or "sources:" in remaining:
                    reasoning, sources_str = re.split(r'[Ss]ources:', remaining, 1)
                    reasoning = reasoning.strip()
                    sources = [s.strip().strip('-').strip('[]').strip() for s in sources_str.split('\n') if s.strip()]
                else:
                    reasoning = remaining.strip()
        
        # Clean up answer
        answer = answer.strip()
        if not answer:
            answer = text[:500]  # Fallback
        
        return answer, confidence, reasoning, sources
    
    def _self_verify(self, question: str, answer: str, context: str) -> SelfVerification:
        """Self-verify the answer against context."""
        try:
            return self.agent.verify_answer(question, answer, context)
        except Exception as e:
            logger.warning(f"ReasoningLoop: self-verification failed: {e}")
            # Fallback: return neutral verification
            return SelfVerification(
                is_correct=True,
                confidence=0.5,
                issues=["Verification skipped due to error"],
                suggestions=[],
            )
    
    def _format_context(self, results: List[RetrievalResult]) -> str:
        """Format retrieved chunks into context string."""
        if not results:
            return "(No relevant context found)"
        
        parts = []
        for i, r in enumerate(results, 1):
            source_display = r.source
            if r.metadata.get("section"):
                source_display += f" → {r.metadata['section']}"
            parts.append(f"### Source: {source_display} (relevance: {r.score:.2f})\n{r.text}\n")
        return "\n---\n".join(parts)


# --- Entity Extraction for KG ---

class EntityExtractor:
    """
    Extract entities and relations from documents during ingest.
    Integrates with the Knowledge Graph.
    """
    
    def __init__(self, agent: Agent):
        self.agent = agent
    
    def extract_from_text(self, text: str, source: str = "") -> Dict:
        """
        Extract entities and relations from text.
        Returns dict compatible with KnowledgeGraph.add_entity/add_relation.
        """
        # Chunk long texts to fit context window
        max_chunk_size = 8000  # tokens
        chunks = [text[i:i+max_chunk_size] for i in range(0, len(text), max_chunk_size)]
        
        all_entities = []
        all_relations = []
        summaries = []
        
        for chunk in chunks:
            try:
                result = self.agent.extract_entities(chunk, source)
                all_entities.extend(result.entities)
                all_relations.extend(result.relations)
                summaries.append(result.summary)
            except Exception as e:
                logger.warning(f"EntityExtractor: extraction failed for chunk: {e}")
        
        # Deduplicate entities by name
        entity_map = {}
        for e in all_entities:
            name = e.get("name", "").strip()
            if name:
                if name not in entity_map or e.get("importance", 0) > entity_map[name].get("importance", 0):
                    entity_map[name] = e
        
        # Deduplicate relations by (source, target, type)
        relation_set = set()
        unique_relations = []
        for r in all_relations:
            key = (r.get("source", ""), r.get("target", ""), r.get("type", ""))
            if key not in relation_set and key[0] and key[1]:
                relation_set.add(key)
                unique_relations.append(r)
        
        return {
            "entities": list(entity_map.values()),
            "relations": unique_relations,
            "summary": " ".join(summaries) if summaries else "",
            "source": source,
        }
    
    def ingest_to_kg(self, text: str, source: str, kg) -> Dict:
        """
        Extract entities/relations and ingest directly into KnowledgeGraph.
        
        Returns summary of what was added.
        """
        result = self.extract_from_text(text, source)
        
        entities_added = 0
        relations_added = 0
        
        for entity in result["entities"]:
            try:
                kg.add_entity(
                    name=entity.get("name", ""),
                    entity_type=entity.get("type", "concept"),
                    description=entity.get("description", ""),
                    source=source,
                    importance=entity.get("importance", 1.0),
                )
                entities_added += 1
            except Exception as e:
                logger.warning(f"EntityExtractor: failed to add entity {entity.get('name')}: {e}")
        
        for relation in result["relations"]:
            try:
                kg.add_relation(
                    source=relation.get("source", ""),
                    target=relation.get("target", ""),
                    relation_type=relation.get("type", "related_to"),
                    weight=relation.get("weight", 1.0),
                    context=relation.get("context", ""),
                )
                relations_added += 1
            except Exception as e:
                logger.warning(f"EntityExtractor: failed to add relation: {e}")
        
        # Set document context
        if result["summary"]:
            kg.set_document_context(
                source=source,
                summary=result["summary"],
                key_concepts=[e.get("name") for e in result["entities"][:10]],
                related_entities=[e.get("name") for e in result["entities"][:5]],
            )
        
        return {
            "source": source,
            "entities_added": entities_added,
            "relations_added": relations_added,
            "total_entities": len(result["entities"]),
            "total_relations": len(result["relations"]),
        }
