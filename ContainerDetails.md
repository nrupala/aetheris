To build Aetheris as a containerized, zero-trust system using only FOSS (Free and Open Source Software) protocols, we define a "Personal Cloud Mesh". This configuration uses [WireGuard](https://www.wireguard.com/) for the encrypted transport layer, [Ollama](https://ollama.com/) for local AI inference, and ChromaDB as the persistent semantic vector store. [1, 2, 3, 4, 5] 
This setup is "stronger than Cloudflare" because it provides End-to-End Encryption (E2EE) where you own the keys, and unlike Cloudflare Tunnels, it does not terminate TLS at a corporate edge. [6] 
## Aetheris FOSS Container Compose (compose.yaml) [7] 

version: "3.9"
services:
  # 1. THE TRANSPORT LAYER: Private WireGuard Mesh
  # Replaces Cloudflare Tunnel with an invisible, encrypted L3 pipe.
  network-mesh:
    image: linuxserver/wireguard:latest
    container_name: aetheris_mesh
    cap_add:
      - NET_ADMIN
      - SYS_MODULE
    environment:
      - PUID=1000
      - PGID=1000
      - TZ=Etc/UTC
      - SERVERURL=auto # Set to your public IP or DDNS for external peer discovery
      - PEERS=mobile_client,laptop_client
    volumes:
      - ./config/wireguard:/config
      - /lib/modules:/lib/modules
    ports:
      - "51820:51820/udp" # Only port open to the world
    sysctls:
      - net.ipv4.conf.all.src_valid_mark=1
    restart: unless-stopped

  # 2. THE INTELLIGENCE LAYER: Local AI Inference
  # Runs LLMs locally for semantic document indexing.
  ai-engine:
    image: ollama/ollama:latest
    container_name: aetheris_ai
    volumes:
      - ./data/ollama:/root/.ollama
    # deploy: # Uncomment for GPU acceleration
    #   resources:
    #     reservations:
    #       devices:
    #         - driver: nvidia
    #           count: 1
    #           capabilities: [gpu]
    restart: unless-stopped

  # 3. THE SEMANTIC STORE: Vector Database
  # Persistence for document "embeddings" (how the AI understands your files).
  vector-db:
    image: chromadb/chroma:latest
    container_name: aetheris_vectors
    environment:
      - IS_PERSISTENT=TRUE
    volumes:
      - ./data/chroma:/chroma/data
    restart: unless-stopped

  # 4. THE CORE AGENT: Orchestrator (Go-based, No JS Frameworks)
  # Acts as the Zero-Trust Gateway and file-watcher.
  aetheris-core:
    build:
      context: ./core
      dockerfile: Dockerfile
    container_name: aetheris_core
    depends_on:
      - network-mesh
      - ai-engine
      - vector-db
    volumes:
      - ./vault:/data/vault:ro # Your actual files (Read-Only for security)
      - ./config/policy:/etc/aetheris/policy:ro # Open Policy Agent rules
    environment:
      - AI_ENDPOINT=http://ai-engine:11434
      - DB_ENDPOINT=http://vector-db:8000
    network_mode: "service:network-mesh" # Forces core through the encrypted mesh
    restart: unless-stopped
networks:
  default:
    name: aetheris_internal

## Key Architectural Pillars

* Invisibility: Only port 51820/udp is exposed. To the public internet, your server is a "black hole". Access to the AI and files is only possible once a device completes a WireGuard handshake.
* Zero-Framework Frontend: The aetheris-core should be a compiled binary (e.g., written in Go or Rust) that serves raw HTML templates. This avoids the security vulnerabilities and "bloat" of modern JS frameworks like Vue or Vite.
* Semantic Persistence: By mounting ./vault to the vector engine, the system automatically indexes new files as they arrive. Even if you delete a file, its "semantic fingerprint" remains in the local ChromaDB until purged.
* Zero-Trust Enforcement: The core agent can utilize Open Policy Agent (OPA) within the container to verify the identity of every single incoming packet before granting access to a file. [2, 8, 9, 10, 11, 12, 13] 

## How to Deploy

   1. Prepare Directories: mkdir -p config/wireguard data/ollama data/chroma vault core.
   2. Add Core Binary: Place your Go/C core source in /core.
   3. Launch: Run docker compose up -d. [10, 14] 


[1] [https://medium.com](https://medium.com/@soumitsr/a-broke-b-chs-guide-to-tech-start-up-choosing-vector-database-part-1-local-self-hosted-4ebe4eec3045)
[2] [https://www.youtube.com](https://www.youtube.com/watch?v=sLRjx0Xa6R4)
[3] [https://www.mintlify.com](https://www.mintlify.com/psviderski/uncloud/advanced/wireguard-mesh)
[4] [https://hub.docker.com](https://hub.docker.com/r/linuxserver/wireguard)
[5] [https://www.1006.org](https://www.1006.org/blog/2024-10-26_running_your_own_foss_llm/)
[6] [https://www.reddit.com](https://www.reddit.com/r/selfhosted/comments/1s1bj3k/cloudflare_tunnel_alternatives/)
[7] [https://webdock.io](https://webdock.io/en/docs/how-guides/docker-guides/how-to-install-and-run-docker-containers-using-docker-compose)
[8] [https://www.reddit.com](https://www.reddit.com/r/selfhosted/comments/1l741kc/octelium_v0110_a_modern_open_source_selfhosted/)
[9] [https://www.youtube.com](https://www.youtube.com/watch?v=3aRENOYwlcM)
[10] [https://www.youtube.com](https://www.youtube.com/watch?v=Jx_xsLcEEI0)
[11] [https://www.ionos.ca](https://www.ionos.ca/digitalguide/server/configuration/docker-compose-tutorial/)
[12] [https://www.youtube.com](https://www.youtube.com/watch?v=OOt_otZXWXA#:~:text=The%20presentation%20focuses%20on%20constructing%20a%20privacy%2Dfocused,setup%20involved%20in%20creating%20such%20a%20pipeline.)
[13] [https://medium.com](https://medium.com/data-science-collective/how-to-set-up-and-use-a-vector-db-in-less-than-10-minutes-4ace45e1f9da)
[14] [https://openclassrooms.com](https://openclassrooms.com/en/courses/7905646-optimize-your-deployment-with-docker-containers/8012885-create-a-docker-compose-file-to-orchestrate-your-containers)
