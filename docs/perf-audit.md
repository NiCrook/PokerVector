# PokerVector Performance Audit

Date: 2026-02-24

## Critical / High Priority

### 1. Regex Recompilation on Every Call — `acr.rs`

**Problem:** Several parse functions call `Regex::new(...).unwrap()` inside the function body. However, the actual call frequency varies significantly by function — not all are "per-line":

Offenders (with actual call frequency per hand):
- `parse_header` (lines 338, 370): exactly **1 regex** per hand — `starts_with("Game Hand #")` and `starts_with("Hand #")` guards (lines 337, 369) ensure only the matching branch compiles its regex. Called **once per hand** (line 60)
- `parse_table_line` (lines 405, 416, 427): **no `starts_with` guards** — the tournament regex (line 405) is always compiled first. If it fails, cash regex (line 416) is compiled. If that fails, stud regex (line 427). Cash hands: **2 compilations**. Tournament: **1**. Stud: **3**. Called **once per hand** (line 66)
- `parse_seat_line` (line 444): 1 regex, called **~2-9 times per hand** (seat loop, lines 72-82)
- `parse_uncalled_bet` (line 632): 1 regex, called **0-1 times per hand** (guarded by `starts_with` check, line 247)
- `parse_stud_dealt_line` (lines 677, 700): The same regex `\[([^\]]+)\]` is compiled once per call to `parse_stud_dealt_line`, then `find_iter` reuses that compiled regex for all bracket matches. Hero lines hit line 677 (1 compilation), non-hero lines hit line 700 (1 compilation) — these are mutually exclusive branches. So it's **1 compilation per dealt line**, not 2. Only applies to **stud hands** (guarded by `is_stud` check, line 232)
- `extract_player_name_from_summary` (line 617): 1 regex, called only from `parse_summary_seat_line` which is guarded by `" and won "` check (line 566) — only fires for **winning seats**, typically **1-2 per hand**

For 1,000 non-stud hands (~80% cash, ~20% tournament): 1,000 header (exactly 1 each) + ~1,800 table (2 for cash, 1 for tournament) + ~6,000 seat + ~600 uncalled + ~1,100 summary winners = **~10,500** regex compilations. The original audit's "30,000" figure assumed every function is called per-line, which is incorrect — the main parse loop (lines 134-266) dispatches to specific functions via `starts_with` guards.

For stud hands, `parse_stud_dealt_line` compiles the bracket regex once per dealt line (not per match — `find_iter` reuses the compiled regex). In a stud hand with ~7 players × ~5 streets = ~35 dealt lines = ~35 regex compilations per stud hand. Less severe than originally claimed but still wasteful for stud-heavy datasets.

**Fix:** Use `std::sync::OnceLock` to compile each regex exactly once:

```rust
use std::sync::OnceLock;

fn seat_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"^Seat (\d+): (.+?) \((\$?[0-9.]+)\)(?:\s+is sitting out)?$"
    ).unwrap())
}
```

**Impact:** Eliminates ~10,500 regex compilations per 1,000 hands (non-stud). However, the main parsing hot path — `try_parse_action` (lines 716-930) — uses NO regex at all, relying entirely on `starts_with`/`strip_prefix` string matching. This function is called ~15-25 times per hand and is the dominant per-line cost. Regex functions only fire at bounded rates for structural lines (header/table: once each, seats: ~6, summary winners: ~1-2).

The actual share of regex compilation in total parser time is **unknown without benchmarking**. `Regex::new` cost depends on pattern complexity (the seat/header patterns with multiple groups and alternations are more expensive than the simple bracket pattern in stud). A rough estimate: maybe 1.5-3x parser speedup, but this is a guess. Zero risk — pure performance change with identical semantics.

Still the highest-leverage parser change since it's mechanical and free.

---

### 2. Per-Hand `hand_exists` Database Round-Trip — `importer.rs`

**Problem:** For every hand in a batch, `store.hand_exists(hand.id).await?` is called individually (line 60). Each call runs `count_rows(Some(format!("id = {}", hand_id)))` (storage.rs:526-534). Note: LanceDB is embedded (not a network database), so these are local operations, not network round-trips. However, there is **no index on the `id` column** (no `create_index` calls in `storage.rs`) — so each `count_rows` does a full column scan of all uint64 IDs. For 5,000 hands against a 10,000-row table, that's 5,000 full scans.

**Fix:** Add a `get_existing_ids` method to `VectorStore` that checks all IDs in a single `IN (...)` query:

```rust
pub async fn get_existing_ids(&self, ids: &[u64]) -> Result<HashSet<u64>> {
    let id_list = ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
    let filter = format!("id IN ({})", id_list);
    // single query, select id column only, return HashSet of existing IDs
}
```

**Impact:** Reduces N full-table scans to ceil(N/32). For 1,000 hands: 1,000 scans → 32. Each scan reads all uint64 IDs from disk/cache — with no index, the per-query cost is proportional to table size. Actual speedup on the dedup phase is likely **2-10x** (dominated by scan I/O and async scheduling overhead per query).

**Alternative fix:** Create a scalar index on the `id` column (`table.create_index(["id"]).execute().await`). This would make individual `count_rows(id = X)` lookups O(log N) instead of O(N), potentially making the batching optimization unnecessary. Worth investigating which approach is simpler.

---

### ~~3. Two Sequential ONNX Inference Passes — `importer.rs`~~ (WITHDRAWN)

**Original claim:** Combine summary and action embedding into one `embed_batch` call for ~30-50% speedup.

**Why this is wrong:** The embedder pads all texts to `max_len` across the batch (embedder.rs:55). Summaries are natural language paragraphs (~50-150 tokens), while action encodings are compact structured strings (~20-80 tokens). Separate batches pad each group to its own max: summaries to ~150 tokens, actions to ~80 tokens. A combined batch pads ALL 64 texts to ~150 tokens — wasting compute on 32 over-padded action texts.

Standard BERT/BGE ONNX models apply the attention mask as an additive mask before softmax — but the QKV projections and attention score matmuls still run on ALL positions including padded ones. The mask only zeroes out softmax output for padded positions. So padded tokens do consume compute: O(batch × num_heads × seq_len²) for attention, O(batch × seq_len × hidden_dim) for projections.

The per-call overhead (tensor allocation, ORT boundary crossing) is minimal relative to the actual inference. **Do not combine the batches.**

---

### 4. `scroll_hands` Loads All Hands with No Limit — `storage.rs`

**Problem:** Every MCP tool that computes stats calls `scroll_hands` which deserializes the full `hand_json` for every matching hand. 10,000 hands × ~5 KB = 50 MB of JSON deserialized per stats query.

There are **32 production call sites** across the MCP tool modules: `tools_advanced.rs` (9), `tools_villains.rs` (5), `tools_stats.rs` (5), `tools_spots.rs` (5), `tools_export.rs` (3), `tools_sessions.rs` (2), `tools_hands.rs` (2), `tools_meta.rs` (1). Plus 3 test call sites in `storage.rs`.

`scroll_hands` has no limit or pagination mechanism at all. (Note: the `limit`/`offset` fields on `SearchParams` are only used by `search_hands` for vector search pagination — `build_filter` correctly ignores them since they're not WHERE-clause filters. Every MCP tool that calls `scroll_hands` passes `limit: None, offset: None`.)

**Fix (short-term):** Adding a `limit` parameter to `scroll_hands` would NOT help most callers. Stats tools (`get_stats`, `get_villain_stats`, `get_position_stats`, etc.) fundamentally need ALL matching hands to compute accurate percentages. Session tools need all hands to detect table sessions. Villain tools need all hands to count frequencies. A `limit` would only help the few tools that discard most results after loading (e.g., `tools_hands.rs:502` loads all hands for a stakes level then `.retain()`s to one table).

**Fix (medium-term):** The real problem is that `scroll_hands` deserializes the full `hand_json` (including `raw_text`, detailed actions, player data) when most callers only need a subset of fields. A more targeted approach:
- For stats: load only the `hand_json` but with a lighter deserialization (skip `raw_text`, skip `stud_cards`)
- For sessions: load only `id`, `timestamp`, `table_name`, `game_type` columns directly (no JSON deserialization needed)
- For villains: load only `opponent_names` column for counting, then full hands only for the top-N villains

**Fix (long-term):** Pre-compute stat-relevant boolean/counter columns at import time (e.g., `is_vpip`, `is_pfr`, `saw_flop`). Many stats could become SQL aggregations. However, complex stats (3-bet%, c-bet, check-raise) require sequential action analysis that can't be expressed in SQL — these would still need Rust-side computation on filtered subsets.

**Impact:** Depends on approach and database size. The severity scales with hand count:
- 500 hands (~2.5 MB JSON): ~20-50ms deserialize — **not a problem**
- 5,000 hands (~25 MB JSON): ~200-500ms — noticeable latency per MCP call
- 50,000 hands (~250 MB JSON): ~2-5 seconds — unacceptable, and repeated on every tool call with no caching between MCP calls

The session optimization (column-only query) could be 10-50x faster for that specific tool. Stats optimization is harder and requires careful validation. For small databases (current use case: 18 hand history files), this is likely not yet a bottleneck.

---

## Medium Priority

### 5. `HashMap<String, f64>` in PotTracker — `action_encoder.rs`

**Problem:** `PotTracker.current_round` is keyed by cloned player name Strings via `entry(action.player.clone())`. Clone happens for: PostSmallBlind, PostBigBlind, PostBlind, BringsIn, Call, Bet, Raise. Does NOT happen for: PostAnte (line 72 — doesn't use current_round), UncalledBet, Fold, Check, and the `_ =>` wildcard. For a typical 6-max hand: 1 SB + 1 BB + ~5-8 money actions = **~7-10 clones per hand**, not 15-30.

Note: `HashMap::entry()` takes the key by value, so the clone happens even if the entry already exists (the cloned String is then dropped for occupied entries).

**Fix:** Use `HashMap<&str, f64>` with lifetime tied to Hand:

```rust
struct PotTracker<'a> {
    current_round: HashMap<&'a str, f64>,
    // ...
}
```

This works because `action.player` borrows from `Hand.actions` which outlives `PotTracker`.

**Impact:** Eliminates ~7-10 String clones per hand. 10,000 hands = 70K-100K fewer heap allocations. Realistic improvement is **< 5%** on action encoding — the allocations are small (player names are ~5-15 bytes) and HashMap with 2-9 entries is fast regardless. The absolute time saved is small.

Note: `try_parse_action` (lines 746-930) also clones player names — 16 `name.clone()` sites, 1 per successfully parsed action = ~15-25 clones per hand. This is larger than PotTracker but inherent to the data model (`Action` owns its player String). Fixing this would require changing `Action.player` to use `&str` with lifetimes or string interning — a much bigger refactor across the entire codebase.

---

### 6. `Option<Vec<Option<f32>>>` Arrow Wrapping — `storage.rs`

**Problem:** Each 384-float embedding is wrapped element-by-element in `Option<f32>` (lines 231-238). For 32 hands: 32 × 384 = 12,288 unnecessary `Option` wraps, plus 32 Vec allocations. Nullability is never used.

**Fix:** Build `FixedSizeListArray` from a flat `Float32Array` instead of `from_iter_primitive` with double-Option wrapping:

```rust
use arrow_array::Float32Array;

let summary_flat: Vec<f32> = embeddings.iter()
    .flat_map(|e| e.summary.iter().copied())
    .collect();

let child = Arc::new(Float32Array::from(summary_flat));
let field = Arc::new(Field::new("item", DataType::Float32, true));
let summary_arr = FixedSizeListArray::try_new(field, EMBEDDING_DIM, child, None).unwrap();
```

Note: The current code uses `from_iter_primitive::<Float32Type, _, _>` (lines 289-292) which requires the `Option<Vec<Option<f32>>>` shape. The `try_new` constructor avoids this by taking a flat child array directly.

**Impact:** Eliminates O(batch × 384) Option wrapping and 32 intermediate Vec allocations. Better memory layout. However, this is in the upsert path which is I/O-bound (writing to LanceDB), so the CPU savings are minor relative to total upsert time. **Low practical impact.**

---

### 7. `raw_text` Stored in Every Hand JSON — `types.rs`, `acr.rs`

**Problem:** `Hand.raw_text` stores the full raw hand history (~600-2200 bytes, avg ~1.5 KB based on test fixtures) in every Hand struct. Serialized into `hand_json` in LanceDB. Stats/session code never uses it. 10,000 hands × 1.5 KB = ~15 MB of unused raw text loaded by `scroll_hands`.

**Fix:** Either `#[serde(skip)]` on `raw_text` and store it in a separate LanceDB column, or don't store it at all.

**Caveat:** `raw_text` is used by the `get_hand` MCP tool to return original hand history text to users. A simple `#[serde(skip)]` would break that tool. The fix must either: (a) store `raw_text` in its own LanceDB column and fetch it only in `get_hand`, or (b) use a separate serialization path for `scroll_hands` that excludes `raw_text` while `get_hand` still deserializes the full JSON.

**Impact:** Reduces `hand_json` size by ~25-40% (raw text is ~1.5 KB avg vs ~2-4 KB for the structured JSON — the original 40-60% estimate was inflated). Reduces `scroll_hands` memory proportionally. **Risk:** Requires schema migration / re-import, and careful handling of `get_hand` backward compatibility.

---

## Low Priority / Quick Wins

### 8. `parse_timestamp` in Sort Comparators — `sessions.rs`

**Problem:** `parse_timestamp(&str)` is called inside sort comparator (line 149). O(N log N) comparisons × 2 parses each = ~9,000 parse calls for 500 hands.

**Fix:** Parse timestamps once, sort by key:

```rust
let mut keyed: Vec<(Option<NaiveDateTime>, &Hand)> = table_hands
    .iter()
    .map(|h| (parse_timestamp(&h.timestamp), *h))
    .collect();
keyed.sort_by_key(|(ts, _)| *ts);
```

**Impact:** O(N log N) → O(N) timestamp parses. Trivial fix, but trivial savings too — 500 hands × log2(500) × 2 ≈ 9,000 datetime parses at ~100ns each ≈ 0.9ms. Correct practice but not a measurable bottleneck.

---

### 9. `fmt_trimmed` Allocates 2 Strings per Call — `action_encoder.rs`

**Problem:** `fmt_trimmed` (line 196) allocates via `format!()` then `.to_string()` for each action token (5-20 per hand).

**Fix:** Write into a pre-allocated buffer:

```rust
fn fmt_trimmed_into(buf: &mut String, val: f64, decimals: usize) {
    let start = buf.len();
    write!(buf, "{:.prec$}", val, prec = decimals).unwrap();
    let trimmed_end = buf[start..].trim_end_matches('0').trim_end_matches('.').len();
    buf.truncate(start + trimmed_end);
}
```

**Impact:** Eliminates 2 allocations per action token, ~10-40 per hand.

---

### 10. `build_alias_map` Clones All Player Names — `action_encoder.rs`

**Problem:** `build_alias_map` creates `HashMap<String, String>` by cloning player names (line 152). Then `classify_action` clones the alias on every action (line 368).

**Fix:** Use `HashMap<&str, &'static str>`:

```rust
fn build_alias_map<'a>(hand: &'a Hand, hero: &str) -> HashMap<&'a str, &'static str> {
    static V_ALIASES: [&str; 9] = ["V1", "V2", "V3", "V4", "V5", "V6", "V7", "V8", "V9"];
    // ...
}
```

**Impact:** ~20-50 fewer String allocations per hand.

---

## Build Configuration

Add to `Cargo.toml`:

```toml
[profile.release]
# opt-level = 3 is already the default for release, no need to set it
lto = "thin"
codegen-units = 1
```

Notes:
- `strip = "symbols"` was in the original recommendation but is a **binary size** optimization, not a performance one. It removes debug symbols from the binary, reducing size but having zero effect on runtime speed. Include it if you want smaller binaries, not for performance.
- `codegen-units = 1` improves optimization but **significantly increases compile time** (no parallel codegen). Only use for final release builds, not during development.
- `lto = "thin"` is a good balance of optimization vs compile time. `lto = true` (fat LTO) provides slightly better optimization but much slower builds.

Build with native CPU features for AVX vectorization on embedding float loops:

```bash
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

Consider `#[inline]` on hot functions, though for a single-crate build the compiler likely inlines these already:
- `current_street()` in `acr.rs` (line 300) — trivial enum match, almost certainly auto-inlined
- `parse_money_or_chips()` in `acr.rs` (line 645) — small delegation function
- `fmt_trimmed()` in `action_encoder.rs` (line 195) — slightly larger, may benefit

Note: `#[inline]` mainly helps cross-crate boundaries. For this single-binary project with LTO enabled, the compiler has full visibility and will make its own inlining decisions. Low priority.

### Stats Multi-Pass Over Actions — `stats/`

**Problem:** `calculate_stats` (calculate.rs:96-283) calls ~20 separate analysis functions per hand, each independently iterating over `hand.actions`: `preflop_vpip_pfr`, `three_bet_analysis`, `fold_to_three_bet_analysis`, `four_bet_analysis`, `fold_to_four_bet_analysis`, `cbet_analysis`, `steal_analysis`, `limp_analysis`, `donk_bet_analysis`, `float_analysis`, `check_raise_by_street_analysis`, `probe_analysis`, `squeeze_analysis`, `cold_call_analysis`, `wwsf_analysis`, `overbet_analysis`, plus the inline `postflop_bets_raises` loop (lines 226-240). That's ~17-20 passes over the actions array per hand.

For a typical hand with 15 actions and 10,000 hands: ~15 × 20 × 10,000 = 3,000,000 action iterations. A single-pass approach could reduce this to 150,000.

**Fix:** Merge analysis functions into a single-pass accumulator that classifies each action once and updates all stat counters. This is a significant refactor.

**Impact:** ~10-20x fewer iterations over actions. However, since actions are small arrays (5-30 elements) that fit in L1 cache, and each iteration is simple comparisons, the real-world speedup is modest — probably 2-5x on the stats computation, which itself is dwarfed by the `scroll_hands` JSON deserialization (finding #4). **Low priority until #4 is fixed.**
