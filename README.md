# snipctl

**Universal CLI Snippet Manager** — capture, search, and reuse commands from any CLI.

Automatically captures commands from `az`, `aws`, and `gcloud` out of the box, and is fully configurable to track any CLI tool (`gh`, `kubectl`, `docker`, `terraform`, etc.).

## Features

- 🔄 **Auto-capture** — shell hooks intercept CLI commands and save them as reusable templates
- 🔍 **Fuzzy search** — interactive TUI to find snippets fast
- 📝 **Smart parameterization** — `--name myRG` becomes `--name {{name}}`
- ☁️ **Multi-CLI** — supports Azure CLI, AWS CLI, Google Cloud CLI, and any custom CLI
- ⚡ **Single binary** — no runtime dependencies, works on Linux, macOS, and Windows
- 📤 **Export/Import** — share snippets across machines as JSON

## Install

### From source

```bash
cargo install --path .
```

### From crates.io (coming soon)

```bash
cargo install snipctl
```

## Quick Start

### 1. Save your first snippet

```bash
# Save commands manually
snipctl save "az group create --name myRG --location eastus"
# ✓ Saved: az group create --name {{name}} --location {{location}}

snipctl save "aws ec2 describe-instances --instance-id i-12345"
# ✓ Saved: aws ec2 describe-instances --instance-id {{instance_id}}

snipctl save "gcloud compute instances list --zone us-central1-a"
# ✓ Saved: gcloud compute instances list --zone {{zone}}
```

### 2. Search and run snippets

```bash
# Interactive fuzzy search
snipctl

# Search with a query
snipctl search "group"

# Filter by CLI
snipctl search --cli aws
```

### 3. Set up auto-capture

```bash
# Print shell hooks for your shell
snipctl hook bash    # or: zsh, fish, powershell

# Add to your shell config (e.g., ~/.bashrc)
eval "$(snipctl hook bash)"
```

Now every successful `az`, `aws`, and `gcloud` command is automatically captured!

### 4. Add custom CLIs

```bash
# Track additional CLIs
snipctl config add gh
snipctl config add kubectl
snipctl config add docker
snipctl config add terraform

# Get updated hooks
snipctl hook bash
```

## Commands

| Command | Description |
|---------|-------------|
| `snipctl` | Interactive fuzzy search (default) |
| `snipctl search [query]` | Search snippets with optional query |
| `snipctl list [--cli az]` | List all snippets, optionally filter by CLI |
| `snipctl save "<cmd>"` | Manually save a command |
| `snipctl capture --cli <name> "<cmd>"` | Capture (used by shell hooks) |
| `snipctl run <id>` | Run a saved snippet |
| `snipctl delete <id>` | Delete a snippet |
| `snipctl edit <id>` | Edit a snippet's template or description |
| `snipctl export [--cli <name>]` | Export snippets as JSON |
| `snipctl import <file>` | Import snippets from JSON file |
| `snipctl hook [shell]` | Print shell hooks for auto-capture |
| `snipctl config add <prefix>` | Add a CLI to track |
| `snipctl config remove <prefix>` | Remove a CLI |
| `snipctl config list` | Show tracked CLIs |

## Configuration

Config file: `~/.config/snipctl/config.toml`

```toml
[general]
storage_path = "~/.local/share/snipctl/snippets.json"

# Default CLIs
[[cli]]
name = "az"
prefix = "az"

[[cli]]
name = "aws"
prefix = "aws"

[[cli]]
name = "gcloud"
prefix = "gcloud"

# User-added CLIs
[[cli]]
name = "gh"
prefix = "gh"
```

## Snippet Storage

Snippets are stored as JSON at `~/.local/share/snipctl/snippets.json`:

```json
{
  "version": "1.0",
  "snippets": [
    {
      "id": "abc12345",
      "cli": "az",
      "template": "az group create --name {{name}} --location {{location}}",
      "original": "az group create --name myRG --location eastus",
      "description": "",
      "tags": ["group", "create"],
      "usage_count": 5,
      "created_at": "2026-03-16T12:00:00Z",
      "last_used": "2026-03-16T14:30:00Z"
    }
  ]
}
```

## Importing from azsnip

If you were using azsnip, you can import your existing snippets:

```bash
snipctl import ~/.azsnip/snippets.json --default-cli az
```

## How It Works

1. **Shell hooks** wrap configured CLI commands (e.g., `az`, `aws`)
2. On **successful execution** (exit code 0), the command is sent to `snipctl capture`
3. **Parameterization** converts flag values into `{{placeholder}}` templates
4. **Deduplication** prevents duplicates — repeat commands just bump the usage count
5. **Fuzzy search** lets you find and replay saved commands instantly

## License

MIT
