# Aetheris — What NOT To Do

Common anti-patterns, mistakes, and pitfalls to avoid when developing or operating Aetheris.

## Architecture & Code

### Don't hold `std::sync::MutexGuard` across `.await`
A `MutexGuard` is not `Send`. Holding it across an `.await` point makes the future non-Send, which breaks Axum handler registration.

**Instead:** Extract the value from the lock, drop the guard, then `.await`, then re-acquire the lock.

```rust
// ❌ Bad: MutexGuard held across await
let data = state.lock().unwrap();
let result = some_async_fn(&data).await; // Not Send!

// ✅ Good: Extract, drop, await, re-lock
let data = state.lock().unwrap().take();
drop(data);
let result = some_async_fn(&data).await;
state.lock().unwrap().replace(data);
```

### Don't define trait methods that return unboxed futures
Traits with async methods require boxing or unstable features. Use `Pin<Box<dyn Future + Send>>` return types or the `async_trait` crate.

```rust
// ❌ Bad: async fn in trait (requires nightly)
trait ModelBridge {
    async fn query(&self, q: &str) -> Result<String>;
}

// ✅ Good: boxed future
trait ModelBridge: Send + Sync {
    fn query(&self, q: &str) -> Pin<Box<dyn Future<Output = Result<String>> + Send>>;
}
```

### Don't forget `Send + Sync` supertraits on shared trait objects
Trait objects stored in `Arc<dyn Trait>` must be `Send + Sync` to work with Axum's shared state.

```rust
// ❌ Bad: can't put in Arc<dyn Bridge>
trait Bridge { }

// ✅ Good: can put in Arc<dyn Bridge>
trait Bridge: Send + Sync { }
```

## Security

### Don't hardcode credentials
Passwords, API keys, and tokens must come from environment variables, not source code.

```bash
# ❌ Bad: password in code
let password = "BCjfTYIIjMASFGVM";

# ✅ Good: password from environment
let password = std::env::var("AETHERIS_PASSWORD").unwrap();
```

### Don't use `unwrap()` in library code
Panics in library code crash the entire process. Use `?` to propagate errors or handle them gracefully.

```rust
// ❌ Bad: panics on error
let data = file.read_to_string().unwrap();

// ✅ Good: propagates error
let data = file.read_to_string().await?;
```

### Don't open unnecessary ports
Every open port is an attack surface. Aetheris should only expose port 8080 (Rust core) behind the Nginx proxy.

- Port 11434 (Ollama) — NEVER expose publicly; only accessible to core via internal Docker network
- Port 9090 (Python orchestrator) — NEVER expose publicly; only accessible to core via internal network
- Port 22 (SSH) — NEVER expose; use Cloudflare Tunnel for admin access

## Operations

### Don't run `docker compose down -v` in production
The `-v` flag removes all volumes, including RAG data, config, and databases. This is irreversible.

```bash
# ❌ Dangerous in production
docker compose down -v

# ✅ Safe restart
docker compose restart
# ✅ Full reset with data preserved
docker compose down && docker compose up -d
```

### Don't deploy without verifying service health
Always run verification checks after deployment:

```bash
curl -u user:pass https://dev.nrupalakolkar.com/api/health
./scripts/verification.sh
```

### Don't ignore WAL corruption
WAL (Write-Ahead Log) corruption indicates potential data integrity issues. If WAL replay fails, investigate immediately before resuming operations.

## Development

### Don't commit to `main` directly
Always use feature branches and PRs. Direct commits to `main` bypass CI checks.

```bash
git checkout -b feature/my-feature
# ... make changes ...
git push -u origin feature/my-feature
# Create PR, get review, then merge
```

### Don't skip `cargo clippy` and `cargo fmt`
These checks run in CI. Skipping them locally means CI will fail. Run them before every commit.

```bash
cargo fmt --all
cargo clippy -- -D warnings
```

### Don't assume LMStudio is available
The production server runs Ollama on port 11434, not LMStudio on port 1234. Always use `AI_ENDPOINT=http://host.docker.internal:11434`.

### Don't add new dependencies without review
Every dependency increases build time, binary size, and attack surface. Ask before adding.

## Configuration

### Don't use incorrect auth credentials
The single production password `BCjfTYIIjMASFGVM` applies to all users: `ai_user`, `rag_user`, `dev_user`. The previous separate password `H5epZhriylz+99+1` for `rag_user` is deprecated.

### Don't reference models that don't exist
The production server only has one model: `qwen2.5:14b`. Don't reference `strand-rust-coder-14b-v1`, `microsoft/phi-4-reasoning-plus`, or other models that may not be loaded.

## Testing

### Don't skip UAT tests
User Acceptance Tests validate the full stack. Run them before any production deployment.

### Don't confuse emulation with production
The emulation environment (`EMULATION.md`) is for testing infrastructure patterns. It is not a production deployment.
