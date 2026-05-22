"""
RAG Client — HTTP client to the RAG engine for cross-container communication.

Since the orchestrator and RAG run in separate containers, we cannot
import rag_pipeline directly. This client calls the RAG HTTP API.
"""

import json
import logging
from typing import Any, Dict, List, Optional

import aiohttp

logger = logging.getLogger(__name__)


class RAGResult:
    """Compatible result object mimicking rag_core.pipeline.RAGResult."""
    def __init__(self, data: Dict):
        self.answer = data.get("answer", "")
        self.sources = data.get("sources", [])
        self.query = data.get("query", "")
        self.model = data.get("model", "")
        self.response_time = data.get("response_time", 0)
        self.tokens_used = data.get("tokens_used", 0)
        self.chunks_searched = data.get("chunks_searched", 0)
        self.confidence = data.get("confidence", 0.0)
        self.iterations_used = data.get("iterations_used", 1)
        self.reasoning_trace = data.get("reasoning_trace", [])
        self.converged = data.get("converged", True)
        self.verification = data.get("verification", {})


class RAGClient:
    """HTTP client to the RAG engine."""

    def __init__(self, endpoint: str = "http://rag-service:8080"):
        self.endpoint = endpoint.rstrip("/")
        self._session: Optional[aiohttp.ClientSession] = None
    
    async def _get_session(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession()
        return self._session
    
    async def query(
        self,
        query: str,
        top_k: int = 5,
        threshold: float = 0.7,
        reasoning: bool = False,
        max_iterations: int = 3,
        confidence_threshold: float = 0.7,
        use_rag: bool = True,
    ) -> Optional[RAGResult]:
        """Query the RAG engine via HTTP."""
        try:
            session = await self._get_session()
            async with session.post(
                f"{self.endpoint}/query",
                json={
                    "query": query,
                    "use_rag": use_rag,
                    "top_k": top_k,
                    "threshold": threshold,
                    "reasoning": reasoning,
                    "max_iterations": max_iterations,
                    "confidence_threshold": confidence_threshold,
                    "include_history": False,
                },
                timeout=aiohttp.ClientTimeout(total=60),
            ) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    return RAGResult(data)
                else:
                    body = await resp.text()
                    logger.error(f"RAG query failed: {resp.status} {body}")
                    return None
        except Exception as e:
            logger.error(f"RAG query exception: {e}")
            return None
    
    async def ingest_file(self, file_path: str, extract_entities: bool = True) -> Optional[Dict]:
        """Ingest a file via RAG engine HTTP API."""
        try:
            session = await self._get_session()
            import os
            if not os.path.exists(file_path):
                return {"error": f"File not found: {file_path}"}
            
            data = aiohttp.FormData()
            data.add_field("file", open(file_path, "rb"), filename=os.path.basename(file_path))
            
            async with session.post(
                f"{self.endpoint}/ingest/file",
                data=data,
                params={"extract_entities": extract_entities, "wait": "true"},
                timeout=aiohttp.ClientTimeout(total=120),
            ) as resp:
                if resp.status == 200:
                    return await resp.json()
                return {"error": f"HTTP {resp.status}"}
        except Exception as e:
            logger.error(f"RAG ingest exception: {e}")
            return {"error": str(e)}
    
    async def health(self) -> bool:
        """Check RAG engine health."""
        try:
            session = await self._get_session()
            async with session.get(
                f"{self.endpoint}/health",
                timeout=aiohttp.ClientTimeout(total=5),
            ) as resp:
                return resp.status == 200
        except Exception:
            return False
    
    async def stats(self) -> Optional[Dict]:
        """Get RAG engine stats."""
        try:
            session = await self._get_session()
            async with session.get(
                f"{self.endpoint}/stats",
                timeout=aiohttp.ClientTimeout(total=5),
            ) as resp:
                if resp.status == 200:
                    return await resp.json()
                return None
        except Exception:
            return None
    
    async def close(self):
        if self._session and not self._session.closed:
            await self._session.close()


class KGClient:
    """HTTP client to the Knowledge Graph via RAG engine."""

    def __init__(self, endpoint: str = "http://rag-service:8080"):
        self.endpoint = endpoint.rstrip("/")
        self._session: Optional[aiohttp.ClientSession] = None
    
    async def _get_session(self) -> aiohttp.ClientSession:
        if self._session is None or self._session.closed:
            self._session = aiohttp.ClientSession()
        return self._session
    
    async def get_personal_context(self, query: str) -> str:
        """Get personal context from KG."""
        try:
            session = await self._get_session()
            async with session.get(
                f"{self.endpoint}/knowledge-graph/profile",
                timeout=aiohttp.ClientTimeout(total=5),
            ) as resp:
                if resp.status == 200:
                    data = await resp.json()
                    return json.dumps(data, indent=2)[:1000]
                return ""
        except Exception:
            return ""
    
    async def get_entity(self, entity_name: str) -> Optional[Dict]:
        """Get an entity from KG."""
        try:
            session = await self._get_session()
            async with session.get(
                f"{self.endpoint}/knowledge-graph/entities",
                params={"limit": 1},
                timeout=aiohttp.ClientTimeout(total=5),
            ) as resp:
                if resp.status == 200:
                    entities = await resp.json()
                    for e in entities:
                        if e.get("name") == entity_name:
                            return e
                return None
        except Exception:
            return None
    
    async def get_stats(self) -> Optional[Dict]:
        """Get KG statistics."""
        try:
            session = await self._get_session()
            async with session.get(
                f"{self.endpoint}/knowledge-graph/stats",
                timeout=aiohttp.ClientTimeout(total=5),
            ) as resp:
                if resp.status == 200:
                    return await resp.json()
                return None
        except Exception:
            return None
    
    async def add_entity(self, name: str, entity_type: str, properties: Dict) -> bool:
        """Add entity to KG via orchestrator's shared KG hub."""
        try:
            session = await self._get_session()
            async with session.post(
                f"{self.endpoint}/knowledge-graph/export",
                json={"name": name, "entity_type": entity_type, "properties": properties},
                timeout=aiohttp.ClientTimeout(total=10),
            ) as resp:
                return resp.status == 200
        except Exception:
            return False
    
    async def close(self):
        if self._session and not self._session.closed:
            await self._session.close()
