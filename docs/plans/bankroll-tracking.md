# Bankroll Tracking

**Category:** Richer Analysis
**Summary:** Profit/loss curves, stakes progression, and bankroll management insights.

## Current State

- Each `Hand` stores `pot`, `rake`, `result.winners`, and all `Action` amounts
- `hero_profit()` in `src/sessions.rs` computes per-hand profit (collected minus invested)
- `PlayerStats.winrate_bb100` gives overall win rate in BB/100
- Sessions track `net_profit` and `net_profit_bb`
- Qdrant payload stores `stakes` (e.g., `"$0.01/$0.02"`), `game_type` (`"cash"/"tournament"`), and `timestamp`
- No persistent bankroll tracking — everything is computed on the fly

## What's Missing

1. **Cumulative P&L curve** across all sessions (not just per-session)
2. **P&L by stakes level** — separate tracking for each blind level
3. **Stakes progression** — when did the player move up/down
4. **Bankroll health indicators** — current bankroll relative to stakes, risk of ruin estimates
5. **Tournament ROI** — buy-in vs cashout tracking

## Build Plan

### Step 1: Bankroll Computation Module

**File:** `src/bankroll.rs` (new module)

```rust
pub struct BankrollPoint {
    pub hand_id: u64,
    pub timestamp: String,
    pub profit: f64,
    pub cumulative_profit: f64,
    pub stakes: String,
}

pub struct StakesBreakdown {
    pub stakes: String,
    pub hands: u32,
    pub profit: f64,
    pub winrate_bb100: f64,
    pub first_hand: String,   // timestamp
    pub last_hand: String,    // timestamp
}

pub struct BankrollSummary {
    pub total_hands: u32,
    pub total_profit: f64,
    pub total_rake_paid: f64,
    pub biggest_winning_hand: (u64, f64),   // (hand_id, profit)
    pub biggest_losing_hand: (u64, f64),
    pub peak_profit: f64,
    pub max_drawdown: f64,                  // largest peak-to-trough decline
    pub stakes_breakdown: Vec<StakesBreakdown>,
    pub pnl_curve: Vec<BankrollPoint>,
}
```

Add `compute_bankroll(hands: &[Hand], hero: &str) -> BankrollSummary`:
1. Sort hands by timestamp
2. Iterate, computing per-hand profit via `hero_profit()`
3. Track cumulative profit, peak, drawdown
4. Group by stakes for `StakesBreakdown`
5. Track rake from `hand.rake`

### Step 2: Drawdown Analysis

**File:** `src/bankroll.rs`

Add drawdown tracking:
```rust
pub struct Drawdown {
    pub start_hand_id: u64,
    pub end_hand_id: u64,
    pub start_timestamp: String,
    pub end_timestamp: String,
    pub peak_profit: f64,
    pub trough_profit: f64,
    pub drawdown_amount: f64,
    pub hands_to_recover: Option<u32>,  // None if still in drawdown
}
```

Add `find_drawdowns(pnl_curve: &[BankrollPoint], min_bb: f64) -> Vec<Drawdown>` — identify all significant drawdown periods.

### Step 3: Stakes Progression Timeline

**File:** `src/bankroll.rs`

Add `StakesChange`:
```rust
pub struct StakesChange {
    pub timestamp: String,
    pub from_stakes: String,
    pub to_stakes: String,
    pub direction: String,  // "moved_up", "moved_down", "same"
    pub bankroll_at_change: f64,
}
```

Detect stakes changes by scanning hands in timestamp order and noting when the `stakes` field changes. Group consecutive hands at the same stakes level.

### Step 4: Tournament ROI (if tournament data exists)

**File:** `src/bankroll.rs`

```rust
pub struct TournamentSummary {
    pub tournaments_played: u32,
    pub total_buy_ins: f64,
    pub total_cashes: f64,
    pub roi_pct: f64,
    pub itm_pct: f64,           // in-the-money percentage
    pub avg_buy_in: f64,
    pub biggest_cash: f64,
}
```

Tournament hands have `GameType::Tournament { tournament_id, .. }`. Group by `tournament_id`, identify cash amounts from the final hand's result.

Note: ACR hand histories may not contain enough info to determine buy-in amounts or final tournament results. This step may need a separate tournament results import or may be limited to what's derivable from HH data.

### Step 5: Expose via MCP

**File:** `src/mcp.rs`

Add new tool `get_bankroll`:
```rust
pub struct GetBankrollParams {
    pub stakes: Option<String>,     // filter to specific stakes
    pub game_type: Option<String>,  // "cash" or "tournament"
    pub hero: Option<String>,
}
```

Returns `BankrollSummary` with P&L curve, drawdowns, stakes breakdown.

Consider a `compact` param that omits the full `pnl_curve` (which could be thousands of points) and only returns summary stats.

## Dependencies

- Existing `hero_profit()` from `src/sessions.rs` — may want to move to a shared `helpers` module
- Existing `VectorStore::scroll_hands()` for data retrieval
- No new crates needed

## Testing

- Unit tests for `compute_bankroll` with synthetic hand sequences at known profit levels
- Unit tests for drawdown detection with known peak/trough patterns
- Unit tests for stakes change detection
- Verify P&L curve sums match total profit
- Integration test with `PolarFox/` data
