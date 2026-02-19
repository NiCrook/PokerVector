# get_session_summary MCP Tool

**Category:** New MCP Tools
**Summary:** Recap a session with key hands and P&L.

## Current State

- `list_sessions` MCP tool already exists — returns session list with `session_id`, `start_time`, `end_time`, `duration_minutes`, `total_hands`, `net_profit`, `net_profit_bb`, and table names
- `review_session` MCP tool already exists — returns `SessionReview` with `PlayerStats` + top-3 wins/losses as `NotableHand`
- `Session` and `SessionReview` structs in `src/sessions.rs` are well-defined
- Missing: a single-call summary that combines the most useful parts into a concise, LLM-friendly recap

## What's Missing

1. **Concise natural language recap** — current tools return raw JSON, no narrative
2. **Key decision points** — not just biggest wins/losses, but most interesting spots
3. **Performance vs. baseline** — how did this session compare to overall stats
4. **Streak/momentum info** — was there a hot/cold streak within the session

## Build Plan

### Step 1: Session Narrative Generator

**File:** `src/sessions.rs` (extend existing)

Add `generate_session_narrative(review: &SessionReview, overall_stats: &PlayerStats) -> String`:

Template-based natural language summary (similar to how `summarizer.rs` works for hands):

```
Session #{id} — {date}, {duration} minutes across {table_count} table(s).
{total_hands} hands at {stakes}. Result: {+/-}{profit} ({profit_bb} BB).

Performance: VPIP {vpip}% (overall {overall_vpip}%), PFR {pfr}% (overall {overall_pfr}%).
Went to showdown {wtsd}% and won {wasd}% of the time.

Biggest win: Hand #{id} — {summary} (+{amount})
Biggest loss: Hand #{id} — {summary} (-{amount})

{streak_info if applicable}
{tilt_warning if applicable}
```

### Step 2: Interesting Hand Detection

**File:** `src/sessions.rs`

Expand beyond top-3 wins/losses. Add `find_interesting_hands()` that identifies:
- **Big pots relative to stakes** — pot > 50 BB
- **Bluff showdowns** — hero or villain won with a weak hand category
- **Suckouts** — all-in with significant equity disadvantage (approximated by final board texture vs. shown hands)
- **Multi-way all-ins** — 3+ players all-in
- **Hero folds to big bets** — folded facing a bet > pot on river

```rust
pub struct InterestingHand {
    pub hand_id: u64,
    pub reason: String,
    pub category: String,    // "big_pot", "bluff", "suckout", "multi_way", "big_fold"
    pub profit: f64,
    pub summary: String,
}
```

### Step 3: Combine into get_session_summary Tool

**File:** `src/mcp.rs`

New tool `get_session_summary`:
```rust
pub struct GetSessionSummaryParams {
    pub session_id: u32,
    pub hero: Option<String>,
}
```

Response includes:
1. Natural language narrative
2. Key stats (session stats + comparison to overall)
3. Interesting hands (expanded beyond just wins/losses)
4. P&L curve (from session-tracking plan)

This is a higher-level tool than `review_session` — designed to give an MCP client (LLM) everything it needs to provide a useful session debrief in one call.

### Step 4: Latest Session Shortcut

Add a convenience: if `session_id` is omitted, return the most recent session. This is the most common use case — "how did my last session go?"

## Dependencies

- Existing `sessions.rs` — `detect_sessions()`, `review_session()`
- Existing `stats/` — `calculate_stats()` for overall baseline comparison
- Session tracking plan (for P&L curve and tilt indicators, but can be built independently)
- No new crates needed

## Testing

- Unit test for narrative generation with known session data
- Unit test for interesting hand detection with mock hands covering each category
- Integration test: call `get_session_summary` on `PolarFox/` data and verify response structure
- Test edge cases: single-hand session, session with no showdowns, multi-table session
