# Town Sovereign MCP Architecture

The sovereign companion extends Town through the Model Context Protocol (MCP).
Town connects to a remote MCP server by URL and gains new tools. Here the server
runs on the Aetheris box and exposes a single tool, sovereign_search.

Ollama serves embedding and language models locally. The MCP server calls Ollama
at 127.0.0.1:11434 and never sends corpus data to a cloud model.
