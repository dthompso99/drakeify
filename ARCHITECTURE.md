# Drakeify Architecture

## Overview

Drakeify is split into two separate binaries to provide clean separation of concerns:

1. **`drakeify`** - HTTP proxy server (headless mode)
2. **`drakeify-cli`** - Interactive CLI, plugin/tool management, and shell compatibility

## Binary Responsibilities

### drakeify (Proxy Server)

**Purpose:** Production-ready HTTP proxy that adds tool and plugin capabilities to any LLM.

**Features:**
- OpenAI-compatible API endpoint (`/v1/chat/completions`)
- Transparent tool execution
- Plugin lifecycle hooks
- Session management
- Stateless operation (suitable for containerized deployments)

**Usage:**
```bash
./target/release/drakeify
```

**Docker:**
The default CMD in the Dockerfile runs this binary.

---

### drakeify-cli (CLI Tool)

**Purpose:** Development, testing, and plugin/tool management.

**Features:**
- Interactive chat mode with tool execution
- Plugin/tool publishing to OCI registry
- Plugin/tool installation from OCI registry
- Package listing and discovery
- Shell command execution (k9s compatibility)

**Usage:**
```bash
# Interactive chat
./target/release/drakeify-cli
./target/release/drakeify-cli chat

# Plugin management
./target/release/drakeify-cli publish --package-type plugin --path ./my-plugin --name my-plugin --version 1.0.0 --description "My plugin"
./target/release/drakeify-cli install --package-type plugin --name my-plugin --version 1.0.0
./target/release/drakeify-cli list --package-type plugin

# Shell command (k9s compatibility)
./target/release/drakeify-cli -c "ls -la"
```

**Docker:**
Available at `/drakeify-cli` and symlinked to `/bin/sh` for k9s shell access.

---

## Code Organization

### Library (`src/lib.rs`)

Shared code used by both binaries:

- Configuration (`DrakeifyConfig`)
- LLM interaction (`llm` module)
- Tool registry (`tools` module)
- Plugin registry (`plugins` module)
- JavaScript runtime (`js_runtime` module)
- Session management (`session` module)
- HTTP proxy (`proxy` module)
- OCI registry client (`registry` module)
- Conversation execution (`run_conversation` function)

### Binaries

**`src/bin/drakeify.rs`** (Proxy Server)
- Minimal entry point
- Loads configuration
- Starts HTTP proxy server
- No CLI argument parsing (except from config/env)

**`src/bin/drakeify-cli.rs`** (CLI Tool)
- Full CLI argument parsing with clap
- Interactive chat mode
- Plugin/tool management commands
- Shell command execution

---

## Configuration

Both binaries share the same configuration system:

1. **Config file:** `drakeify.toml`
2. **Environment variables:** `DRAKEIFY_*` (take precedence)

See `drakeify.toml.example` for all available options.

---

## Docker Deployment

The Docker image includes both binaries:

```dockerfile
COPY --from=builder /usr/src/drakeify/target/x86_64-unknown-linux-musl/release/drakeify /drakeify
COPY --from=builder /usr/src/drakeify/target/x86_64-unknown-linux-musl/release/drakeify-cli /drakeify-cli
COPY --from=builder /usr/src/drakeify/target/x86_64-unknown-linux-musl/release/drakeify-cli /bin/sh
```

- `/drakeify` - Default CMD, runs the proxy server
- `/drakeify-cli` - Available for management tasks
- `/bin/sh` - Symlinked to drakeify-cli for k9s shell compatibility

---

## k9s Shell Compatibility

When k9s opens a shell in a container, it looks for `/bin/sh`. By symlinking `drakeify-cli` to `/bin/sh`, we provide:

1. **Shell command execution:** `drakeify-cli -c "command"` behaves like `sh -c "command"`
2. **Interactive access:** Users can run drakeify-cli commands from k9s shell
3. **Debugging:** Full access to plugin/tool management from within the container

---

## Development Workflow

### Building

```bash
cargo build --release
```

Produces:
- `target/release/drakeify`
- `target/release/drakeify-cli`

### Testing Proxy

```bash
./target/release/drakeify
```

### Testing CLI

```bash
./target/release/drakeify-cli
```

### Docker Build

```bash
docker build -t drakeify .
```

### Docker Run

```bash
docker run -p 8080:8080 drakeify
```

---

## Migration Notes

**Previous Architecture:**
- Single binary with CLI subcommands
- `headless` flag determined mode
- Mixed concerns (proxy + CLI in one binary)

**New Architecture:**
- Two separate binaries
- Clean separation of concerns
- Smaller proxy binary for production
- Full-featured CLI for development

**Breaking Changes:**
- None for Docker deployments (default CMD unchanged)
- CLI users should use `drakeify-cli` instead of `drakeify run`

