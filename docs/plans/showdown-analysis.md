# Showdown Analysis

**Category:** Richer Analysis
**Summary:** Analyze what hands opponents show down with, broken down by position and action line.

## Current State

- `ActionType::Shows { cards: Vec<Option<Card>>, description: Option<String> }` captures showdown reveals
- `Action { player, action_type, street: Showdown }` records who showed what
- `Hand.result.winners` tracks who won and amounts
- `went_to_showdown` is stored in Qdrant payload as a boolean
- `won_at_showdown_pct` and `went_to_showdown_pct` exist in `PlayerStats`
- `Hand.players` has `position` for each player
- No analysis of *what* hands are shown down — only *whether* showdown occurred

## What's Missing

1. **Showdown hand collection** — gathering the actual cards shown per villain
2. **Categorization** — classifying shown hands (premium pairs, broadways, suited connectors, etc.)
3. **Positional showdown ranges** — what does villain show from each position
4. **Action-line correlation** — what does villain show when they 3-bet, c-bet, check-raise, etc.
5. **Showdown frequency by spot** — how often does villain reach showdown in specific lines

## Build Plan

### Step 1: Hand Category System

**File:** `src/stats/showdown.rs` (new module)

Define hand categories for Hold'em:
```rust
pub enum HandCategory {
    PremiumPair,        // AA, KK, QQ
    MediumPair,         // JJ–88
    SmallPair,          // 77–22
    BigBroadway,        // AK, AQ, AJ
    Broadway,           // KQ, KJ, QJ, etc.
    SuitedConnector,    // 87s, 76s, etc.
    SuitedAce,          // A2s–A9s
    OffsuitAce,         // A2o–A9o
    SuitedGapper,       // 86s, 75s, etc.
    Trash,              // everything else
}

pub fn categorize_holecards(cards: &[Card]) -> HandCategory
```

For Omaha, categorize by key features: double-suited, rundown, paired, etc. (separate enum or extend).

### Step 2: Showdown Data Extraction

**File:** `src/stats/showdown.rs`

```rust
pub struct ShowdownEntry {
    pub hand_id: u64,
    pub player: String,
    pub cards: Vec<Card>,
    pub category: HandCategory,
    pub position: Option<Position>,
    pub action_line: String,       // e.g., "open-raise, c-bet, barrel, barrel"
    pub won: bool,
    pub pot_type: String,          // from classify_pot_type
}

pub fn extract_showdowns(hands: &[Hand], player: &str) -> Vec<ShowdownEntry>
```

For each hand, find `Shows` actions for the target player, extract their cards (filter out `None` partial reveals), determine their action line by summarizing their actions per street.

### Step 3: Action Line Summarization

**File:** `src/stats/showdown.rs`

Convert a player's actions into a compact action line string:
```rust
pub fn summarize_action_line(hand: &Hand, player: &str) -> String
```

Examples:
- `"open-raise → c-bet → barrel → barrel"` (aggressive line)
- `"call → check-call → check-call"` (passive line)
- `"3-bet → c-bet → check"` (3-bet then gave up)
- `"limp → call → fold"` (limp-call then folded)

Key actions to track per street: open, raise, 3-bet, call, check, check-raise, bet (c-bet/donk/probe), fold.

### Step 4: Showdown Range Summary

**File:** `src/stats/showdown.rs`

```rust
pub struct ShowdownRangeSummary {
    pub player: String,
    pub total_showdowns: u32,
    pub category_distribution: HashMap<HandCategory, u32>,
    pub by_position: HashMap<String, Vec<ShowdownEntry>>,
    pub by_action_line: HashMap<String, Vec<ShowdownEntry>>,
    pub by_pot_type: HashMap<String, Vec<ShowdownEntry>>,
}

pub fn summarize_showdown_range(entries: &[ShowdownEntry]) -> ShowdownRangeSummary
```

### Step 5: Expose via MCP

**File:** `src/mcp.rs`

Add new tool `get_showdown_analysis`:
```rust
pub struct GetShowdownAnalysisParams {
    pub player: String,             // villain to analyze
    pub position: Option<String>,   // filter by position
    pub pot_type: Option<String>,   // filter by pot type
    pub hero: Option<String>,
}
```

Returns `ShowdownRangeSummary` — what this player shows down with, broken down by position and action line.

## Dependencies

- Existing `stats/` module — new submodule
- Existing `classify_pot_type()` in `src/stats/mod.rs`
- `Hand.actions` for action line extraction
- No new crates needed

## Testing

- Unit tests for `categorize_holecards` with representative hands from each category
- Unit tests for `summarize_action_line` with known action sequences
- Unit tests for `extract_showdowns` using hands with `Shows` actions from `PolarFox/` test data
- Verify category distribution sums match total showdowns
- Test partial reveals (`None` cards) are handled gracefully
