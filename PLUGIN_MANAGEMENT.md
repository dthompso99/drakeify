# Plugin and Tool Management

Drakeify now supports publishing and installing plugins and tools from an OCI registry!

## Overview

Plugins and tools can be packaged and distributed as OCI artifacts, making it easy to share and discover community-created extensions.

## Configuration

Add the following to your `drakeify.toml`:

```toml
# Plugin/Tool Registry Configuration
registry_url = "https://zot.hallrd.click"
# registry_username = ""  # Optional: For private registries
# registry_password = ""  # Optional: For private registries
# registry_insecure = false  # Optional: Allow insecure connections (default: false)
```

## Publishing a Plugin or Tool

### Package Structure

Your plugin or tool package should have the following structure:

**For Plugins:**
```
my-plugin/
  ├── plugin.js       # The plugin code (required)
  └── README.md       # Documentation (optional)
```

**For Tools:**
```
my-tool/
  ├── tool.js         # The tool code (required)
  └── README.md       # Documentation (optional)
```

### Publishing Command

Use the `drakeify-cli` binary for plugin/tool management:

```bash
# Publish a plugin
drakeify-cli publish \
  --package-type plugin \
  --path ./examples/example-plugin \
  --name example-plugin \
  --version 1.0.0 \
  --description "An example plugin that logs requests" \
  --author "Your Name" \
  --license "MIT"

# Publish a tool
drakeify-cli publish \
  --package-type tool \
  --path ./my-tool \
  --name my-tool \
  --version 1.0.0 \
  --description "A useful tool" \
  --author "Your Name" \
  --license "MIT"
```

## Installing a Plugin or Tool

```bash
# Install a plugin
drakeify-cli install \
  --package-type plugin \
  --name example-plugin \
  --version 1.0.0

# Install a tool
drakeify-cli install \
  --package-type tool \
  --name my-tool \
  --version 1.0.0
```

Plugins are installed to the `plugins/` directory and tools to the `tools/` directory.

## Listing Available Packages

```bash
# List available plugins
drakeify-cli list --package-type plugin

# List available tools
drakeify-cli list --package-type tool
```

**Note:** The list command is not yet fully implemented as OCI registries don't have a standard catalog API.

## Package Metadata

When publishing, a `metadata.json` file is automatically created with the following structure:

```json
{
  "type": "plugin",
  "name": "example-plugin",
  "version": "1.0.0",
  "description": "An example plugin that logs requests",
  "author": "Your Name",
  "license": "MIT",
  "homepage": null,
  "dependencies": {},
  "drakeify_version": ">=0.1.0",
  "tags": [],
  "created": "2024-01-01T00:00:00Z"
}
```

## Example: Publishing the Example Plugin

```bash
# Build drakeify first
cargo build --release

# Publish the example plugin
./target/release/drakeify-cli publish \
  --package-type plugin \
  --path ./examples/example-plugin \
  --name example-plugin \
  --version 1.0.0 \
  --description "An example plugin that logs requests" \
  --author "Drakeify Team" \
  --license "MIT"
```

## Example: Installing a Plugin

```bash
# Install the example plugin
./target/release/drakeify-cli install \
  --package-type plugin \
  --name example-plugin \
  --version 1.0.0

# The plugin will be installed to plugins/example-plugin/
# and will be automatically loaded on next run
```

## Running the Agent

After installing plugins/tools, run the agent:

```bash
# Interactive mode
./target/release/drakeify-cli

# Or explicitly
./target/release/drakeify-cli chat

# Proxy mode
./target/release/drakeify
```

## Technical Details

- Packages are stored as OCI artifacts using the OCI Distribution specification
- Plugins are stored under `plugins/<name>` in the registry
- Tools are stored under `tools/<name>` in the registry
- Each package is a gzipped tarball containing the code and metadata
- Versioning follows semantic versioning (semver)

## Future Enhancements

- Dependency resolution
- Version constraints and updates
- Package search and discovery
- Package signing and verification
- Private registry authentication

