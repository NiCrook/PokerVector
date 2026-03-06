# PokerVector

A poker hand history engine exposed as an MCP server. Import hand histories from online poker clients, and query them from any MCP-compatible AI client (Claude Desktop, Claude Code, Cursor, VS Code Copilot, etc.).

PokerVector parses, embeds, and indexes your hands locally. You bring your own LLM — PokerVector serves the data.

## Supported Formats

- **AmericasCardroom (ACR / WPN)** — Hold'em, Omaha, Omaha H/L, 5-Card Omaha, 7-Card Stud H/L
- Cash games and tournaments
- No Limit, Pot Limit, Fixed Limit

> **Note on tournaments:** ACR hand histories do not include buy-in, payout, or finish position data. Tournament tools focus on in-game analysis (stack trajectories, push/fold decisions, bubble play) rather than results tracking like ROI or ITM%.

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
cargo run --release -- scan

# 2. Import hand histories
cargo run --release -- import                   # all configured accounts
cargo run --release -- import ./path/to/hands/  # specific directory

# 3. Check status
cargo run --release -- status

# 4. Start the MCP server
cargo run --release -- mcp                      # hero from config
cargo run --release -- mcp --hero YourUsername   # explicit hero
```

You can also manually register an account:

```bash
cargo run --release -- add-account ./path/to/hand/histories/
```

## MCP Client Setup

Add PokerVector to your MCP client config. For example, in Claude Desktop's `claude_desktop_config.json`:

```json
{
  "mcpServers": {
    "pokervector": {
      "command": "cargo",
      "args": ["run", "--release", "--manifest-path", "/path/to/PokerVector/Cargo.toml", "--", "mcp"]
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

## MCP Tools (54)

### Search & Retrieval

| Tool | Description |
|------|-------------|
| `search_hands` | Semantic or action-sequence search with filters (position, stakes, villain, pot type, result, date range) |
| `get_hand` | Fetch full details of a hand by ID |
| `get_hand_history` | Return original raw hand history text |
| `get_hand_context` | Get surrounding hands from the same table for context |
| `search_similar_hands` | Find structurally similar hands by betting pattern |
| `count_hands` | Count matching hands without returning data |
| `query_hands` | Power-user raw SQL WHERE clause queries |

### Stats & Analysis

| Tool | Description |
|------|-------------|
| `get_stats` | Aggregate stats (VPIP, PFR, 3-bet%, c-bet, steal, 25+ metrics) with filters |
| `get_pool_stats` | Player pool averages with distributions (mean, median, P25, P75) |
| `get_multiway_stats` | Stats filtered to multiway pots (3+ to flop) |
| `get_street_stats` | Per-street bet/raise/call/check/fold frequencies |
| `get_range_analysis` | Starting hand distributions by position and action |
| `get_preflop_chart` | Preflop hand chart with open/3bet/call/fold frequencies |
| `get_board_stats` | Performance by board texture (monotone, paired, connected, etc.) |
| `get_bankroll_graph` | Running profit/loss data points over time |
| `get_trends` | Stats over time bucketed by day, week, or month |
| `find_leaks` | Automated leak detection against baseline ranges |
| `detect_tilt` | Flag sessions where play deviated from baseline |
| `compare_stats` | Side-by-side stat comparison for two players |

### Villain Tools

| Tool | Description |
|------|-------------|
| `list_villains` | List opponents with hand counts and key stats |
| `get_villain_profile` | Comprehensive villain report in one call |
| `get_villain_tendencies` | How a villain reacts to specific betting lines |
| `get_sizing_profile` | Bet sizing patterns by street and outcome |
| `get_showdown_hands` | Hands where a villain revealed holdings |
| `get_positional_matchups` | Hero vs villain breakdown by position |
| `get_best_villains` | Opponents hero profits most against |
| `get_worst_villains` | Opponents hero loses most to |
| `get_similar_villains` | Find villains matching a stat profile |
| `cluster_villains` | Classify opponents into archetypes (Nit, TAG, LAG, Whale, etc.) |

### Session & Game Selection

| Tool | Description |
|------|-------------|
| `list_sessions` | List detected cash game sessions |
| `review_session` | Session review with stats, per-table breakdown, notable hands |
| `get_table_profitability` | Profit by stakes or table |

### Spot Finding

| Tool | Description |
|------|-------------|
| `get_bluff_candidates` | Hands where hero folded but a bluff might have worked |
| `get_coolers` | Showdown hands where hero invested heavily and lost |
| `get_equity_spots` | Hands where hero was all-in |
| `get_squeeze_spots` | Squeeze-eligible spots and what hero did |
| `get_runout_analysis` | Hero win rate on different board textures |
| `get_runout_frequencies` | Turn/river card frequency distributions |

### Tournament

| Tool | Description |
|------|-------------|
| `get_tournament_summary` | Tournament overview: stacks, blind levels, biggest pots |
| `get_tournament_stack_stats` | Stack and M-ratio trajectory across a tournament |
| `get_push_fold_review` | Review decisions at low M-ratio |
| `get_bubble_play` | Bubble play analysis: tightening vs aggression |
| `get_effective_stacks` | Effective stack depths for significant pots |

### Study & Export

| Tool | Description |
|------|-------------|
| `quiz_hand` | Hand quiz — hides hero's action for study |
| `get_hand_as_replayer` | Step-by-step replay with running pot/stack sizes |
| `export_hands` | Export as CSV or raw hand history text |
| `auto_tag_hands` | Auto-classify hands (cooler, hero call, big bluff, etc.) |
| `tag_hand` | Add custom tags/labels to a hand |
| `remove_tag` | Remove tags from a hand |
| `get_tags` | Get tags applied to a hand |

### Admin

| Tool | Description |
|------|-------------|
| `watch_directory` | Import new hands from configured directories |
| `get_last_import` | Last import info and total hand count |
| `reimport_hand` | Re-parse and re-embed a single hand |
| `get_database_health` | Storage diagnostics and data quality checks |

## Data Storage

All data is stored locally at `~/.pokervector/data/` (LanceDB embedded database). Config lives at `~/.pokervector/config.toml`.

## Windows Build Notes

The `tokenizers` crate must use `fancy-regex` (not `onig`) and disable `esaxx_fast` to avoid CRT conflicts. This is already configured in `Cargo.toml`.

## License

AGPL-3.0 — see [LICENSE](LICENSE) for details.
