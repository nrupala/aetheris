# Aetheris agent-action authorization policy (versioned, OPA v1 syntax).
#
# Agent authorization is separate from HTTP authorization (aetheris.authz).
# Consulted by OpaBridge.authorize_agent via
# POST {OPA_ENDPOINT}/v1/data/aetheris/agents/allow, which Agent.check_policy uses.
#
# Action allowlists are enumerated from the per-role allowlist in the check_policy
# local fallback (core/src/agents/mod.rs). Only the actions each agent role is
# authorized for are listed.
#
# Input contract (see core/src/bridge.rs AuthzInput; method/path are empty for
# agent calls):
#   input.role    string  researcher | coder | reviewer | planner | analyst
#   input.action  string  the action string the role passes to check_policy
package aetheris.agents

default allow = false

allow if {
    input.role == "researcher"
    input.action in {"query", "read", "extract_entities", "list_sources"}
}

allow if {
    input.role == "coder"
    input.action in {"write", "read", "execute_readonly", "list_directory"}
}

allow if {
    input.role == "reviewer"
    input.action in {"read", "evaluate", "query_kg", "list_sources"}
}

allow if {
    input.role == "planner"
    input.action in {"read", "query", "query_kg", "list_agents", "coordinate"}
}

# analyst has no AgentRole in agents/mod.rs today; allowlist is provisional,
# mirroring a read-only set drawn from the framework's existing action strings.
allow if {
    input.role == "analyst"
    input.action in {"query", "read", "query_kg", "list_sources"}
}
