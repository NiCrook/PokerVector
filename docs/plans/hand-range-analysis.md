# hand_range_analysis MCP Tool

**Category:** New MCP Tools
**Summary:** Analyze what ranges a villain shows up with in specific spots.

## Current State

- `ActionType::Shows { cards, description }` captures revealed hands at showdown
- `Player.position` tracks position per hand
- `classify_pot_type()` categorizes hands as walk/limp/SRP/3bet/4bet
- Actions are fully recorded per street, allowing action line reconstruction
- No range analysis or hand frequency tracking exists

## What's Missing

1. **Range construction from showdown data** — collecting shown hands by spot
2. **Spot definition** — what constitutes a "spot" (position + action line + pot type)
3. **Range visualization** — frequency grid or text representation
4. **Sample size warnings** — ranges from small samples are unreliable
5. **Range vs. optimal comparison** — how does observed range compare to a baseline

## Build Plan

### Step 1: Spot Definition

**File:** `src/stats/ranges.rs` (new module)

```rust
pub struct Spot {
    pub position: Option<Position>,
    pub pot_type: String,           // "SRP", "3bet", "4bet"
    pub action_line: String,        // "open-raise", "call-3bet", "3-bet", etc.
    pub street_reached: Street,     // how far the hand went
}

pub fn classify_spot(hand: &Hand, player: &str) -> Spot
```

Action line classification for preflop:
- `"open-raise"` — first voluntary raise
- `"call-open"` — cold-called an open
- `"3-bet"` — re-raised an open
- `"call-3bet"` — called a 3-bet
- `"4-bet"` — re-raised a 3-bet
- `"limp"` — first voluntary call (no prior raise)
- `"squeeze"` — raised with a caller and raiser ahead

### Step 2: Range Collection

**File:** `src/stats/ranges.rs`

```rust
pub struct RangeEntry {
    pub hand_id: u64,
    pub cards: Vec<Card>,
    pub category: HandCategory,     // from showdown-analysis plan
    pub won: bool,
}

pub struct SpotRange {
    pub spot: Spot,
    pub total_times_in_spot: u32,       // regardless of showdown
    pub showdown_count: u32,
    pub entries: Vec<RangeEntry>,
    pub category_frequencies: HashMap<String, u32>,
    pub sample_size_warning: bool,       // true if < 20 showdowns
}

pub fn collect_ranges(
    hands: &[Hand],
    player: &str,
) -> HashMap<String, SpotRange>     // keyed by spot description
```

For each hand, determine the player's spot, check if they showed down, and record the entry. Also count total appearances in each spot (even without showdown) to give context on showdown frequency.

### Step 3: Range Grid Representation

**File:** `src/stats/ranges.rs`

For Hold'em, represent the range as a 13x13 grid (standard range chart):

```rust
pub struct RangeGrid {
    pub grid: [[u32; 13]; 13],      // frequency count
    pub total: u32,
    pub text: String,               // text representation like "AA, KK, QQ, AKs, AKo"
}

pub fn build_range_grid(entries: &[RangeEntry]) -> RangeGrid
```

Grid layout: row = first card rank, col = second card rank. Upper triangle = suited, lower triangle = offsuit, diagonal = pairs.

Text representation: list combos that appear at least once, sorted by conventional hand strength.

### Step 4: Expose via MCP

**File:** `src/mcp.rs`

New tool `get_hand_ranges`:
```rust
pub struct GetHandRangesParams {
    pub player: String,
    pub position: Option<String>,       // filter by position
    pub spot: Option<String>,           // filter by action line (e.g., "3-bet")
    pub min_showdowns: Option<u32>,     // minimum sample size (default 5)
    pub hero: Option<String>,
}
```

Returns:
```rust
pub struct RangeAnalysis {
    pub player: String,
    pub spots: Vec<SpotRange>,
    pub overall_showdown_hands: Vec<RangeEntry>,
    pub overall_grid: RangeGrid,
}
```

### Step 5: Sample Size Guidance

Include clear warnings in the response:
- < 10 showdowns: "Very small sample — range is unreliable"
- 10–30 showdowns: "Small sample — treat as directional only"
- 30–100 showdowns: "Moderate sample — patterns are emerging"
- 100+ showdowns: "Reliable sample size"

## Dependencies

- Showdown analysis plan (for `HandCategory` and `categorize_holecards`)
- Existing `classify_pot_type()` in `src/stats/mod.rs`
- Existing `Hand.actions` for action line reconstruction
- No new crates needed

## Limitations

- Only analyzes hands that reached showdown — players' full range is wider than what they show down with (survivorship bias)
- Omaha ranges are harder to represent in a grid format — may need a different visualization
- Small samples are inevitable for specific spots — must be clearly communicated

## Testing

- Unit tests for `classify_spot` with known action sequences
- Unit tests for `build_range_grid` with known card combinations
- Test text representation matches expected format
- Test sample size warnings trigger at correct thresholds
- Integration test with `PolarFox/` data for a frequent opponent
