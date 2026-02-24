# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What This Is

PokerVector is a poker hand history engine exposed as an MCP server. Users import hand history files from online poker clients (currently ACR/AmericasCardroom), and the system parses, embeds, and indexes them into LanceDB (embedded database, no external services needed). Any MCP-compatible client queries the data — PokerVector serves data, the user brings their own LLM.

All four MVP milestones are complete (parsers, embedding+storage, search+stats, MCP server). See `MVP.md` for the full spec.

## Build & Test Commands

```bash
cargo build                          # compile
cargo test                           # run all tests (unit + integration)
cargo test test_name                  # run a single test by name
cargo test --test parser_tests        # run only integration tests
cargo run -- scan                     # auto-detect poker clients, save accounts to config
cargo run -- add-account ./path/      # manually add an account
cargo run -- import                   # import all configured accounts
cargo run -- import ./PolarFox/       # import a specific directory
cargo run -- status                   # show config + database info
cargo run -- mcp                      # start MCP server, hero from config
cargo run -- mcp --hero PolarFox      # start MCP server with explicit hero
```

Data is stored in `~/.pokervector/data/` (LanceDB embedded database). No Docker or external services required.

## Architecture

**Full pipeline:** Raw HH text → `split_hands()` → per-hand text → `AcrParser::parse_hand()` → `Hand` struct → `summarize()` + `encode_action_sequence()` → `Embedder::embed_batch()` (both) → `VectorStore::upsert()` (named vectors: "summary" + "action") → LanceDB

**MCP server flow:** Client JSON-RPC call → `rmcp` dispatch → tool method on `PokerVectorMcp` → query LanceDB (search/scroll) → JSON response over stdout

**Key modules:**
- `src/types.rs` — Site-agnostic types. `Hand` is the central struct (Serialize/Deserialize). Includes `PokerVariant` (Holdem/Omaha/FiveCardOmaha/SevenCardStud), `BettingLimit` (NoLimit/PotLimit/FixedLimit), `StudPlayerCards`, and stud `Street` variants (ThirdStreet through SeventhStreet).
- `src/parsers/mod.rs` — `SiteParser` trait, `parse_auto()` auto-detection, utilities (`parse_card`, `parse_money`, `split_hands`, `calculate_position`).
- `src/parsers/acr.rs` — ACR parser. Two-pass name resolution (longest-first) for multi-word player names. Supports Hold'em, Omaha, 5-Card Omaha, 7-Card Stud (with stud streets, `brings in`, no-button tables). State machine: Preflop/3rd Street → ... → Showdown → Summary. Bomb pot detection via summary `BombPot` line.
- `src/summarizer.rs` — Deterministic `Hand` → natural language summary for embedding.
- `src/action_encoder.rs` — Structured action sequence encoding. `encode_action_sequence()` converts a `Hand` into a stakes-normalized, anonymized betting line (e.g. `PRE: HERO_OPEN(3bb) V1_3BET(9bb)`). Uses PotTracker for accurate sizing, ActionLabel classification (open/3bet/4bet/cbet/raise multipliers), BB normalization (preflop) and pot-fraction sizing (postflop).
- `src/embedder.rs` — ONNX Runtime (`ort`) + `tokenizers` + `hf-hub` for BGE-small-en-v1.5 (384-dim, 512 token limit). `Embedder::embed()` requires `&mut self`. Model auto-downloads on first run.
- `src/storage.rs` — LanceDB wrapper. Single table `poker_hands` with named vector columns `summary` + `action` (384-dim FixedSizeList<Float32>). Data at `~/.pokervector/data/`. `VectorStore::new()` connects and opens/creates the table in one step — fully initialized on return, no separate setup call needed. Handles upsert via merge-insert (`HandEmbeddings`), vector search by column name, SQL filter queries, scroll, dedup. Stores full `Hand` as JSON in `hand_json` column.
- `src/search.rs` — Search with SQL WHERE filters. `build_filter()` returns `Option<String>`. `SearchMode` enum (Semantic/Action) routes to the appropriate named vector column. `search_similar_actions()` finds structurally similar hands by ID.
- `src/sessions.rs` — Session detection from `Vec<Hand>`, cash games only. Groups hands into `TableSession`s (5-min gap) and merges into multi-table `Session`s (30-min gap). `SessionReview` with aggregate stats and `NotableHand`s (biggest wins/losses).
- `src/stats.rs` — 25+ aggregate stats (VPIP, PFR, 3-bet%, c-bet, steal, etc.) computed in-memory from `Vec<Hand>`. Also `list_villains`.
- `src/mcp.rs` — MCP server via `rmcp` 0.15. `PokerVectorMcp` struct with `#[tool_router]`/`#[tool_handler]` macros. Seven tools: `search_hands` (with `search_mode` param), `get_hand`, `get_stats`, `list_villains`, `list_sessions`, `review_session`, `search_similar_hands`. Uses `Parameters<T>` wrapper for tool arguments.
- `src/config.rs` — Persistent config at `~/.pokervector/config.toml`. Structs: `SiteKind`, `Account`, `Config`. Load/save/merge logic. `data_dir()` returns `~/.pokervector/data/`.
- `src/scanner.rs` — Auto-detection of installed poker clients. ACR scanner checks `C:\AmericasCardroom\handHistory\` for account subdirectories. `scan_all()` aggregates all site scanners.
- `src/main.rs` — CLI via clap: `import`, `status`, `mcp`, `scan`, `add-account` subcommands. `import` with no path imports all configured accounts. `mcp` with no `--hero` uses first configured account. MCP mode logs to stderr (stdout is protocol).

## Config System

Config lives at `~/.pokervector/config.toml`. Created by `scan` or `add-account` commands.

```toml
[[accounts]]
site = "acr"
hero = "PolarFox"
path = "C:\\AmericasCardroom\\handHistory\\PolarFox"
manual = false
```

Data stored in `~/.pokervector/data/` (LanceDB embedded database, no configuration needed).

Accounts are keyed on `(site, hero)` — merge logic prevents duplicates. `manual` flag distinguishes user-added accounts from scanner-discovered ones.

## Windows Build Notes

- `tokenizers` must use `fancy-regex` feature (not `onig`) and disable `esaxx_fast` to avoid CRT conflicts (`/MD` vs `/MT`).
- `ndarray` must be 0.17 to match `ort` 2.0.0-rc.11.
- `ort` `Session::run` requires `&mut self`, so embedder is behind `Arc<Mutex<Embedder>>` in MCP server.
- `protoc` (Protocol Buffers compiler) must be installed for `lance-encoding` build. Install via `choco install protoc`.

## ACR Format Quirks

- Cash headers: `Hand #ID - GAME (LIMIT) - $SB/$BB[, Ante $ANTE] - TIMESTAMP UTC`
  - GAME: `Holdem`, `Omaha H/L`, `5Card Omaha`, `7Stud H/L`
  - LIMIT: `No Limit`, `Pot Limit`, `Fixed Limit`
- Tournament headers: `Game Hand #ID - Tournament #TID - GAME (LIMIT) - Level L (SB/BB) - TIMESTAMP UTC`
- Tournament amounts lack `$` prefix
- `shows [- Jc]` means partial card reveal (dash = unknown)
- Side pots: `Side pot(N) AMOUNT` lines and `collected X from side pot-N`
- `Main pot X | Rake X` lines appear inline between streets, not just in summary
- Winner detection combines "collected" action lines with summary seat lines (some non-showdown wins only appear in summary)
- Stud: table line is `TableName M-max` (no button), streets are `*** 3rd STREET ***` through `*** 7th STREET ***`, `brings in` action, per-player dealt cards each street
- Bomb pots: `BombPot` line appears in `*** SUMMARY ***` section
- H/L: show descriptions contain `HI -` and `| LO -`, split pots have multiple `collected` lines

## Git Conventions

Commit messages follow the format: `type(scope): message`

The `message` should read as: "this commit will {message}".

**Types:** `feat`, `fix`, `refactor`, `test`, `docs`, `chore`, `perf`

**Examples:**
- `feat(mcp): add find_leaks tool`
- `fix(parser): handle missing ante in tournament headers`
- `refactor(stats): extract StatAccumulator for per-position tracking`
- `test(sessions): add tilt detection unit tests`
- `docs(plans): add build plans for roadmap features`

## Test Data

`PolarFox/` contains 18 real ACR hand history files covering cash games, tournaments, antes, side pots, split pots, multi-word player names, sitting out, all-in scenarios, Omaha H/L, 5-Card Omaha, 7-Card Stud H/L, and bomb pots. Test fixtures in `tests/fixtures/` are extracted from these files.
