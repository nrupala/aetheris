"""
Model Abstraction Layer — Pydantic AI wrapper for all LLM calls.

Provides:
- Model-agnostic routing (LMStudio, GPT4All, OpenAI, Anthropic, etc.)
- Automatic fallback chain (primary → secondary → CPU fallback)
- Structured output support via Pydantic models
- MCP (Model Context Protocol) tool integration
- Type-safe tool definitions

Architecture:
    LMStudio (primary, GPU) → GPT4All (fallback, CPU) → Error

No LangChain. Clean Pydantic AI abstractions only.
"""

import json
import time
import logging
import requests
from typing import List, Optional, Dict, Any, Type, Iterator
from dataclasses import dataclass, field
from enum import Enum
from pydantic import BaseModel, Field

logger = logging.getLogger(__name__)


# --- Provider Types ---

class Provider(str, Enum):
    LMSTUDIO = "lmstudio"
    GPT4ALL = "gpt4all"
    OLLAMA = "ollama"
    OPENAI = "openai"
    ANTHROPIC = "anthropic"
    CUSTOM = "custom"


class ModelCapability(str, Enum):
    CHAT = "chat"
    EMBEDDING = "embedding"
    VISION = "vision"
    STRUCTURED_OUTPUT = "structured_output"
    TOOL_USE = "tool_use"


@dataclass
class ModelInfo:
    """Model metadata."""
    name: str
    provider: Provider
    endpoint: str
    capabilities: List[ModelCapability] = field(default_factory=list)
    api_key: str = ""
    max_tokens: int = 4096
    context_window: int = 8192
    priority: int = 0  # 0 = primary, higher = fallback


@dataclass
class ModelResponse:
    """Unified response from any provider."""
    text: str
    model: str
    provider: Provider
    tokens_in: int = 0
    tokens_out: int = 0
    latency_ms: float = 0
    finish_reason: str = "stop"
    raw: dict = field(default_factory=dict)


@dataclass
class ToolDefinition:
    """MCP-style tool definition for LLM function calling."""
    name: str
    description: str
    parameters: dict  # JSON Schema
    handler: callable = None


# --- Model Router ---

class ModelRouter:
    """
    Routes LLM requests through a chain of providers with automatic fallback.
    
    Primary → Secondary → ... → CPU Fallback → Error
    
    Tracks which provider succeeded for each request type.
    """
    
    def __init__(self, models: List[ModelInfo]):
        self.models = sorted(models, key=lambda m: m.priority)
        self._stats: Dict[str, Dict] = {}  # model_name → {success, failure, latency}
    
    def chat(
        self,
        messages: List[Dict[str, str]],
        temperature: float = 0.1,
        max_tokens: int = 2048,
        stream: bool = False,
        tools: List[ToolDefinition] = None,
        response_format: Type[BaseModel] = None,
    ) -> ModelResponse:
        """
        Send chat request through provider chain.
        Falls back on failure.
        """
        last_error = None
        
        for model in self.models:
            start = time.time()
            try:
                response = self._call_provider(
                    model=model,
                    messages=messages,
                    temperature=temperature,
                    max_tokens=max_tokens,
                    stream=stream,
                    tools=tools,
                    response_format=response_format,
                )
                self._record_success(model.name, time.time() - start)
                return response
            except Exception as e:
                last_error = e
                self._record_failure(model.name, time.time() - start)
                logger.warning(
                    f"ModelRouter: {model.provider.value}/{model.name} failed: {e}"
                )
                if model.priority == 0:
                    logger.info(f"ModelRouter: falling back from primary {model.name}")
        
        raise ModelRouterError(
            f"All providers failed. Last error: {last_error}"
        ) from last_error
    
    def _call_provider(
        self,
        model: ModelInfo,
        messages: List[Dict[str, str]],
        temperature: float,
        max_tokens: int,
        stream: bool,
        tools: List[ToolDefinition],
        response_format: Type[BaseModel],
    ) -> ModelResponse:
        """Dispatch to provider-specific implementation."""
        if model.provider == Provider.LMSTUDIO:
            return self._call_openai_compatible(model, messages, temperature, max_tokens, stream, tools, response_format)
        elif model.provider == Provider.GPT4ALL:
            return self._call_openai_compatible(model, messages, temperature, max_tokens, stream, tools, response_format)
        elif model.provider == Provider.OLLAMA:
            return self._call_openai_compatible(model, messages, temperature, max_tokens, stream, tools, response_format)
        elif model.provider == Provider.OPENAI:
            return self._call_openai_compatible(model, messages, temperature, max_tokens, stream, tools, response_format)
        elif model.provider == Provider.ANTHROPIC:
            return self._call_anthropic(model, messages, temperature, max_tokens, stream, tools)
        else:
            return self._call_openai_compatible(model, messages, temperature, max_tokens, stream, tools, response_format)
    
    def _call_openai_compatible(
        self,
        model: ModelInfo,
        messages: List[Dict[str, str]],
        temperature: float,
        max_tokens: int,
        stream: bool,
        tools: List[ToolDefinition],
        response_format: Type[BaseModel],
    ) -> ModelResponse:
        """Call OpenAI-compatible API (LMStudio, GPT4All, Ollama, OpenAI)."""
        payload = {
            "model": model.name,
            "messages": messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
            "stream": stream,
        }
        
        if tools:
            payload["tools"] = [self._tool_to_openai_format(t) for t in tools]
        
        if response_format:
            payload["response_format"] = {
                "type": "json_schema",
                "json_schema": response_format.model_json_schema(),
            }
        
        headers = {"Content-Type": "application/json"}
        if model.api_key:
            headers["Authorization"] = f"Bearer {model.api_key}"
        
        resp = requests.post(
            f"{model.endpoint}/v1/chat/completions",
            json=payload,
            headers=headers,
            timeout=120,
        )
        
        if resp.status_code != 200:
            raise ProviderError(
                f"{model.provider.value} returned {resp.status_code}: {resp.text}"
            )
        
        data = resp.json()
        choice = data["choices"][0]
        usage = data.get("usage", {})
        
        return ModelResponse(
            text=choice["message"]["content"],
            model=model.name,
            provider=model.provider,
            tokens_in=usage.get("prompt_tokens", 0),
            tokens_out=usage.get("completion_tokens", 0),
            finish_reason=choice.get("finish_reason", "stop"),
            raw=data,
        )
    
    def _call_anthropic(
        self,
        model: ModelInfo,
        messages: List[Dict[str, str]],
        temperature: float,
        max_tokens: int,
        stream: bool,
        tools: List[ToolDefinition],
    ) -> ModelResponse:
        """Call Anthropic Claude API."""
        system_msg = None
        user_messages = []
        for msg in messages:
            if msg["role"] == "system":
                system_msg = msg["content"]
            else:
                user_messages.append(msg)
        
        payload = {
            "model": model.name,
            "messages": user_messages,
            "temperature": temperature,
            "max_tokens": max_tokens,
        }
        
        if system_msg:
            payload["system"] = system_msg
        
        if tools:
            payload["tools"] = [self._tool_to_anthropic_format(t) for t in tools]
        
        headers = {
            "Content-Type": "application/json",
            "x-api-key": model.api_key,
            "anthropic-version": "2023-06-01",
        }
        
        resp = requests.post(
            f"{model.endpoint}/v1/messages",
            json=payload,
            headers=headers,
            timeout=120,
        )
        
        if resp.status_code != 200:
            raise ProviderError(
                f"Anthropic returned {resp.status_code}: {resp.text}"
            )
        
        data = resp.json()
        return ModelResponse(
            text=data["content"][0]["text"],
            model=model.name,
            provider=model.provider,
            tokens_in=data.get("usage", {}).get("input_tokens", 0),
            tokens_out=data.get("usage", {}).get("output_tokens", 0),
            finish_reason=data.get("stop_reason", "end_turn"),
            raw=data,
        )
    
    def _tool_to_openai_format(self, tool: ToolDefinition) -> dict:
        return {
            "type": "function",
            "function": {
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            },
        }
    
    def _tool_to_anthropic_format(self, tool: ToolDefinition) -> dict:
        return {
            "name": tool.name,
            "description": tool.description,
            "input_schema": tool.parameters,
        }
    
    def _record_success(self, model_name: str, latency: float):
        if model_name not in self._stats:
            self._stats[model_name] = {"success": 0, "failure": 0, "latency_total": 0}
        self._stats[model_name]["success"] += 1
        self._stats[model_name]["latency_total"] += latency
    
    def _record_failure(self, model_name: str, latency: float):
        if model_name not in self._stats:
            self._stats[model_name] = {"success": 0, "failure": 0, "latency_total": 0}
        self._stats[model_name]["failure"] += 1
    
    def get_stats(self) -> Dict:
        return self._stats


# --- Structured Output Models ---

class EntityExtract(BaseModel):
    """Structured output for entity extraction from documents."""
    entities: List[Dict[str, Any]] = Field(
        description="List of entities with name, type, description, importance"
    )
    relations: List[Dict[str, Any]] = Field(
        description="List of relations with source, target, type, weight"
    )
    summary: str = Field(description="Brief summary of the document")


class AnswerWithConfidence(BaseModel):
    """Structured output for reasoning loop answers."""
    answer: str = Field(description="The answer to the question")
    confidence: float = Field(
        ge=0.0, le=1.0,
        description="Confidence score (0.0-1.0)"
    )
    reasoning: str = Field(description="Explanation of how the answer was derived")
    sources_used: List[str] = Field(description="List of sources referenced")
    needs_iteration: bool = Field(
        description="Whether the answer needs further refinement"
    )


class SelfVerification(BaseModel):
    """Structured output for self-verification step."""
    is_correct: bool = Field(description="Whether the answer appears correct")
    confidence: float = Field(ge=0.0, le=1.0, description="Verification confidence")
    issues: List[str] = Field(description="List of identified issues or gaps")
    suggestions: List[str] = Field(description="Suggestions for improvement")


# --- MCP Tool Registry ---

class MCPToolRegistry:
    """
    Model Context Protocol tool registry.
    Manages tools available to LLM function calling.
    
    Pattern: Code Mode — fewer broad tools beat many narrow tools.
    """
    
    def __init__(self):
        self._tools: Dict[str, ToolDefinition] = {}
    
    def register(self, tool: ToolDefinition):
        self._tools[tool.name] = tool
    
    def unregister(self, name: str):
        self._tools.pop(name, None)
    
    def get_all(self) -> List[ToolDefinition]:
        return list(self._tools.values())
    
    def get_by_name(self, name: str) -> Optional[ToolDefinition]:
        return self._tools.get(name)
    
    def execute(self, name: str, arguments: dict) -> Any:
        tool = self._tools.get(name)
        if not tool:
            raise ToolNotFoundError(f"Tool not found: {name}")
        if not tool.handler:
            raise ToolError(f"Tool {name} has no handler")
        return tool.handler(**arguments)


# --- High-Level Agent Interface ---

class Agent:
    """
    High-level agent interface using Pydantic AI abstractions.
    
    Wraps ModelRouter + MCPToolRegistry + structured outputs.
    Used by both RAG pipeline and Agent Orchestrator.
    """
    
    def __init__(self, router: ModelRouter, tool_registry: MCPToolRegistry = None):
        self.router = router
        self.tool_registry = tool_registry or MCPToolRegistry()
    
    def chat(
        self,
        messages: List[Dict[str, str]],
        temperature: float = 0.1,
        max_tokens: int = 2048,
        tools: bool = True,
    ) -> ModelResponse:
        """Simple chat with optional tool use."""
        tool_list = self.tool_registry.get_all() if tools else []
        return self.router.chat(
            messages=messages,
            temperature=temperature,
            max_tokens=max_tokens,
            tools=tool_list,
        )
    
    def chat_structured(
        self,
        messages: List[Dict[str, str]],
        response_format: Type[BaseModel],
        temperature: float = 0.1,
    ) -> BaseModel:
        """Chat with structured output parsing."""
        resp = self.router.chat(
            messages=messages,
            temperature=temperature,
            max_tokens=4096,
            response_format=response_format,
        )
        
        try:
            return response_format.model_validate_json(resp.text)
        except Exception:
            # Fallback: try to extract JSON from response
            import re
            json_match = re.search(r'\{.*\}', resp.text, re.DOTALL)
            if json_match:
                return response_format.model_validate_json(json_match.group())
            raise StructuredOutputError(
                f"Failed to parse structured output from {resp.model}"
            )
    
    def extract_entities(self, text: str, source: str = "") -> EntityExtract:
        """Extract entities and relations from text using structured output."""
        messages = [
            {
                "role": "system",
                "content": """You are an entity extraction assistant.
Extract entities and relations from the provided text.
Entity types: concept, tool, project, technology, person, file, service, configuration, event
Relation types: depends_on, uses, created_by, related_to, implements, references, contains, located_at, configures, belongs_to
Return valid JSON matching the schema.""",
            },
            {
                "role": "user",
                "content": f"Extract entities and relations from this document:\n\n{text}",
            },
        ]
        
        return self.chat_structured(messages, EntityExtract, temperature=0.0)
    
    def verify_answer(
        self,
        question: str,
        answer: str,
        context: str,
    ) -> SelfVerification:
        """Self-verify an answer against context."""
        messages = [
            {
                "role": "system",
                "content": """You are a self-verification assistant.
Evaluate whether the provided answer correctly addresses the question
based on the given context. Identify any gaps, hallucinations, or inaccuracies.""",
            },
            {
                "role": "user",
                "content": f"""Question: {question}

Context: {context}

Proposed Answer: {answer}

Evaluate this answer for correctness, completeness, and accuracy.""",
            },
        ]
        
        return self.chat_structured(messages, SelfVerification, temperature=0.0)
    
    def get_stats(self) -> Dict:
        return {
            "router": self.router.get_stats(),
            "tools": list(self.tool_registry.get_all()),
        }


# --- Custom Exceptions ---

class ModelRouterError(Exception):
    """All providers failed."""
    pass


class ProviderError(Exception):
    """Individual provider failed."""
    pass


class ToolNotFoundError(Exception):
    """Tool not found in registry."""
    pass


class ToolError(Exception):
    """Tool execution error."""
    pass


class StructuredOutputError(Exception):
    """Failed to parse structured output."""
    pass


# --- Default Configuration ---

def create_default_router(
    ollama_endpoint: str = "http://ollama:11434",
    ollama_model: str = "qwen2.5:14b",
    lmstudio_endpoint: str = "http://localhost:1234",
    lmstudio_model: str = "microsoft/phi-4-reasoning-plus",
) -> ModelRouter:
    """
    Create default model router with Ollama primary.
    """
    models = [
        ModelInfo(
            name=ollama_model,
            provider=Provider.OLLAMA,
            endpoint=ollama_endpoint,
            capabilities=[ModelCapability.CHAT, ModelCapability.STRUCTURED_OUTPUT],
            priority=0,
        ),
        ModelInfo(
            name=lmstudio_model,
            provider=Provider.LMSTUDIO,
            endpoint=lmstudio_endpoint,
            capabilities=[ModelCapability.CHAT, ModelCapability.STRUCTURED_OUTPUT],
            priority=1,
        ),
    ]
    
    return ModelRouter(models)
