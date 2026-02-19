# Positional Stats

**Category:** Richer Analysis
**Summary:** Break down VPIP, PFR, 3-bet, and other key stats by position (BTN, CO, BB, etc.).

## Current State

- `PlayerStats` already has `positions: Option<HashMap<String, PositionStats>>`
- `PositionStats` currently only tracks `{ hands, vpip, pfr }` — three fields
- `calculate_stats` in `src/stats/calculate.rs` populates this with per-position VPIP/PFR
- Each `Hand` has `hero_position: Option<Position>` and each `Player` has `position: Option<Position>`
- Qdrant payload stores `hero_position` as a filterable string field

## What's Missing

1. `PositionStats` is too sparse — only VPIP/PFR, missing 3-bet, steal, c-bet, etc.
2. No way to request stats for a single position via MCP
3. No positional comparison view (e.g., "how do I play BTN vs CO?")

## Build Plan

### Step 1: Expand PositionStats

**File:** `src/stats/calculate.rs` and the struct definition (likely in `mod.rs` or `calculate.rs`)

Expand `PositionStats` to include:
```rust
pub struct PositionStats {
    pub hands: f64,
    pub vpip: f64,
    pub pfr: f64,
    pub three_bet_pct: f64,
    pub fold_to_three_bet: f64,
    pub cbet_flop: f64,
    pub fold_to_cbet_flop: f64,
    pub steal_pct: f64,         // only meaningful for CO/BTN/SB
    pub wwsf: f64,
    pub went_to_showdown_pct: f64,
    pub won_at_showdown_pct: f64,
    pub aggression_factor: f64,
    pub winrate_bb100: f64,
}
```

In the main `calculate_stats` loop, accumulate the same opportunity/count pairs already computed for overall stats, but keyed by position. After the loop, compute percentages per position the same way.

### Step 2: Refactor Accumulation

**File:** `src/stats/calculate.rs`

The current code accumulates ~28 stat counters as individual variables. To avoid doubling them for positional tracking:

- Create a `StatAccumulator` struct holding all opportunity/count pairs
- Use one `StatAccumulator` for overall stats and a `HashMap<Position, StatAccumulator>` for positional
- Each hand updates both the overall and the position-specific accumulator
- After the loop, convert each accumulator to its output struct

This is a refactor of the existing calculation loop, not new logic.

### Step 3: MCP Filter by Position

**File:** `src/mcp.rs`

The `get_stats` tool already accepts a `position` filter param that maps to `hero_position` in Qdrant. This means users can already request stats for hands played from a specific position. However, the response returns overall `PlayerStats` computed from those filtered hands.

To add an explicit positional breakdown:
- Add optional param `by_position: Option<bool>` to `GetStatsParams`
- When `true`, include the `positions` HashMap in the response (currently it may be omitted or `None`)
- Ensure `positions` is always populated in `calculate_stats` (make it non-optional)

### Step 4: Position Comparison

**File:** `src/stats/calculate.rs` or new `src/stats/position.rs`

Add helper:
```rust
pub fn compare_positions(stats: &PlayerStats, pos_a: &str, pos_b: &str) -> Option<PositionComparison>
```

Where `PositionComparison` shows each stat side-by-side with a delta. This is a pure data transform on already-computed stats — no new Qdrant queries needed.

Expose via MCP as a formatting option or new tool `compare_positions`.

## Dependencies

- Existing `stats/` module — refactor, not rewrite
- No new crates needed

## Testing

- Unit tests comparing positional stats against known hand distributions from `PolarFox/` data
- Verify that overall stats remain unchanged after refactor (regression test)
- Test edge cases: positions with zero hands, stud hands (no position), heads-up (only BTN/BB)
