"""
MCP Server — Model Context Protocol implementation for Aetheris.

Provides:
- Tool Registry: Register and expose tools for LLM function calling
- Resource Server: Expose files, knowledge, and state as MCP resources
- Prompt Templates: Reusable prompt templates for agent workflows

Based on the MCP specification: https://modelcontextprotocol.io/
"""

import json
import time
import logging
from typing import Any, Callable, Dict, List, Optional
from dataclasses import dataclass, field

logger = logging.getLogger(__name__)


@dataclass
class MCPTool:
    """A tool exposed via MCP."""
    name: str
    description: str
    parameters: Dict[str, Any]  # JSON Schema
    handler: Callable
    tags: List[str] = field(default_factory=list)


@dataclass
class MCPResource:
    """A resource exposed via MCP."""
    uri: str
    name: str
    description: str
    mime_type: str = "text/plain"
    content: Optional[str] = None
    content_fn: Optional[Callable] = None


@dataclass
class MCPPrompt:
    """A prompt template exposed via MCP."""
    name: str
    description: str
    arguments: List[Dict[str, str]]  # [{name, description, required}]
    template: str


class ToolRegistry:
    """Registry for MCP tools."""

    def __init__(self):
        self._tools: Dict[str, MCPTool] = {}
    
    def register(
        self,
        name: str,
        description: str,
        parameters: Dict[str, Any],
        tags: Optional[List[str]] = None,
    ):
        """Decorator to register a function as an MCP tool."""
        def decorator(func: Callable):
            self._tools[name] = MCPTool(
                name=name,
                description=description,
                parameters=parameters,
                handler=func,
                tags=tags or [],
            )
            return func
        return decorator
    
    def register_tool(self, tool: MCPTool):
        """Register a tool directly."""
        self._tools[tool.name] = tool
    
    def get_tool(self, name: str) -> Optional[MCPTool]:
        return self._tools.get(name)
    
    def list_tools(self) -> List[Dict]:
        return [
            {
                "name": t.name,
                "description": t.description,
                "inputSchema": t.parameters,
                "tags": t.tags,
            }
            for t in self._tools.values()
        ]
    
    async def call_tool(self, name: str, arguments: Dict[str, Any]) -> Dict:
        """Execute a tool by name."""
        tool = self._tools.get(name)
        if not tool:
            return {"error": f"Tool not found: {name}"}
        try:
            result = await tool.handler(**arguments) if arguments else await tool.handler()
            return {"success": True, "result": result}
        except Exception as e:
            logger.error(f"Tool {name} failed: {e}")
            return {"error": str(e)}


class ResourceServer:
    """Manages MCP resources."""

    def __init__(self):
        self._resources: Dict[str, MCPResource] = {}
    
    def add_resource(self, resource: MCPResource):
        self._resources[resource.uri] = resource
    
    def get_resource(self, uri: str) -> Optional[MCPResource]:
        return self._resources.get(uri)
    
    def list_resources(self) -> List[Dict]:
        return [
            {
                "uri": r.uri,
                "name": r.name,
                "description": r.description,
                "mimeType": r.mime_type,
            }
            for r in self._resources.values()
        ]
    
    async def read_resource(self, uri: str) -> Optional[str]:
        resource = self._resources.get(uri)
        if not resource:
            return None
        if resource.content_fn:
            resource.content = await resource.content_fn()
        return resource.content


class PromptLibrary:
    """Manages MCP prompt templates."""

    def __init__(self):
        self._prompts: Dict[str, MCPPrompt] = {}
    
    def add_prompt(self, prompt: MCPPrompt):
        self._prompts[prompt.name] = prompt
    
    def get_prompt(self, name: str) -> Optional[MCPPrompt]:
        return self._prompts.get(name)
    
    def list_prompts(self) -> List[Dict]:
        return [
            {
                "name": p.name,
                "description": p.description,
                "arguments": p.arguments,
            }
            for p in self._prompts.values()
        ]
    
    def render_prompt(self, name: str, arguments: Dict[str, str]) -> Optional[str]:
        prompt = self._prompts.get(name)
        if not prompt:
            return None
        template = prompt.template
        for key, value in arguments.items():
            template = template.replace(f"{{{key}}}", value)
        return template


class MCPServer:
    """Complete MCP server combining tools, resources, and prompts."""

    def __init__(self, name: str = "aetheris", version: str = "2.0.0"):
        self.name = name
        self.version = version
        self.tools = ToolRegistry()
        self.resources = ResourceServer()
        self.prompts = PromptLibrary()
        self._capabilities = {
            "tools": {"list": True, "call": True},
            "resources": {"list": True, "read": True},
            "prompts": {"list": True, "render": True},
        }
    
    def get_server_info(self) -> Dict:
        return {
            "name": self.name,
            "version": self.version,
            "capabilities": self._capabilities,
        }
    
    async def handle_request(self, method: str, params: Optional[Dict] = None) -> Dict:
        """Route MCP protocol requests."""
        params = params or {}
        
        if method == "initialize":
            return self.get_server_info()
        
        elif method == "tools/list":
            return {"tools": self.tools.list_tools()}
        
        elif method == "tools/call":
            return await self.tools.call_tool(params.get("name"), params.get("arguments", {}))
        
        elif method == "resources/list":
            return {"resources": self.resources.list_resources()}
        
        elif method == "resources/read":
            content = await self.resources.read_resource(params.get("uri"))
            if content is None:
                return {"error": "Resource not found"}
            return {"contents": [{"uri": params["uri"], "text": content}]}
        
        elif method == "prompts/list":
            return {"prompts": self.prompts.list_prompts()}
        
        elif method == "prompts/render":
            rendered = self.prompts.render_prompt(params.get("name"), params.get("arguments", {}))
            if rendered is None:
                return {"error": "Prompt not found"}
            return {"messages": [{"role": "user", "content": rendered}]}
        
        else:
            return {"error": f"Unknown method: {method}"}
