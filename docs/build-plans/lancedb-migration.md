# LanceDB Migration Plan

Migrate the vector storage layer from Qdrant (client-server, requires Docker) to LanceDB (embedded, local directory). This eliminates the Docker dependency and makes PokerVector distributable as a single binary.

## Current State

- `src/storage.rs` — `VectorStore` wraps `qdrant_client::Qdrant`, manages a single collection with named vectors ("summary" + "action", 384-dim cosine)
- `src/search.rs` — `SearchMode` enum routes to the correct named vector, builds Qdrant `Filter` objects from search params
- `src/mcp.rs` — `PokerVectorMcp` holds `Arc<VectorStore>`, 7 tools that call search/scroll/get
- `src/main.rs` — `import` command upserts hands, `status` command reads collection info
- Point ID = `hand.id` (u64), payload stores 20+ metadata fields + `hand_json` (full Hand as JSON)

## Target State

- `src/storage.rs` — `VectorStore` wraps `lancedb::Connection` + `lancedb::Table`, same public API
- `src/search.rs` — Builds SQL-like filter strings instead of Qdrant `Filter` objects
- `src/mcp.rs` — No changes needed (depends on `VectorStore` and `search.rs` abstractions)
- Data lives in `~/.pokervector/data/` (local directory, no Docker)

## Dependencies

```toml
# Remove
qdrant-client = "..."

# Add
lancedb = "0.26"
arrow-array = "55"
arrow-schema = "55"
futures = "0.3"  # already present for tokio
```

Check `arrow` version compatibility with `lancedb 0.26` — they must match (both arrow 55.x).

## Schema Design

Single LanceDB table `poker_hands` with columns:

| Column | Arrow Type | Source |
|--------|-----------|--------|
| `id` | `UInt64` | `hand.id` (primary key for upsert) |
| `hand_json` | `Utf8` | Full `Hand` serialized as JSON |
| `summary_text` | `Utf8` | Natural language summary |
| `action_text` | `Utf8` | Action sequence encoding |
| `site` | `Utf8` | `"ACR"` |
| `game_type` | `Utf8` | `"cash"` / `"tournament"` |
| `variant` | `Utf8` | `"holdem"`, `"omaha"`, etc. |
| `betting_limit` | `Utf8` | `"no_limit"`, `"pot_limit"`, `"fixed_limit"` |
| `is_hi_lo` | `Boolean` | `hand.is_hi_lo` |
| `is_bomb_pot` | `Boolean` | `hand.is_bomb_pot` |
| `stakes` | `Utf8` | `"$0.01/$0.02"` or `"L1 25/50"` |
| `table_size` | `UInt8` | `hand.table_size` |
| `hero` | `Utf8` | Hero name or `""` |
| `hero_position` | `Utf8` | Position string or `""` |
| `hero_cards` | `Utf8` | Space-separated cards |
| `hero_result` | `Utf8` | `"won"`, `"lost"`, `"folded"`, `"sat_out"` |
| `board` | `Utf8` | Space-separated cards |
| `num_players` | `UInt64` | Count of active players |
| `went_to_showdown` | `Boolean` | Derived from actions |
| `timestamp` | `Utf8` | UTC timestamp |
| `pot_type` | `Utf8` | From `classify_pot_type()` |
| `opponent_names` | `Utf8` | Comma-separated (was array in Qdrant) |
| `tournament_id` | `UInt64` | Optional, 0 if not tournament |
| `pot_amount` | `Float64` | `hand.pot.amount` or 0.0 |
| `summary_vector` | `FixedSizeList<Float32, 384>` | Summary embedding |
| `action_vector` | `FixedSizeList<Float32, 384>` | Action embedding |

Notes:
- `opponent_names` changes from Qdrant array to comma-separated string (filter with SQL `LIKE` or switch to a join table later)
- `tournament_id` uses 0 instead of null (simpler Arrow construction)
- `id` column is the merge key for upsert dedup

## Migration Steps

### Step 1: Add LanceDB Dependencies

Update `Cargo.toml` — add `lancedb`, `arrow-array`, `arrow-schema`. Remove `qdrant-client`.

Verify it compiles (will break storage.rs/search.rs — expected).

### Step 2: Rewrite `src/storage.rs`

Replace the Qdrant implementation with LanceDB. Keep the same public API surface:

```rust
pub struct VectorStore {
    db: lancedb::Connection,
    table_name: String,
}

pub struct SearchResult { /* unchanged */ }
pub struct HandEmbeddings { /* unchanged */ }
```

**Method mapping:**

| Current (Qdrant) | New (LanceDB) |
|-------------------|---------------|
| `new(url, collection)` | `new(data_dir, table_name)` — `lancedb::connect(data_dir).execute().await` |
| `ensure_collection()` | `ensure_table()` — `db.create_empty_table(name, schema)` if not exists |
| `upsert_hand()` | Build `RecordBatch`, `table.merge_insert(&["id"]).when_matched_update_all(None).when_not_matched_insert_all()` |
| `upsert_hands_batch()` | Same as above but with batch `RecordBatch` |
| `search(vector_name, query, limit, filter)` | `table.vector_search(&query).column(vector_name).limit(limit).only_if(filter_str)` |
| `get_hand_vector(hand_id, vector_name)` | `table.query().only_if("id = {hand_id}").select([vector_name]).execute()` |
| `hand_exists(hand_id)` | `table.query().only_if("id = {hand_id}").limit(1).execute()`, check if empty |
| `count()` | `table.count_rows(None).await` |
| `get_hand(hand_id)` | `table.query().only_if("id = {hand_id}").limit(1).execute()`, deserialize `hand_json` |
| `scroll_hands(filter)` | `table.query().only_if(filter_str).execute()`, collect all results |

Key implementation details:
- `connect()` takes a path string, not a URL — use `~/.pokervector/data/`
- Table open: `db.open_table(name).execute().await` (separate from create)
- `ensure_table()` should try `open_table` first, fall back to `create_empty_table`
- Results come back as `Vec<RecordBatch>` — need helper to extract columns by name
- `hand_json` column → `serde_json::from_str::<Hand>()` to reconstruct `Hand` structs
- Arrow `RecordBatch` construction: build parallel arrays (`StringArray`, `UInt64Array`, `FixedSizeListArray`, etc.)

### Step 3: Rewrite `src/search.rs` Filter Building

Replace Qdrant `Filter`/`Condition` types with SQL-like filter strings.

```rust
// Before (Qdrant):
conditions.push(Condition::matches("hero_position", value.clone()));
Filter::must(conditions)

// After (LanceDB):
conditions.push(format!("hero_position = '{}'", value));
Some(conditions.join(" AND "))
```

**Function changes:**

| Function | Change |
|----------|--------|
| `build_filter()` | Returns `Option<String>` instead of `Option<Filter>`. Builds SQL WHERE fragments. |
| `search_hands()` | Passes filter string to `store.search()`. `SearchMode` maps to column name: `Semantic` → `"summary_vector"`, `Action` → `"action_vector"`. |
| `search_similar_actions()` | `must_not(has_id)` becomes `"id != {hand_id}"` appended to filter. |

SQL filter escaping: use parameterized-style quoting or validate inputs (MCP params are user-provided strings, need to escape single quotes).

### Step 4: Update `src/main.rs`

- `import` command: change `VectorStore::new(url, collection)` to `VectorStore::new(data_dir, table_name)`
- `status` command: adapt collection info display (row count, table existence)
- Remove Qdrant URL from `Config` / connection logic (data dir is derived from config path)
- `mcp` command: same `VectorStore::new()` change

### Step 5: Update `src/config.rs`

The `[qdrant]` section in config becomes optional or replaced:

```toml
# Before
[qdrant]
url = "http://localhost:6334"
collection = "poker_hands"

# After
[storage]
data_dir = "~/.pokervector/data"  # optional, has default
```

Or just hardcode the data dir to `~/.pokervector/data/` and remove the config section entirely. Less to configure = fewer support issues.

### Step 6: Create Vector Indexes

After the table has enough data, create indexes for fast search:

```rust
// In ensure_table() or after first import
table.create_index(&["summary_vector"], Index::Auto).execute().await?;
table.create_index(&["action_vector"], Index::Auto).execute().await?;
```

LanceDB auto-selects IVF-PQ for vector columns. Index creation can happen lazily (small datasets don't need it — brute force is fast enough under ~50k rows).

### Step 7: Update Tests

- Integration tests in `tests/` that touch storage need updating
- Unit tests in `storage.rs` / `search.rs` — use temp directories for LanceDB data
- No Qdrant Docker container needed for CI anymore (big win)

### Step 8: Update Documentation

- `CLAUDE.md` — update architecture section, remove Qdrant references, add LanceDB details
- `README.md` — remove Docker/Qdrant setup instructions
- Remove `qdrant` config from example `config.toml`

## Files Changed

| File | Change Type |
|------|-------------|
| `Cargo.toml` | Remove `qdrant-client`, add `lancedb` + `arrow-*` |
| `src/storage.rs` | Full rewrite (same public API) |
| `src/search.rs` | Rewrite filter building (Qdrant Filter → SQL strings) |
| `src/main.rs` | Update VectorStore construction, remove Qdrant URL plumbing |
| `src/config.rs` | Replace `[qdrant]` config section |
| `src/mcp.rs` | Minimal — update VectorStore construction if signature changed |
| `src/lib.rs` | No change (re-exports stay the same) |
| `tests/` | Update integration tests for LanceDB |
| `CLAUDE.md` | Update docs |

## Risks & Considerations

1. **Arrow version conflicts** — `lancedb` pins specific `arrow-*` versions. Check that `ort` (ONNX Runtime) doesn't also depend on conflicting Arrow versions. If so, isolate with feature flags or version pinning.

2. **Upsert performance** — LanceDB `merge_insert` may be slower than Qdrant upsert for large batches. Profile and consider plain `add()` (append) + periodic dedup if needed.

3. **No cursor-based pagination** — LanceDB uses `offset`/`limit` instead of Qdrant's `next_page_offset` cursor. For `scroll_hands()` (which loads all matching hands), just collect the full stream — same as current behavior but without pagination.

4. **Filter SQL injection** — MCP tool params are user strings that go into `only_if()` SQL predicates. Must escape/validate to prevent injection. Use a whitelist of allowed column names and sanitize values.

5. **opponent_names filtering** — Currently an array in Qdrant with `Condition::matches` for "contains" semantics. In LanceDB with a comma-separated string, need `opponent_names LIKE '%VillainName%'`. Less precise but works for the use case.

6. **Data directory permissions** — `~/.pokervector/data/` must be writable. Handle permission errors gracefully on first run.

7. **Migration path for existing users** — Anyone who has data in Qdrant will need to re-import. Since it's pre-product (dev only), this is acceptable. Document in release notes.
