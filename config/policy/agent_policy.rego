package aetheris.agent_policy

import future.keywords.if
import future.keywords.in

default allow = false

# Researcher actions
allow if {
    input.agent.role == "researcher"
    input.agent.action in ["query", "read", "extract_entities", "list_sources"]
}

# Coder actions
allow if {
    input.agent.role == "coder"
    input.agent.action in ["write", "read", "execute_readonly", "list_directory"]
}

# Reviewer actions
allow if {
    input.agent.role == "reviewer"
    input.agent.action in ["read", "evaluate", "query_kg", "list_sources"]
}

# Planner actions
allow if {
    input.agent.role == "planner"
    input.agent.action in ["read", "query", "query_kg", "list_agents", "coordinate"]
}

# Admin override
allow if {
    input.agent.role == "admin"
}
