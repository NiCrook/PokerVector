# compare_players MCP Tool

**Category:** New MCP Tools
**Summary:** Side-by-side stat comparison between two or more players.

## Current State

- `calculate_stats(hands, hero)` computes full `PlayerStats` for any player
- `list_villains()` provides summary stats for all opponents
- Each player's stats are computed independently — no comparison view
- Qdrant `opponent_names` field allows filtering hands involving specific players

## What's Missing

1. **Side-by-side comparison** — no way to view two players' stats together
2. **Delta highlighting** — no way to see where players differ most
3. **Head-to-head record** — how do two players perform against each other
4. **Shared hand context** — how many hands do they share in the dataset

## Build Plan

### Step 1: Comparison Data Structure

**File:** `src/stats/compare.rs` (new module)

```rust
pub struct StatComparison {
    pub name: String,           // stat name (e.g., "vpip")
    pub values: Vec<f64>,       // one per player, in order
    pub delta: f64,             // max - min
    pub notable: bool,          // true if delta exceeds a threshold
}

pub struct PlayerComparison {
    pub players: Vec<String>,
    pub hand_counts: Vec<u32>,          // hands per player
    pub shared_hands: u32,              // hands where all players were present
    pub stats: Vec<StatComparison>,     // all stats side-by-side
    pub biggest_differences: Vec<StatComparison>,  // top 5 by delta
}
```

### Step 2: Comparison Logic

**File:** `src/stats/compare.rs`

```rust
pub fn compare_players(
    hands: &[Hand],
    players: &[&str],
) -> PlayerComparison
```

1. For each player, compute `calculate_stats(hands, player)`
2. Build `StatComparison` for each of the 28 stats
3. Mark as `notable` if delta > threshold (e.g., VPIP difference > 10%, AF difference > 1.0)
4. Sort by delta to find `biggest_differences`
5. Count shared hands (hands where all specified players appear)

### Step 3: Hero vs. Villain Shortcut

Most common use case: compare hero against a single villain.

Add convenience wrapper:
```rust
pub fn hero_vs_villain(
    hands: &[Hand],
    hero: &str,
    villain: &str,
) -> PlayerComparison
```

Additionally filters to only hands where both players are present, giving the most relevant comparison.

### Step 4: Expose via MCP

**File:** `src/mcp.rs`

New tool `compare_players`:
```rust
pub struct ComparePlayersParams {
    pub players: Vec<String>,       // 2–4 player names
    pub shared_only: Option<bool>,  // only use hands where all players are present
    pub hero: Option<String>,
}
```

Validation: require at least 2 players, max 4.

Returns `PlayerComparison` as JSON.

## Dependencies

- Existing `calculate_stats()` — called once per player
- Existing `VectorStore::scroll_hands()` — single scroll, then compute
- No new crates needed

## Testing

- Unit test comparing two synthetic players with known stat differences
- Verify `biggest_differences` is correctly sorted by delta
- Test `shared_only` mode filters correctly
- Test edge case: player with zero hands in dataset
- Integration test with `PolarFox/` data comparing hero against a frequent opponent
