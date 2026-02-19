# Session Tracking

**Category:** Richer Analysis
**Summary:** Group hands by session, track win/loss over time, session duration, and tilt detection.

## Current State

`src/sessions.rs` already provides foundational session detection:
- `detect_table_sessions()` groups hands by `table_name` and splits on >5 minute gaps
- `detect_sessions()` merges table sessions with <30 minute gaps across tables
- `review_session()` computes `PlayerStats` + top-3 wins/losses as `NotableHand`
- `Session` struct has `session_id`, `start_time`, `end_time`, `duration_minutes`, `tables`, `total_hands`, `net_profit`, `net_profit_bb`
- MCP tools `list_sessions` and `review_session` already expose this data

## What's Missing

1. **Win/loss over time within a session** — no running P&L curve per hand
2. **Tilt detection** — no behavioral analysis after big losses
3. **Session persistence** — sessions are recomputed every call from scrolled hands
4. **Multi-session trends** — no comparison across sessions

## Build Plan

### Step 1: Running P&L Curve

**File:** `src/sessions.rs`

- Add a `PnlPoint` struct: `{ hand_index: u32, hand_id: u64, timestamp: String, cumulative_profit: f64, cumulative_profit_bb: f64 }`
- Add `compute_pnl_curve(hands: &[Hand], hero: &str) -> Vec<PnlPoint>` — iterate hands in timestamp order, accumulate profit using existing `hero_profit()` helper
- Add `pnl_curve: Vec<PnlPoint>` field to `SessionReview`
- Call `compute_pnl_curve()` inside `review_session()`

### Step 2: Tilt Detection

**File:** `src/sessions.rs` (new section) or `src/tilt.rs` (new module)

Define tilt indicators by comparing a sliding window of N hands against session baseline:
- **VPIP spike** — VPIP in window > session VPIP + threshold (e.g., +15%)
- **Aggression spike** — AF jumps significantly after a big loss
- **Loss streak** — 3+ consecutive losses exceeding 5 BB each
- **Stack-off frequency** — all-in rate increases after a loss > 20 BB

Add struct:
```rust
pub struct TiltIndicator {
    pub hand_id: u64,
    pub timestamp: String,
    pub indicator_type: String,   // "vpip_spike", "loss_streak", etc.
    pub description: String,      // human-readable explanation
    pub severity: f64,            // 0.0–1.0
}
```

Add `detect_tilt(hands: &[Hand], hero: &str) -> Vec<TiltIndicator>` — scan session hands with a sliding window (default 10 hands), compare stats against full-session stats.

Include `tilt_indicators: Vec<TiltIndicator>` in `SessionReview`.

### Step 3: Multi-Session Trends

**File:** `src/sessions.rs`

Add `SessionTrend` struct:
```rust
pub struct SessionTrend {
    pub session_count: u32,
    pub total_hands: u32,
    pub total_profit: f64,
    pub total_profit_bb: f64,
    pub avg_session_duration: f64,
    pub avg_hands_per_session: f64,
    pub best_session: (u32, f64),    // (session_id, profit)
    pub worst_session: (u32, f64),
    pub winning_session_pct: f64,
}
```

Add `compute_trends(sessions: &[Session]) -> SessionTrend`.

### Step 4: Expose via MCP

**File:** `src/mcp.rs`

- Extend `review_session` response to include `pnl_curve` and `tilt_indicators` (no new tool needed)
- Add new tool `get_session_trends` with params `{ limit?: u32 }` — scrolls hands, detects sessions, computes trends

## Dependencies

- Existing `sessions.rs` module (foundation already built)
- Existing `stats.rs` for per-window stat calculation
- No new crates needed

## Testing

- Unit tests for `compute_pnl_curve` with known hand sequences and expected cumulative values
- Unit tests for `detect_tilt` with synthetic hand sequences that trigger each indicator
- Unit tests for `compute_trends` with multiple mock sessions
- Integration test using `PolarFox/` data to verify sessions detect correctly and P&L sums match
