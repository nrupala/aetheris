"""
Default MCP Tools for the Agent Orchestrator.

Tools available to all agents:
- rag_query: Query the RAG knowledge base
- rag_ingest: Index a file into the knowledge base
- kg_lookup: Look up entities/relations in the knowledge graph
- kg_context: Get personal context from KG for a query
- file_read: Read a file from the workspace
- file_write: Write a file to the workspace
- code_execute: Execute code in a sandbox (future)
- web_search: Search the web (future, behind OPA gate)
"""

import os
import json
import logging
from typing import Any, Callable, Dict, List, Optional

from .server import MCPTool, ToolRegistry

logger = logging.getLogger(__name__)


def register_default_tools(registry: ToolRegistry, rag_pipeline=None, kg=None):
    """Register all default tools with the registry."""
    
    # RAG Query Tool
    @registry.register(
        name="rag_query",
        description="Query the RAG knowledge base with semantic search",
        parameters={
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "The question to ask"},
                "top_k": {"type": "integer", "description": "Number of results", "default": 5},
                "threshold": {"type": "number", "description": "Min similarity score", "default": 0.7},
            },
            "required": ["query"],
        },
        tags=["rag", "search", "knowledge"],
    )
    async def rag_query(query: str, top_k: int = 5, threshold: float = 0.7) -> Dict:
        if not rag_pipeline:
            return {"error": "RAG pipeline not available"}
        result = rag_pipeline.query(query, top_k=top_k, threshold=threshold)
        return {
            "answer": result.answer,
            "sources": result.sources,
            "confidence": result.confidence,
            "model": result.model,
        }
    
    # RAG Ingest Tool
    @registry.register(
        name="rag_ingest",
        description="Index a file into the RAG knowledge base",
        parameters={
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Path to file to index"},
                "extract_entities": {"type": "boolean", "description": "Extract entities into KG", "default": True},
            },
            "required": ["file_path"],
        },
        tags=["rag", "ingest", "index"],
    )
    async def rag_ingest(file_path: str, extract_entities: bool = True) -> Dict:
        if not rag_pipeline:
            return {"error": "RAG pipeline not available"}
        if not os.path.exists(file_path):
            return {"error": f"File not found: {file_path}"}
        result = rag_pipeline.ingest_file(
            file_path,
            metadata={"source": file_path},
            extract_entities=extract_entities,
        )
        return result
    
    # KG Lookup Tool
    @registry.register(
        name="kg_lookup",
        description="Look up an entity in the knowledge graph",
        parameters={
            "type": "object",
            "properties": {
                "entity_name": {"type": "string", "description": "Name of entity to look up"},
                "include_relations": {"type": "boolean", "description": "Include relations", "default": True},
            },
            "required": ["entity_name"],
        },
        tags=["kg", "lookup", "graph"],
    )
    async def kg_lookup(entity_name: str, include_relations: bool = True) -> Dict:
        if not kg:
            return {"error": "Knowledge Graph not available"}
        entity = kg.get_entity(entity_name)
        if not entity:
            return {"error": f"Entity not found: {entity_name}"}
        result = {"entity": entity}
        if include_relations:
            result["relations"] = kg.get_relations_for_entity(entity_name)
        return result
    
    # KG Context Tool
    @registry.register(
        name="kg_context",
        description="Get personal context from KG for a query",
        parameters={
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "Query to get context for"},
            },
            "required": ["query"],
        },
        tags=["kg", "context", "personal"],
    )
    async def kg_context(query: str) -> Dict:
        if not kg:
            return {"error": "Knowledge Graph not available"}
        context = kg.get_personal_context(query)
        return {"context": context}
    
    # File Read Tool
    @registry.register(
        name="file_read",
        description="Read a file from the workspace",
        parameters={
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Path to file"},
                "max_bytes": {"type": "integer", "description": "Max bytes to read", "default": 10000},
            },
            "required": ["file_path"],
        },
        tags=["file", "read", "workspace"],
    )
    async def file_read(file_path: str, max_bytes: int = 10000) -> Dict:
        if not os.path.exists(file_path):
            return {"error": f"File not found: {file_path}"}
        try:
            with open(file_path, "r", encoding="utf-8", errors="replace") as f:
                content = f.read(max_bytes)
            return {"content": content, "truncated": os.path.getsize(file_path) > max_bytes}
        except Exception as e:
            return {"error": str(e)}
    
    # File Write Tool
    @registry.register(
        name="file_write",
        description="Write content to a file in the workspace",
        parameters={
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Path to file"},
                "content": {"type": "string", "description": "Content to write"},
            },
            "required": ["file_path", "content"],
        },
        tags=["file", "write", "workspace"],
    )
    async def file_write(file_path: str, content: str) -> Dict:
        try:
            os.makedirs(os.path.dirname(file_path) or ".", exist_ok=True)
            with open(file_path, "w", encoding="utf-8") as f:
                f.write(content)
            return {"success": True, "bytes_written": len(content)}
        except Exception as e:
            return {"error": str(e)}
    
    # List Directory Tool
    @registry.register(
        name="list_directory",
        description="List files in a workspace directory",
        parameters={
            "type": "object",
            "properties": {
                "dir_path": {"type": "string", "description": "Directory to list"},
            },
            "required": ["dir_path"],
        },
        tags=["file", "directory", "workspace"],
    )
    async def list_directory(dir_path: str) -> Dict:
        if not os.path.exists(dir_path):
            return {"error": f"Directory not found: {dir_path}"}
        try:
            entries = os.listdir(dir_path)
            files = []
            for e in entries:
                full = os.path.join(dir_path, e)
                files.append({
                    "name": e,
                    "type": "directory" if os.path.isdir(full) else "file",
                    "size": os.path.getsize(full) if os.path.isfile(full) else None,
                })
            return {"entries": files}
        except Exception as e:
            return {"error": str(e)}
