package aetheris.authz

import future.keywords.if
import future.keywords.in

default allow = false

allow if {
    is_authenticated
    has_permission
    not is_blacklisted
}

is_authenticated if {
    [valid, header, payload] := io.jwt.decode_verify(input.token, {
        "cert": input.trusted_public_key,
        "aud": "aetheris-mesh"
    })
    valid
}

has_permission if {
    input.user_role == "admin"
}

has_permission if {
    input.user_role == "analyst"
    input.method == "GET"
    input.action in ["semantic_search", "read_vault"]
}

has_permission if {
    input.user_role == "system_agent"
    input.action in ["index_file", "snapshot"]
}

is_blacklisted if {
    input.peer_id in data.blacklisted_peers
}

allow if {
    is_authenticated
    input.file_metadata.sensitivity == "high"
    input.user_clearance == "top_secret"
}
