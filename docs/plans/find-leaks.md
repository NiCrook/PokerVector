# find_leaks MCP Tool

**Category:** New MCP Tools
**Summary:** Flag statistical anomalies and exploitable patterns in a player's game.

## Current State

- `PlayerStats` provides 28 stats covering preflop and postflop play
- `PositionStats` gives per-position VPIP/PFR (to be expanded per positional-stats plan)
- Stats are computed in-memory from `Vec<Hand>` via `calculate_stats()`
- No automated analysis of whether stats indicate leaks or weaknesses

## What's Missing

1. **Leak definition framework** — structured rules for what constitutes a leak
2. **Contextual thresholds** — leaks should depend on game type (6-max vs 9-max, stakes level)
3. **Severity ranking** — which leaks cost the most money
4. **Actionable advice** — what to do about each leak
5. **Positional leaks** — leaks that only appear in specific positions

## Build Plan

### Step 1: Leak Detection Framework

**File:** `src/stats/leaks.rs` (new module)

```rust
pub struct Leak {
    pub id: String,                 // unique identifier, e.g., "preflop_too_loose"
    pub category: LeakCategory,
    pub stat_name: String,          // which stat triggered the leak
    pub actual_value: f64,
    pub expected_range: (f64, f64), // (low, high) for the acceptable range
    pub severity: LeakSeverity,
    pub title: String,
    pub description: String,        // detailed explanation
    pub suggestion: String,         // actionable advice
    pub position: Option<String>,   // if position-specific
}

pub enum LeakCategory {
    Preflop,
    Postflop,
    Positional,
    Aggression,
    Showdown,
}

pub enum LeakSeverity {
    Minor,      // suboptimal but not costly
    Moderate,   // noticeable impact on winrate
    Major,      // significant leak, priority fix
}

pub struct LeakReport {
    pub player: String,
    pub hands_analyzed: u32,
    pub leaks: Vec<Leak>,
    pub strengths: Vec<String>,     // stats that are in a good range
    pub overall_assessment: String, // brief narrative
}
```

### Step 2: Leak Rules Engine

**File:** `src/stats/leaks.rs`

Define leak rules as a collection of threshold checks. Each rule is a function:

```rust
type LeakRule = fn(stats: &PlayerStats, table_size: u8) -> Option<Leak>;
```

**Preflop leaks:**
| Rule | 6-max Range | 9-max Range | Severity |
|---|---|---|---|
| VPIP too high | >32% | >22% | Moderate |
| VPIP too low | <20% | <14% | Minor |
| PFR too low relative to VPIP | gap >10% | gap >8% | Moderate |
| 3-bet too low | <5% | <4% | Minor |
| 3-bet too high | >12% | >10% | Minor |
| Fold to 3-bet too high | >68% | >68% | Major |
| Fold to 3-bet too low | <35% | <35% | Moderate |
| Limp% too high | >8% | >5% | Moderate |
| Cold call too high | >12% | >10% | Minor |
| Steal too low (CO/BTN) | <25% | <25% | Moderate |

**Postflop leaks:**
| Rule | Range | Severity |
|---|---|---|
| C-bet flop too high | >75% | Moderate |
| C-bet flop too low | <40% | Moderate |
| C-bet turn too low (given flop cbet) | <35% | Minor |
| Fold to c-bet flop too high | >60% | Major |
| Fold to c-bet turn too high | >65% | Moderate |
| AF too low | <1.5 | Moderate |
| AF too high | >4.5 | Minor |
| WWSF too low | <42% | Moderate |
| Check-raise too low | <5% | Minor |
| Donk bet too high | >10% | Minor |

**Showdown leaks:**
| Rule | Range | Severity |
|---|---|---|
| WTSD too high | >35% | Moderate |
| WTSD too low | <22% | Minor |
| W$SD too low | <48% | Major |

### Step 3: Table Size Detection

Leak thresholds differ by table size. Determine the dominant table size from the hand data:
```rust
fn dominant_table_size(hands: &[Hand]) -> u8
```

Use mode of `hand.table_size` across all hands. Apply 6-max thresholds for <=6 players, 9-max for >6.

### Step 4: Positional Leak Detection

If expanded `PositionStats` are available (per positional-stats plan), check for position-specific leaks:
- VPIP from UTG > 18% (too loose)
- PFR from BTN < 25% (too tight)
- VPIP from SB > 30% (too loose)
- Fold to steal from BB > 70% (too exploitable)

### Step 5: Expose via MCP

**File:** `src/mcp.rs`

New tool `find_leaks`:
```rust
pub struct FindLeaksParams {
    pub player: Option<String>,     // defaults to hero
    pub category: Option<String>,   // filter to "preflop", "postflop", etc.
    pub hero: Option<String>,
}
```

Returns `LeakReport` as JSON.

Can be used on hero (self-improvement) or on villains (finding exploits).

## Dependencies

- Existing `calculate_stats()` — provides all stat values
- Positional stats plan (optional, for position-specific leaks)
- No new crates needed

## Testing

- Unit tests for each leak rule with stats at boundary values
- Test that a "perfect" stat line produces no leaks
- Test that an extremely bad stat line triggers all relevant leaks
- Test 6-max vs 9-max threshold selection
- Verify severity ordering is consistent
- Integration test with `PolarFox/` data
