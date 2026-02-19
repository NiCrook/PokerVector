# Villain Profiling

**Category:** Richer Analysis
**Summary:** Per-villain stat summaries, tendencies, and leak detection.

## Current State

- `list_villains()` in `src/stats/villains.rs` computes a `VillainSummary` per opponent with 11 stats (VPIP, PFR, AF, 3-bet, FT3B, c-bet flop, FT c-bet flop, steal, WWSF)
- `VillainSummary` is returned by the `list_villains` MCP tool
- Qdrant payload stores `opponent_names` as a list field, filterable with `villain` param
- `calculate_stats()` can be called with any player name as "hero" to compute full stats for a villain

## What's Missing

1. **Full villain stats** — `VillainSummary` only has 11 of the 28 available stats
2. **Villain tendencies** — no natural language profiling (e.g., "tight-aggressive, folds to 3-bets, rarely c-bets turn")
3. **Leak detection** — no automated identification of exploitable patterns
4. **Head-to-head stats** — how does hero perform specifically against this villain
5. **Villain history** — no way to pull hands involving a specific villain

## Build Plan

### Step 1: Full Villain Stats

**File:** `src/stats/villains.rs`

- Extend `VillainSummary` to include all 28 stats from `PlayerStats`, or replace it with a call to `calculate_stats(hands, villain_name)`
- The existing `calculate_stats` already works for any player, not just the hero — it takes `hero: &str` which is just the player to analyze
- Refactor `list_villains` to use `calculate_stats` internally, then select key fields for the summary view
- Add `get_villain_profile(hands: &[Hand], villain: &str, hero: &str) -> VillainProfile` for the detailed single-villain view

### Step 2: Tendency Classification

**File:** `src/stats/villains.rs` (new section)

Add `classify_villain(stats: &PlayerStats) -> VillainProfile`:
```rust
pub struct VillainProfile {
    pub stats: PlayerStats,
    pub style: String,              // "TAG", "LAG", "Nit", "Fish/Calling Station", "Maniac"
    pub tendencies: Vec<String>,    // human-readable tendency descriptions
    pub leaks: Vec<Leak>,
}

pub struct Leak {
    pub category: String,       // "preflop", "postflop", "positional"
    pub description: String,    // "Folds to 3-bets 78% of the time (>65% is exploitable)"
    pub severity: String,       // "minor", "moderate", "major"
    pub exploit: String,        // "3-bet wider for value against this player"
}
```

Style classification thresholds:
- **Nit:** VPIP < 15, PFR < 12
- **TAG:** VPIP 15–25, PFR 12–22, AF > 2.0
- **LAG:** VPIP 25–35, PFR 20–30, AF > 2.5
- **Fish/Calling Station:** VPIP > 35, PFR < 15, AF < 1.5
- **Maniac:** VPIP > 40, PFR > 30, AF > 3.0

### Step 3: Leak Detection

**File:** `src/stats/villains.rs`

Define leak rules as threshold checks on stats:
- FT3B > 65% → "Folds too much to 3-bets"
- FT3B < 30% → "Calls 3-bets too wide"
- C-bet flop > 80% → "C-bets too frequently, can be check-raised"
- C-bet flop < 40% → "Misses c-bet spots, can probe"
- FT c-bet > 60% → "Folds to c-bets too often"
- VPIP - PFR gap > 15 → "Calls too much preflop, rarely raises"
- Steal > 45% → "Steals too wide from late position"
- WWSF < 40% with high VPIP → "Enters pots but gives up postflop"
- Limp% > 20% → "Limps too often preflop"

### Step 4: Head-to-Head Stats

**File:** `src/stats/villains.rs`

Add `head_to_head(hands: &[Hand], hero: &str, villain: &str) -> HeadToHead`:
```rust
pub struct HeadToHead {
    pub shared_hands: u32,          // hands both players were in
    pub hero_stats: PlayerStats,    // hero's stats in shared hands only
    pub villain_stats: PlayerStats, // villain's stats in shared hands only
    pub hero_profit_vs_villain: f64,
}
```

Filter `hands` to only those where both hero and villain are seated (and not sitting out), then compute stats for each.

### Step 5: Expose via MCP

**File:** `src/mcp.rs`

Add new tool `get_villain_profile`:
```rust
pub struct GetVillainProfileParams {
    pub villain: String,
    pub hero: Option<String>,
}
```

Returns `VillainProfile` with full stats, style, tendencies, leaks, and head-to-head data.

## Dependencies

- Existing `stats/` module — extends `villains.rs`
- Existing `calculate_stats` function (already works for any player)
- No new crates needed

## Testing

- Unit tests for style classification with mock `PlayerStats` at each archetype boundary
- Unit tests for leak detection with stats that trigger each leak rule
- Unit tests for head-to-head filtering — verify only shared hands are included
- Integration test using `PolarFox/` data to profile real opponents
