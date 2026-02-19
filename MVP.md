# PokerVector — MVP Specification

## Overview

PokerVector is a poker hand history engine exposed as an MCP (Model Context Protocol) server. Users import hand history files from online poker clients, and the system parses, embeds, and indexes them into a vector database. Any MCP-compatible client (Claude Code, Claude Desktop, Cursor, VS Code Copilot, etc.) can then query the data — the user's own chatbot handles reasoning and analysis, PokerVector just serves the data.

**We build the data layer. The user brings their own LLM.**

**MVP Scope:** No-Limit Hold'em cash games and tournaments. AmericasCardroom (ACR) hand history format.

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  User's MCP Client (Claude, Cursor, etc.)                   │
│  - Sends natural language questions                         │
│  - LLM decides which PokerVector tools to call              │
│  - LLM reasons over returned data                           │
└──────────────────────┬──────────────────────────────────────┘
                       │ JSON-RPC over stdio
                       v
┌─────────────────────────────────────────────────────────────┐
│  PokerVector (MCP Server)                                   │
│                                                             │
│  Tools exposed:                                             │
│    search_hands  — semantic + filtered hand retrieval        │
│    get_hand      — fetch a specific hand by ID              │
│    get_stats     — aggregate stats with optional filters    │
│    list_villains — list tracked opponents                   │
│                                                             │
│  Internals:                                                 │
│    Parsers ──> Summarizer ──> Embedder ──> Qdrant           │
└─────────────────────────────────────────────────────────────┘
```

---

## CLI Interface

```bash
# Account discovery and setup
pokervector scan                                       # auto-detect installed poker clients
pokervector add-account ./path/ --hero "username"      # manually register an account

# Data management
pokervector import                                     # import all configured accounts
pokervector import ./PolarFox/                         # import a specific directory (hero inferred from dir name)
pokervector import ./hand_histories/ --hero "username"  # explicit hero name
pokervector status                                     # show config, accounts, and Qdrant hand count

# MCP server mode (launched by the client automatically)
pokervector mcp                                        # hero from first configured account
pokervector mcp --hero PolarFox                        # explicit hero override
```

**Account auto-detection:** `pokervector scan` checks known poker client install paths (e.g. `C:\AmericasCardroom\handHistory\`) for subdirectories containing hand history files. Each subdirectory is an account, with the hero name inferred from the directory name. Discovered accounts are saved to `~/.pokervector/config.toml`.

**Zero-arg workflow:** After running `scan` once, both `import` (no path) and `mcp` (no `--hero`) work automatically using saved config. Explicit path/hero args still work for one-off use.

The user configures their MCP client once:

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

After that, their chatbot discovers and calls PokerVector tools automatically.

---

## MCP Tools

### `search_hands`

Semantic search over hand history, with optional metadata filters.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `query` | string | yes | Natural language search query |
| `position` | string | no | Filter by hero position (BTN, CO, HJ, LJ, SB, BB) |
| `pot_type` | string | no | Filter: SRP, 3bet, 4bet, limp, walk |
| `villain` | string | no | Filter by opponent name |
| `stakes` | string | no | Filter by stakes (e.g. "$0.01/$0.02") |
| `result` | string | no | Filter: won, lost, folded |
| `limit` | int | no | Max results (default 10) |

**Returns:** JSON array of hand summaries with hand_id, score, summary, hero_position, hero_cards, stakes, hero_result, and pot_type.

### `get_hand`

Fetch full details of a specific hand.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `hand_id` | u64 | yes | The numeric hand ID |

**Returns:** Complete hand record — players, actions per street, board, result, as serialized JSON.

### `get_stats`

Aggregate statistics with optional filters.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `hero` | string | no | Player name to compute stats for (defaults to configured hero) |
| `position` | string | no | Filter by position |
| `villain` | string | no | Stats against a specific opponent |
| `pot_type` | string | no | Filter by pot type |
| `stakes` | string | no | Filter by stakes |

**Returns:** VPIP, PFR, 3-bet%, fold-to-3bet%, aggression factor, win rate (bb/100), hands played, c-bet frequencies, steal/fold-to-steal, limp stats, donk/float/check-raise/probe/squeeze/cold-call/WWSF/overbet percentages, and positional breakdown.

### `list_villains`

List tracked opponents with hand counts and key stats.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `min_hands` | int | no | Minimum hands played (default 10) |
| `hero` | string | no | Hero name override (defaults to configured hero) |

**Returns:** JSON array of opponent summaries with name, hand count, VPIP, PFR, aggression factor, 3-bet%, fold-to-3bet%, c-bet flop%, fold-to-cbet flop%, steal%, and WWSF.

---

## Core Components

### 1. Hand History Parsers

**MVP target:** AmericasCardroom (ACR / Winning Poker Network).

**Multi-site design:** All parsers implement a shared `SiteParser` trait. The system auto-detects the site from file content and routes to the correct parser.

```rust
// parsers/mod.rs
pub trait SiteParser {
    fn parse_hand(&self, hand_text: &str, hero: &str) -> Result<Hand>;
    fn detect(content: &str) -> bool where Self: Sized;
}
```

Auto-detection works by inspecting the first line of a file:
- ACR cash: `Hand #XXXXXXX - Holdem (No Limit) - $X.XX/$X.XX`
- ACR tournament: `Game Hand #XXXXXXX - Tournament #XXXXXXX - Holdem (No Limit) - Level XX`
- PokerStars (future): `PokerStars Hand #XXXXXXX:`

**Site-agnostic types** live in `types.rs` — every parser produces the same `Hand` struct with fields for id, site, game_type, timestamp, table info, players, hero info, actions per street, board, pot/rake, and result.

**ACR format notes** (observed from real hand histories):
- Cash and tournament hands share ~95% of the same structure
- Cash uses `$` prefix on amounts, tournaments use raw chip values
- Some cash games include antes (e.g. `$0.01/$0.02, Ante $0.01`)
- Rake and JP Fee reported separately: `Rake $0.04 | JP Fee $0.02`
- `Main pot` and `Side pot(N)` shown inline during streets
- `shows [- Jc]` means partial card reveal (dash = unknown card)
- Two-pass name resolution handles multi-word player names (e.g. "Lost It", "Stanley kersey")

**Parsing approach:**
- Line-by-line regex-based parser with state machine (Preflop → Flop → Turn → River → Showdown → Summary)
- Hands separated by blank lines (`split_hands` with byte-level detection for \n and \r\n)
- Winner detection combines "collected" action lines with summary seat lines
- Skip malformed hands gracefully with warnings
- Tested against 392 real ACR hands across 10 files

### 2. Hand Summarizer

Converts structured `Hand` into deterministic natural language for embedding:

```
NL Hold'em $1/$2 | Hero (CO) with Ah Kd | Stack: $200
Preflop: Hero raises $6, BTN 3-bets $20, Hero calls
Flop: Qs Jd 4c | BTN bets $28, Hero calls
Turn: 7h | BTN checks, Hero bets $45, BTN folds
Result: Hero wins $97 pot
```

Embedding the summary (not the raw HH) ensures semantic search matches on poker concepts — position, hand strength, pot type, action sequences.

### 3. Embedding Model

Local embedding via ONNX Runtime — no external API calls.

- **Model:** `all-MiniLM-L6-v2` (384-dim vectors, auto-downloaded via hf-hub on first run)
- **Crate:** `ort` 2.0 (Rust ONNX Runtime bindings) + `tokenizers` + `ndarray`
- **Runs on CPU** — no GPU required
- **Batch embedding** for import efficiency (batches of 32)

### 4. Vector Storage (Qdrant)

Runs locally as a Docker container (`docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant`).

**Collection schema:**

```
Collection: "poker_hands"
  vector: [f32; 384]
  payload:
    hand_id: int64
    site: string                 # "ACR"
    game_type: string            # "cash" or "tournament"
    stakes: string               # "$0.01/$0.02" or "L5 25/50"
    table_size: int              # 6, 8, 9
    hero: string
    hero_position: string        # BTN, CO, HJ, LJ, SB, BB
    hero_cards: string           # "Ah Kd"
    hero_result: string          # won, lost, folded, sat_out
    board: string
    num_players: int
    went_to_showdown: bool
    timestamp: string
    summary: string              # Natural language summary
    hand_json: string            # Full Hand struct serialized
    pot_type: string             # SRP, 3bet, 4bet, limp, walk
    opponent_names: string[]
    pot_amount: float            # optional
    tournament_id: int64         # optional
```

Payload fields enable hybrid search — vector similarity + metadata filters. Import deduplicates by checking `hand_exists` before upserting.

### 5. MCP Transport

JSON-RPC layer over stdio using the `rmcp` crate (v0.15).

- `#[tool_router]` / `#[tool_handler]` macros auto-generate tool discovery and dispatch
- Parameter schemas generated via `schemars` for AI-friendly tool descriptions
- `PokerVectorMcp` struct holds `Arc<VectorStore>`, `Arc<Mutex<Embedder>>`, and hero name
- Logging to stderr (stdout reserved for MCP protocol)
- Stateless per-request — all state lives in Qdrant

---

## Project Structure

```
pokervector/
├── Cargo.toml
├── CLAUDE.md                # Claude Code project instructions
├── MVP.md                   # This file
├── src/
│   ├── main.rs              # CLI entry (clap): import, status, mcp, scan, add-account
│   ├── lib.rs               # Re-exports for integration tests
│   ├── types.rs             # Hand, Player, Action, Card, Position — site-agnostic
│   ├── parsers/
│   │   ├── mod.rs           # SiteParser trait, auto-detect, utilities
│   │   └── acr.rs           # AmericasCardroom / WPN format
│   ├── config.rs            # Config file (~/.pokervector/config.toml): load, save, merge
│   ├── scanner.rs           # Auto-detect installed poker clients and accounts
│   ├── summarizer.rs        # Hand → natural language summary
│   ├── embedder.rs          # ONNX embedding model (all-MiniLM-L6-v2)
│   ├── storage.rs           # Qdrant client: upsert, search, scroll, filters
│   ├── search.rs            # Semantic search with metadata filters
│   ├── stats.rs             # Aggregate stats (VPIP, PFR, 3bet%, 25+ metrics)
│   └── mcp.rs               # MCP server: tool definitions + dispatch
├── tests/
│   ├── parser_tests.rs      # 20 integration tests
│   └── fixtures/            # Sample hand histories
└── PolarFox/                # Real ACR hand history files (10 files, 392 hands)
```

---

## Dependencies

| Crate | Purpose |
|---|---|
| `clap` | CLI argument parsing |
| `tokio` | Async runtime |
| `serde` / `serde_json` | Serialization |
| `chrono` | Timestamps |
| `regex` | Hand history parsing |
| `glob` | File pattern matching |
| `ort` | ONNX Runtime (local embeddings) |
| `ndarray` | Tensor operations for embedding |
| `tokenizers` | Tokenization for embedding model |
| `hf-hub` | HuggingFace model download |
| `qdrant-client` | Vector DB client |
| `rmcp` | MCP server protocol (stdio transport) |
| `schemars` | JSON Schema generation for MCP tool params |
| `anyhow` / `thiserror` | Error handling |
| `dirs` | Cross-platform home directory for config path |
| `toml` | Config file serialization/deserialization |
| `tracing` | Logging |

---

## Milestones

### M1 — Parsers ✅
- [x] Define site-agnostic types in `types.rs` (Hand, Player, Action, Card, Position, GameType, etc.)
- [x] Define `SiteParser` trait with auto-detect in `parsers/mod.rs`
- [x] Implement ACR parser (`parsers/acr.rs`) — cash + tournament hands
- [x] Hero name inference from directory name
- [x] Unit tests against real ACR hand history samples (20 tests, 392 hands)
- [x] CLI `import` prints parsed output

### M2 — Embedding + Storage ✅
- [x] Hand → summary text conversion (`summarizer.rs`)
- [x] Local embedding model integration (ort + MiniLM, batch of 32)
- [x] Qdrant integration (upsert, payload indexing, deduplication)
- [x] CLI `import` parses, embeds, and stores end-to-end
- [x] CLI `status` shows hand count from Qdrant

### M3 — Search + Stats ✅
- [x] Semantic search with metadata filters (`search.rs`)
- [x] Aggregate stat calculations — 25+ metrics: VPIP, PFR, 3bet%, fold-to-3bet%, aggression, winrate bb/100, c-bet, steal, limp, donk, float, check-raise, probe, squeeze, cold-call, WWSF, overbet, positional breakdown (`stats.rs`)
- [x] Villain tracking and lookup with configurable min-hands threshold

### M4 — MCP Server ✅
- [x] MCP transport layer (rmcp 0.15, stdio)
- [x] Expose search_hands, get_hand, get_stats, list_villains as MCP tools
- [x] `--hero` flag on mcp subcommand
- [ ] Integration test: simulate MCP client calls (manual testing via Claude Code)
- [ ] User-facing setup documentation (README)

### M5 — Account Auto-Detection & Config ✅
- [x] Config file at `~/.pokervector/config.toml` (accounts, Qdrant URL/collection)
- [x] ACR scanner: auto-detect accounts from `C:\AmericasCardroom\handHistory\`
- [x] `pokervector scan` — discover and save new accounts
- [x] `pokervector add-account` — manually register accounts
- [x] Zero-arg `import` — import all configured accounts
- [x] Zero-arg `mcp` — hero from first configured account
- [x] De-hardcoded Qdrant URL and collection name (from config)

---

## Resolved Questions

1. **Qdrant deployment:** Docker container. Start with `docker run -p 6333:6333 -p 6334:6334 qdrant/qdrant`.
2. **Hand deduplication:** Import checks `hand_exists` by ID before upserting — re-importing is safe and skips existing hands.
3. **Config file:** Persistent config at `~/.pokervector/config.toml`. Stores discovered/manual accounts and Qdrant connection settings. Created by `scan` or `add-account` commands.

---

## Out of Scope (Post-MVP)

- Additional site parsers (PokerStars, GGPoker, 888, etc.)
- Real-time HUD overlay
- Web UI
- Hand range visualization
- GTO solver integration
- Multi-user / cloud deployment
- Additional site scanners (PokerStars, GGPoker install paths)
