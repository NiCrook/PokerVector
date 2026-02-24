# Plan: Break Down `src/mcp/mod.rs` Into Smaller Files

## Problem

`mod.rs` is 2810 lines with 37 tool methods all in one `#[tool_router] impl` block, plus the `ServerHandler` impl. It's hard to navigate, and unrelated tools (e.g. quiz_hand vs get_database_health) share the same file.

## Target Structure

```
src/mcp/
├── mod.rs              # struct, new(), ServerHandler, re-exports (~80 lines)
├── params.rs           # (unchanged) all param structs
├── helpers.rs          # (unchanged) shared utilities
├── analysis.rs         # (unchanged) pure analysis functions
├── tools_search.rs     # search & retrieval tools
├── tools_stats.rs      # stats, villain lists, comparisons
├── tools_sessions.rs   # session detection & review
├── tools_hands.rs      # single-hand tools (detail, replay, quiz, raw, reimport, context)
├── tools_villains.rs   # villain-focused tools (profile, tendencies, showdowns, matchups, similar)
├── tools_spots.rs      # spot-finding tools (coolers, equity, squeeze, auto-tag, multiway)
├── tools_export.rs     # export, count, query, bankroll graph
├── tools_meta.rs       # import, last_import, database_health
└── tools_advanced.rs   # leak detection, tilt, trends, street stats, sizing, board, range, preflop chart
```

## Tool Assignments

### `tools_search.rs` (2 tools)
- `search_hands` — semantic/action search
- `search_similar_hands` — find similar by hand ID

### `tools_stats.rs` (4 tools)
- `get_stats` — aggregate player stats
- `list_villains` — opponent list with stats
- `get_best_villains` — most profitable opponents
- `get_worst_villains` — least profitable opponents
- `compare_stats` — side-by-side player comparison

### `tools_sessions.rs` (2 tools)
- `list_sessions` — session list
- `review_session` — detailed session review

### `tools_hands.rs` (5 tools)
- `get_hand` — full hand details
- `get_hand_as_replayer` — step-by-step replay
- `quiz_hand` — decision quiz
- `get_hand_history` — raw text
- `get_hand_context` — surrounding table hands

### `tools_villains.rs` (5 tools)
- `get_villain_profile` — comprehensive report
- `get_villain_tendencies` — action-reaction patterns
- `get_showdown_hands` — villain holdings at showdown
- `get_positional_matchups` — hero vs villain by position
- `get_similar_villains` — find villains matching a stat profile

### `tools_spots.rs` (4 tools)
- `auto_tag_hands` — classify hands by archetype
- `get_coolers` — showdown losses
- `get_equity_spots` — all-in hands
- `get_squeeze_spots` — squeeze-eligible preflop spots
- `get_multiway_stats` — multiway pot stats

### `tools_export.rs` (4 tools)
- `export_hands` — CSV or raw export
- `count_hands` — filtered count
- `query_hands` — raw SQL WHERE
- `get_bankroll_graph` — cumulative profit over time

### `tools_meta.rs` (3 tools)
- `watch_directory` — import hand histories
- `get_last_import` — import status
- `get_database_health` — database diagnostics
- `reimport_hand` — re-parse and re-embed a hand

### `tools_advanced.rs` (8 tools)
- `find_leaks` — automated leak detection
- `detect_tilt` — session-level tilt detection
- `get_trends` — stats over time
- `get_street_stats` — per-street action frequencies
- `get_sizing_profile` — bet sizing distributions
- `get_board_stats` — performance by board texture
- `get_range_analysis` — starting hand distribution
- `get_preflop_chart` — preflop hand chart
- `get_table_profitability` — profit by stakes/table

## Constraint: rmcp `#[tool_router]`

The `#[tool_router]` macro must annotate a single `impl` block. We **cannot** split tools across multiple `impl` blocks with separate `#[tool_router]` annotations — rmcp doesn't support merging routers.

**Approach:** Each `tools_*.rs` file defines regular `async fn` methods on `PokerVectorMcp` (via a separate `impl PokerVectorMcp` block — Rust allows multiple impl blocks for the same type). The single `#[tool_router] impl` in `mod.rs` becomes a thin dispatch layer: each `#[tool]` method is a one-liner that calls the real implementation in the appropriate tools file.

```rust
// mod.rs — thin dispatch
#[tool_router]
impl PokerVectorMcp {
    #[tool(description = "Search poker hand histories...")]
    async fn search_hands(&self, Parameters(p): Parameters<SearchHandsParams>) -> Result<CallToolResult, ErrorData> {
        self.tool_search_hands(p).await
    }
    // ... one-liner per tool
}

// tools_search.rs — real implementation
impl PokerVectorMcp {
    pub(crate) async fn tool_search_hands(&self, params: SearchHandsParams) -> Result<CallToolResult, ErrorData> {
        // full implementation here
    }
}
```

## Phases

### Phase 1: Scaffold the dispatch pattern (1 tool)

Move `search_hands` implementation to `tools_search.rs` as `tool_search_hands`. Replace the body in `mod.rs` with `self.tool_search_hands(p).await`. Verify it compiles and the MCP test still works. This proves the pattern.

**Files touched:** `mod.rs`, `tools_search.rs` (new)

### Phase 2: Extract search & retrieval tools

Move `search_similar_hands` to `tools_search.rs`.

**Files touched:** `mod.rs`, `tools_search.rs`

### Phase 3: Extract stats tools

Create `tools_stats.rs`. Move: `get_stats`, `list_villains`, `get_best_villains`, `get_worst_villains`, `compare_stats`.

**Files touched:** `mod.rs`, `tools_stats.rs` (new)

### Phase 4: Extract session tools

Create `tools_sessions.rs`. Move: `list_sessions`, `review_session`.

**Files touched:** `mod.rs`, `tools_sessions.rs` (new)

### Phase 5: Extract single-hand tools

Create `tools_hands.rs`. Move: `get_hand`, `get_hand_as_replayer`, `quiz_hand`, `get_hand_history`, `get_hand_context`.

**Files touched:** `mod.rs`, `tools_hands.rs` (new)

### Phase 6: Extract villain tools

Create `tools_villains.rs`. Move: `get_villain_profile`, `get_villain_tendencies`, `get_showdown_hands`, `get_positional_matchups`, `get_similar_villains`.

**Files touched:** `mod.rs`, `tools_villains.rs` (new)

### Phase 7: Extract spot-finding tools

Create `tools_spots.rs`. Move: `auto_tag_hands`, `get_coolers`, `get_equity_spots`, `get_squeeze_spots`, `get_multiway_stats`.

**Files touched:** `mod.rs`, `tools_spots.rs` (new)

### Phase 8: Extract export & query tools

Create `tools_export.rs`. Move: `export_hands`, `count_hands`, `query_hands`, `get_bankroll_graph`.

**Files touched:** `mod.rs`, `tools_export.rs` (new)

### Phase 9: Extract meta/admin tools

Create `tools_meta.rs`. Move: `watch_directory`, `get_last_import`, `get_database_health`, `reimport_hand`.

**Files touched:** `mod.rs`, `tools_meta.rs` (new)

### Phase 10: Extract advanced analysis tools

Create `tools_advanced.rs`. Move: `find_leaks`, `detect_tilt`, `get_trends`, `get_street_stats`, `get_sizing_profile`, `get_board_stats`, `get_range_analysis`, `get_preflop_chart`, `get_table_profitability`.

**Files touched:** `mod.rs`, `tools_advanced.rs` (new)

### Phase 11: Clean up mod.rs

- Remove now-dead imports that only the moved tools used
- Verify all `mod` declarations are present
- Run full `cargo test` and manual MCP test
- `mod.rs` should be ~250 lines: struct + new + thin dispatch + ServerHandler

## Validation at Each Phase

1. `cargo build` — compiles
2. `cargo test` — all pass
3. After Phase 1: manual MCP test with `search_hands` to confirm tool dispatch works through the indirection

## Notes

- Each `tools_*.rs` file needs its own imports (types, stats, search, etc.) — copy only what's used
- The `helpers::mcp_error` function is shared across all files — already in `helpers.rs`
- The `impl PokerVectorMcp` blocks in tools files use `pub(crate)` visibility for the methods
- No public API changes — the MCP schema is identical before and after
