# PokerVector

A poker hand history engine exposed as an MCP server. Import hand histories from online poker clients, and query them from any MCP-compatible AI client (Claude Desktop, Claude Code, Cursor, VS Code Copilot, etc.).

PokerVector parses, embeds, and indexes your hands locally. You bring your own LLM — PokerVector serves the data.

## Supported Formats

- **AmericasCardroom (ACR / WPN)** — Hold'em, Omaha, Omaha H/L, 5-Card Omaha, 7-Card Stud H/L
- Cash games and tournaments
- No Limit, Pot Limit, Fixed Limit

## Prerequisites

- **Rust** (stable toolchain)
- **protoc** (Protocol Buffers compiler) — required by the LanceDB build
  - Windows: `choco install protoc`
  - macOS: `brew install protobuf`
  - Linux: `apt install protobuf-compiler`

No Docker or external services needed. PokerVector uses LanceDB, an embedded vector database that stores everything locally.

## Installation

```bash
git clone https://github.com/NiCrook/PokerVector.git
cd PokerVector
cargo build --release
```

The embedding model (BGE-small-en-v1.5) downloads automatically on first run.

## Quick Start

```bash
# 1. Auto-detect installed poker clients and save accounts
cargo run -- scan

# 2. Import hand histories
cargo run -- import                   # all configured accounts
cargo run -- import ./PolarFox/       # specific directory

# 3. Check status
cargo run -- status

# 4. Start the MCP server
cargo run -- mcp                      # hero from config
cargo run -- mcp --hero PolarFox      # explicit hero
```

You can also manually register an account:

```bash
cargo run -- add-account ./path/to/hand/histories/
```

## MCP Client Setup

Add PokerVector to your MCP client config. For example, in Claude Desktop's `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "pokervector": {
      "command": "cargo",
      "args": ["run", "--manifest-path", "/path/to/PokerVector/Cargo.toml", "--", "mcp"]
    }
  }
}
```

Or if you've installed the binary to your PATH:

```json
{
  "mcpServers": {
    "pokervector": {
      "command": "pokervector",
      "args": ["mcp"]
    }
  }
}
```

Once configured, your AI client discovers PokerVector's tools automatically.

## MCP Tools

| Tool | Description |
|------|-------------|
| `search_hands` | Semantic or action-sequence search with filters (position, stakes, villain, pot type, result) |
| `get_hand` | Fetch full details of a hand by ID |
| `get_stats` | Aggregate stats (VPIP, PFR, 3-bet%, c-bet, steal, 25+ metrics) with filters |
| `list_villains` | List opponents with hand counts and key stats |
| `list_sessions` | List detected cash game sessions |
| `review_session` | Session review with aggregate stats and notable hands |
| `search_similar_hands` | Find structurally similar hands by action sequence |

## Data Storage

All data is stored locally at `~/.pokervector/data/` (LanceDB embedded database). Config lives at `~/.pokervector/config.toml`.

## Windows Build Notes

The `tokenizers` crate must use `fancy-regex` (not `onig`) and disable `esaxx_fast` to avoid CRT conflicts. This is already configured in `Cargo.toml`.

## License

AGPL-3.0 — see [LICENSE](LICENSE) for details.
