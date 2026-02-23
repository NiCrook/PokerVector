# Phase 2: Storage, Search & CLI Rewrite

**Goal:** Replace `src/storage.rs` and `src/search.rs` with LanceDB implementations, update `src/main.rs` call sites, and add `config::data_dir()`. These modules are tightly coupled — storage API changes break search, and signature changes break main.rs — so they must change together to maintain a compilable state.

**Revert:** `git revert` this commit restores all four modules.

**Depends on:** Phase 1 (dependencies compile, Send+Sync confirmed)

## Storage: `src/storage.rs`

### Internal Structure

```rust
pub struct VectorStore {
    db: lancedb::Connection,
    table: lancedb::Table,  // opened/created by ensure_table(), stored for reuse
    table_name: String,
}
```

Note: `ensure_table()` must be called before any other method. Consider making `new()` call it internally so the table handle is always available, or use a two-step `connect()` + `ensure_table()` pattern. If `Table` is not `Clone`, wrap in `Arc` or restructure access.

### Public API Changes

| Method | Signature Change |
|--------|-----------------|
| `new()` | `new(url, collection)` → `new(data_dir, table_name)`. May also call `ensure_table()` internally. |
| `ensure_collection()` | Renamed to `ensure_table()`. Creates LanceDB table if not exists. Returns `&self` or is folded into `new()`. |
| `search()` | `filter: Option<Filter>` → `filter: Option<String>` (SQL WHERE) |
| `scroll_hands()` | `filter: Option<Filter>` → `filter: Option<String>` (SQL WHERE) |
| All others | Signatures unchanged |

### `SearchResult` Simplification

Drop the `payload: HashMap<String, serde_json::Value>` indirection. Replace with named fields and add `Serialize`:

```rust
#[derive(serde::Serialize)]
pub struct SearchResult {
    pub hand_id: u64,
    pub score: f32,
    pub summary: String,
    pub hero_position: String,
    pub hero_cards: String,
    pub stakes: String,
    pub hero_result: String,
    pub pot_type: String,
}
```

This matches what `HandSearchResult` in `search.rs` already extracts — the two types merge into one. `Serialize` is required because `mcp.rs` serializes search results with `serde_json::to_string_pretty()`.

**Distance-to-score conversion:** LanceDB vector search returns a `_distance` column (lower = more similar), not a score (higher = more similar). For cosine distance, convert to a familiar score with `score = 1.0 - distance`. This preserves the existing semantics where higher score = better match, which MCP clients may depend on.

### Schema

Single LanceDB table `poker_hands`:

| Column | Arrow Type | Notes |
|--------|-----------|-------|
| `id` | `UInt64` | Merge key for upsert dedup |
| `hand_json` | `Utf8` | Full `Hand` as JSON |
| `summary_text` | `Utf8` | Natural language summary |
| `action_text` | `Utf8` | Action sequence encoding |
| `site` | `Utf8` | `"ACR"` |
| `game_type` | `Utf8` | `"cash"` / `"tournament"` |
| `variant` | `Utf8` | `"holdem"`, `"omaha"`, etc. |
| `betting_limit` | `Utf8` | `"no_limit"`, `"pot_limit"`, `"fixed_limit"` |
| `is_hi_lo` | `Boolean` | |
| `is_bomb_pot` | `Boolean` | |
| `stakes` | `Utf8` | `"$0.01/$0.02"` or `"L1 25/50"` |
| `table_size` | `UInt8` | |
| `hero` | `Utf8` | |
| `hero_position` | `Utf8` | |
| `hero_cards` | `Utf8` | Space-separated |
| `hero_result` | `Utf8` | `"won"`, `"lost"`, `"folded"`, `"sat_out"` |
| `board` | `Utf8` | Space-separated |
| `num_players` | `UInt64` | |
| `went_to_showdown` | `Boolean` | |
| `timestamp` | `Utf8` | UTC string |
| `pot_type` | `Utf8` | |
| `opponent_names` | `Utf8` | Stored as `,name1,name2,` (leading/trailing commas for precise LIKE matching) |
| `tournament_id` | `UInt64` | 0 if not tournament |
| `pot_amount` | `Float64` | 0.0 if missing |
| `summary` | `FixedSizeList<Float32, 384>` | Summary embedding vector |
| `action` | `FixedSizeList<Float32, 384>` | Action embedding vector |

Vector column names are `summary` and `action` — matching the existing `vector_name` strings used in `search.rs` and `mcp.rs` (no mapping needed).

`opponent_names` stored with leading/trailing commas so `LIKE '%,Fish,%'` won't false-match "Fishy" or "Kingfish".

### Method Implementation Notes

The API pseudocode below is illustrative — consult lancedb Rust crate docs for actual method names, signatures, and builder patterns.

**`new(data_dir, table_name)`:**
- Connect to LanceDB: `lancedb::connect(data_dir)...`
- Open or create the table (fold `ensure_table` logic here or call separately)
- Store the `Table` handle in the struct

**`ensure_table()`:**
Try opening the table — if it fails, create with empty schema.

**`upsert_hand()` / `upsert_hands_batch()`:**
Build a `RecordBatch` via `build_record_batch()` helper (replaces `build_point()`), then use LanceDB's merge-insert API with `id` as the merge key.

**`search(vector_name, query, limit, filter)`:**
Use LanceDB's vector search builder — set the column, limit, and optional SQL filter. Results come as `RecordBatch` — extract columns by name into `SearchResult` fields directly.

**`scroll_hands(filter)`:**
Use LanceDB's query builder with optional SQL filter. Collect the result stream (via `futures::TryStreamExt::try_collect()`), deserialize `hand_json` column from each batch.

**`get_hand(hand_id)`:** Query with SQL filter `id = {hand_id}`, limit 1, deserialize `hand_json`.

**`hand_exists(hand_id)`:** Same as `get_hand`, check if result is empty.

**`count()`:** Use LanceDB's row count API.

**`get_hand_vector(hand_id, vector_name)`:** Query with select on the vector column, extract from `FixedSizeListArray`.

---

## Search: `src/search.rs`

### `build_filter()` → returns `Option<String>`

```rust
pub fn build_filter(params: &SearchParams) -> Option<String> {
    let mut conditions = Vec::new();

    if let Some(ref pos) = params.position {
        conditions.push(format!("hero_position = '{}'", sanitize(pos)));
    }
    if let Some(ref villain) = params.villain {
        conditions.push(format!("opponent_names LIKE '%,{},%'", sanitize(villain)));
    }
    // ... etc for each field

    if conditions.is_empty() { None }
    else { Some(conditions.join(" AND ")) }
}
```

### Filter field mapping

| Field | SQL Fragment |
|-------|-------------|
| `position` | `hero_position = '{value}'` |
| `pot_type` | `pot_type = '{value}'` |
| `villain` | `opponent_names LIKE '%,{value},%'` |
| `stakes` | `stakes = '{value}'` |
| `result` | `hero_result = '{value}'` |
| `game_type` | `game_type = '{value}'` |
| `variant` | `variant = '{value}'` |
| `betting_limit` | `betting_limit = '{value}'` |

### SQL injection prevention

```rust
fn sanitize(value: &str) -> String {
    value.replace('\'', "''")
}
```

Column names are hardcoded, so no column injection risk.

### `search_hands()` — simplified

No longer needs to map from `SearchResult` payload HashMap to `HandSearchResult` — `SearchResult` now has named fields and derives `Serialize`. Remove `HandSearchResult` entirely; `search_hands()` returns `Vec<SearchResult>`.

### `search_similar_actions()` — stays in `search.rs`

Exclusion filter changes from Qdrant `must_not` to SQL string:

```rust
let exclude = format!("id != {}", hand_id);
let combined = match filter {
    Some(f) => format!("{} AND {}", f, exclude),
    None => exclude,
};
```

Function stays in `search.rs` (not moved to storage.rs) so `mcp.rs` import paths don't change.

### Remove all Qdrant imports

```rust
// Remove entirely:
use qdrant_client::qdrant::{Condition, Filter, PointId};
```

### Updated tests

- `test_build_filter_empty` — assert returns `None` (unchanged)
- `test_build_filter_position` — assert returns `Some("hero_position = 'BTN'")`
- `test_build_filter_multiple` — assert string contains all conditions joined by ` AND `
- New: `test_sanitize_single_quotes` — `O'Brien` → `O''Brien`
- New: `test_villain_uses_like_with_commas` — villain "Fish" produces `opponent_names LIKE '%,Fish,%'`

---

## CLI: `src/main.rs`

### Add `config::data_dir()` to `src/config.rs`

```rust
pub fn data_dir() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".pokervector").join("data")
}
```

This is added now (not deferred to Phase 3) because `main.rs` needs it for the `VectorStore::new()` calls.

### Update all 4 VectorStore call sites

```rust
// Before (each site):
let store = storage::VectorStore::new(&cfg.qdrant.url, &cfg.qdrant.collection).await?;
store.ensure_collection().await?;

// After (each site):
let data_dir = config::data_dir();
let store = storage::VectorStore::new(data_dir.to_str().unwrap(), "poker_hands").await?;
store.ensure_table().await?;  // or omit if new() handles it
```

Four call sites:
1. Import with explicit path (line ~206)
2. Import all accounts (line ~222)
3. Status command (line ~262)
4. MCP command (line ~287)

### Print message changes

- `"Connecting to Qdrant..."` → `"Opening database..."`
- Status: `"Qdrant: {} / {}"` → `"Data: {}"` with `data_dir.display()`
- Status errors: remove Docker instructions, replace with generic error message

---

## Impact on `mcp.rs`

**No changes needed.** All `mcp.rs` call sites work because:
- `build_filter()` return type changes from `Option<Filter>` to `Option<String>` — callers pass the result through to `scroll_hands()`, which now accepts `Option<String>`
- `search_similar_actions()` filter param changes from `Option<Filter>` to `Option<String>` — mcp.rs passes `None` which infers to either type
- `search_hands()` now returns `Vec<SearchResult>` instead of `Vec<HandSearchResult>` — both are serializable, mcp.rs just calls `to_string_pretty()` on the result
- `VectorStore::new()` is not called in mcp.rs (only in main.rs)

---

## Storage Tests

```rust
#[tokio::test]
async fn test_ensure_table_creates_new() {
    let dir = tempfile::tempdir().unwrap();
    let store = VectorStore::new(dir.path().to_str().unwrap(), "test").await.unwrap();
    store.ensure_table().await.unwrap();
    assert_eq!(store.count().await.unwrap(), 0);
}

#[tokio::test]
async fn test_upsert_and_get_round_trip() {
    // Upsert a hand, get_hand by ID, verify fields match
}

#[tokio::test]
async fn test_hand_exists() {
    // Upsert, verify exists=true, check non-existent=false
}

#[tokio::test]
async fn test_scroll_with_filter() {
    // Upsert hands with different game_type, filter by "game_type = 'cash'"
}
```

All tests use `tempfile::tempdir()` — no Docker needed.

## Success Criteria

- `cargo build` succeeds
- All storage and search unit tests pass
- No Qdrant imports remain in `storage.rs` or `search.rs`
- `mcp.rs` compiles without changes
- `cargo run -- status` works (shows data dir + count)
- `cargo run -- import ./PolarFox/` works end-to-end
