To maximize security and performance, we will use a Multi-Stage Build. The first stage uses the full Rust toolchain to compile your code into a static binary. The second stage—the Distroless image—contains only that binary.
There is no shell, no package manager, and no unnecessary libraries for an attacker to exploit. This is the "Zero-Trust" gold standard for container security.
The Multi-Stage Distroless Dockerfile
dockerfile

# --- STAGE 1: The Builder ---
# We use the official Rust image to compile the binary
FROM rust:1.75-slim AS builder

WORKDIR /usr/src/aetheris
COPY . .

# Install build dependencies for static linking
RUN apt-get update && apt-get install -y musl-tools
RUN rustup target add x86_64-unknown-linux-musl

# Build the orchestrator as a static binary
# This ensures it has ZERO dependencies on the host OS
RUN cargo build --release --target x86_64-unknown-linux-musl

# --- STAGE 2: The Distroless Runner ---
# Google's "static" distroless image contains only:
# - ca-certificates (for secure TLS)
# - tzdata (for timezones)
# - user accounts
FROM gcr.io/distroless/static-debian12:latest

# Copy the compiled binary from the builder stage
COPY --from=builder /usr/src/aetheris/target/x86_64-unknown-linux-musl/release/aetheris-core /aetheris-core

# Use a non-root user (Standard security practice)
USER 1000:1000

# Execute the core
ENTRYPOINT ["/aetheris-core"]

Use code with caution.
Why this is "Stronger than Cloudflare"

    Zero Shell Access: If an attacker somehow bypasses your WireGuard mesh and hits this container, they cannot run ls, cd, or curl. There is no /bin/sh. The container only knows how to be the Aetheris Core. [1][4]
    Immutability: The filesystem is essentially read-only for the application logic.
    Minimal Attack Surface: A standard Ubuntu container is ~100MB and contains hundreds of vulnerabilities (CVEs). This Distroless image is ~5MB and typically contains zero known vulnerabilities because there is no OS code to attack. [4][5]
    Static Linking: Because we compiled for x86_64-unknown-linux-musl, the binary carries its own libraries. It doesn't even need a dynamic linker on the host. [3]

How to Integrate with your Compose
Update the aetheris-core section in your previously created compose.yaml:
yaml

  aetheris-core:
    build: .
    container_name: aetheris_core
    # ... other config ...
    security_opt:
      - no-new-privileges:true # Prevents the binary from ever gaining root