# PokerVector Setup & Operations

## Prerequisites

- Rust toolchain
- Docker (for Qdrant)

## Qdrant (Vector Database)

**Start Qdrant with persistent storage:**

```bash
docker run -d -p 6333:6333 -p 6334:6334 -v qdrant_data:/qdrant/storage qdrant/qdrant
```

The `-v qdrant_data:/qdrant/storage` flag is **required** — without it, all imported hand data is lost when the container stops or restarts. This has bitten us before.

- REST API: `http://localhost:6333`
- gRPC (used by the app): `http://localhost:6334`

**Check if Qdrant is running and has data:**

```bash
curl http://localhost:6333/healthz
curl http://localhost:6333/collections/poker_hands
```

If the collection doesn't exist or has 0 points, you need to re-import.

## Import Hand Histories

Only needed once (data persists in Qdrant):

```bash
cargo run -- import ./PolarFox/       # import test data (392 hands)
cargo run -- import                    # import all configured accounts
```

You do NOT need to re-import after `cargo build`. The Qdrant database is independent of the Rust binary.

## Config

Lives at `~/.pokervector/config.toml`. Created by `scan` or `add-account`:

```bash
cargo run -- scan                      # auto-detect poker clients
cargo run -- add-account ./path/       # manually add an account
cargo run -- status                    # show config + Qdrant info
```

## MCP Server

```bash
cargo run -- mcp                       # hero from config
cargo run -- mcp --hero PolarFox       # explicit hero
```

Logs go to stderr, JSON-RPC protocol on stdout.

### Testing MCP from CLI

MCP uses stdio transport. To test manually, write a script (stdin must stay open):

```bash
cat > /tmp/mcp_test.sh << 'SCRIPT'
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}'
echo '{"jsonrpc":"2.0","method":"notifications/initialized"}'
sleep 1
echo '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"list_sessions","arguments":{"limit":5}}}'
sleep 10
SCRIPT
bash /tmp/mcp_test.sh | cargo run -- mcp --hero PolarFox 2>/dev/null | grep '"id":2'
```

Do NOT use `printf` or inline `{ }` subshells — stdin closes before the server reads the tool call.

## MCP Tools

| Tool | Description |
|------|-------------|
| `search_hands` | Semantic search with filters (position, stakes, villain, game_type, etc.) |
| `get_hand` | Full hand details by ID |
| `get_stats` | Aggregate stats (VPIP, PFR, 3-bet%, c-bet, etc.) |
| `list_villains` | Opponent summaries with key stats |
| `list_sessions` | Detected cash game sessions (30-min inactivity gap) |
| `review_session` | Detailed session review: stats, per-table breakdown, notable hands |

## Troubleshooting

| Problem | Cause | Fix |
|---------|-------|-----|
| "Failed to scroll hands" | Collection doesn't exist in Qdrant | `cargo run -- import ./PolarFox/` |
| Collection missing after restart | Qdrant ran without `-v` volume flag | Restart with `-v qdrant_data:/qdrant/storage`, then re-import |
| Binary locked during `cargo build` | MCP server process is still running | Kill the running `pokervector.exe` process first |
| MCP tool call not received | stdin pipe closed too early | Use script file with `sleep` delays, not inline pipes |
