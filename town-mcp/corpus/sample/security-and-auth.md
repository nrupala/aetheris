# Security and Authentication

The MCP server authenticates every request from Town with a bearer token in the
Authorization header. It is published only through the Cloudflare Tunnel and sits
behind Cloudflare Access using a service token, so requests are authorized at the
edge before they ever reach the process. The service binds to localhost and is
never exposed on a public port directly.
