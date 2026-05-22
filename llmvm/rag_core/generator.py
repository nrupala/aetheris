"""
Generator — LLM wrapper for OpenAI-compatible APIs (LMStudio).
Handles system prompts, context formatting, streaming, and token limits.

No LangChain. No abstractions over abstractions.
Just requests.post() and parse the JSON.
"""

import json
from typing import List, Optional, Iterator, Dict
from dataclasses import dataclass
from .config import RAGConfig, config
from .retriever import RetrievalResult


@dataclass
class LLMResponse:
    text: str
    usage: Dict
    model: str


class Generator:
    """Generate answers using LMStudio's /v1/chat/completions endpoint."""

    def __init__(self, cfg: Optional[RAGConfig] = None):
        self.cfg = cfg or config

    def _build_system_prompt(self, context: str) -> str:
        """Build the system prompt with RAG context."""
        return f"""You are Aetheris, a sovereign AI assistant with access to a personal knowledge base.

## Context
The following information has been retrieved from your knowledge base:

{context}

## Instructions
- Answer based primarily on the context provided above
- If the context doesn't contain enough information, say so clearly
- Cite sources when referencing specific documents (use [source] format)
- Do not fabricate information not present in the context
- Keep answers concise and actionable
- If asked about file management, use the context to guide your response
- Be direct and avoid filler phrases like "Based on the provided context"

## Knowledge Base Sources
{self._list_sources(context)}
"""

    def _list_sources(self, context: str) -> str:
        """Extract source list from context for transparency."""
        sources = []
        for line in context.split('\n'):
            if line.startswith('### Source:'):
                src = line.replace('### Source:', '').strip()
                if src not in sources:
                    sources.append(src)
        return "\n".join(f"- {s}" for s in sources) if sources else "- (no sources provided)"

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

    def generate(
        self,
        query: str,
        context: List[RetrievalResult],
        history: Optional[List[Dict]] = None
    ) -> LLMResponse:
        """
        Generate a RAG-augmented answer.

        Args:
            query: User's question
            context: Retrieved knowledge chunks
            history: Optional conversation history [{role, content}]

        Returns:
            LLMResponse with text and usage info
        """
        import requests

        context_str = self._format_context(context)
        system_prompt = self._build_system_prompt(context_str)

        messages = [{"role": "system", "content": system_prompt}]

        if history:
            messages.extend(history)

        messages.append({"role": "user", "content": query})

        payload = {
            "model": self.cfg.default_model,
            "messages": messages,
            "temperature": self.cfg.temperature,
            "max_tokens": self.cfg.max_tokens,
            "stream": False
        }

        resp = requests.post(
            f"{self.cfg.ai_endpoint}/v1/chat/completions",
            json=payload,
            headers=self.cfg.headers,
            timeout=self.cfg.request_timeout
        )

        if resp.status_code != 200:
            return LLMResponse(
                text=f"Error: API returned {resp.status_code} - {resp.text}",
                usage={"error": resp.status_code},
                model=self.cfg.default_model
            )

        data = resp.json()
        choice = data["choices"][0]
        usage = data.get("usage", {})

        return LLMResponse(
            text=choice["message"]["content"],
            usage=usage,
            model=data.get("model", self.cfg.default_model)
        )

    def generate_stream(
        self,
        query: str,
        context: List[RetrievalResult],
        history: Optional[List[Dict]] = None
    ) -> Iterator[str]:
        """
        Stream a RAG-augmented response token by token.
        Yields text chunks as they arrive.
        """
        import requests

        context_str = self._format_context(context)
        system_prompt = self._build_system_prompt(context_str)

        messages = [{"role": "system", "content": system_prompt}]
        if history:
            messages.extend(history)
        messages.append({"role": "user", "content": query})

        payload = {
            "model": self.cfg.default_model,
            "messages": messages,
            "temperature": self.cfg.temperature,
            "max_tokens": self.cfg.max_tokens,
            "stream": True
        }

        resp = requests.post(
            f"{self.cfg.ai_endpoint}/v1/chat/completions",
            json=payload,
            headers=self.cfg.headers,
            timeout=self.cfg.request_timeout,
            stream=True
        )

        for line in resp.iter_lines():
            if not line:
                continue
            line = line.decode('utf-8')
            if line.startswith("data: "):
                data_str = line[6:]
                if data_str == "[DONE]":
                    break
                try:
                    data = json.loads(data_str)
                    delta = data["choices"][0].get("delta", {})
                    content = delta.get("content", "")
                    if content:
                        yield content
                except (json.JSONDecodeError, KeyError, IndexError):
                    continue

    def generate_direct(self, query: str, history: Optional[List[Dict]] = None) -> LLMResponse:
        """
        Generate without RAG context (pure LLM, no retrieval).
        Use for general questions not requiring knowledge base.
        """
        import requests

        messages = [{"role": "system", "content": "You are Aetheris, a sovereign AI assistant."}]
        if history:
            messages.extend(history)
        messages.append({"role": "user", "content": query})

        payload = {
            "model": self.cfg.default_model,
            "messages": messages,
            "temperature": self.cfg.temperature,
            "max_tokens": self.cfg.max_tokens,
            "stream": False
        }

        resp = requests.post(
            f"{self.cfg.ai_endpoint}/v1/chat/completions",
            json=payload,
            headers=self.cfg.headers,
            timeout=self.cfg.request_timeout
        )

        if resp.status_code != 200:
            return LLMResponse(
                text=f"Error: API returned {resp.status_code}",
                usage={"error": resp.status_code},
                model=self.cfg.default_model
            )

        data = resp.json()
        return LLMResponse(
            text=data["choices"][0]["message"]["content"],
            usage=data.get("usage", {}),
            model=data.get("model", self.cfg.default_model)
        )
