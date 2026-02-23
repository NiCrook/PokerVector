# Phase 4: Tests & Documentation

**Goal:** Remove `#[ignore]` from storage tests, update all documentation, and eliminate every remaining Qdrant reference.

**Revert:** `git revert` this commit restores the old test/doc state.

**Depends on:** Phases 1-3 (everything compiles and works end-to-end)

## Storage Tests

Remove all `#[ignore]` annotations from storage tests. They no longer need Docker — just `tempfile::tempdir()`.

Ensure these tests exist and pass (created in Phase 2):
- `test_ensure_table_creates_new`
- `test_upsert_and_get_round_trip`
- `test_hand_exists`
- `test_scroll_with_filter`

## Integration Tests

### `tests/parser_tests.rs`
No changes — parser tests don't touch storage.

### Consider adding: `tests/storage_tests.rs`
End-to-end integration test: parse a fixture hand → summarize → embed → upsert → search → verify. Uses temp dir. Optional — unit tests in `storage.rs` may be sufficient.

## Documentation Updates

### `CLAUDE.md`

**Build & Test Commands:**
- Remove `docker run` Qdrant command
- Remove "(needs Qdrant)" annotations from import/status/mcp commands
- Add: data is stored in `~/.pokervector/data/` (no external services needed)

**Architecture section updates:**
- `src/storage.rs` — "LanceDB wrapper. Single table `poker_hands` with named vector columns `summary` + `action` (384-dim). Data at `~/.pokervector/data/`."
- `src/search.rs` — "Search with SQL WHERE filters. `build_filter()` returns `Option<String>`."
- Full pipeline: replace "Qdrant" with "LanceDB" in the pipeline description

**Config System:**
- Remove `[qdrant]` section from example config
- Remove `url` and `collection` fields
- Add: "Data stored in `~/.pokervector/data/` (LanceDB embedded database, no configuration needed)"

**Remove from Windows Build Notes:**
- Any Qdrant-specific notes

**Add to Windows Build Notes (if applicable):**
- Any Arrow/LanceDB build issues discovered during Phase 1

### `Cargo.toml` comments
Remove any Qdrant-related comments if present.

## Grep Sweep

Run and resolve all hits:
```bash
grep -ri "qdrant" src/ tests/ CLAUDE.md Cargo.toml
```

Expected: zero results after this phase.

Also check for stale references:
```bash
grep -ri "docker" CLAUDE.md
grep -ri "6333\|6334" src/ CLAUDE.md
```

## Memory Update

Update `MEMORY.md` to reflect:
- LanceDB instead of Qdrant
- No Docker dependency
- `config::data_dir()` helper
- SQL filter strings instead of Qdrant Filter objects
- `ensure_table()` instead of `ensure_collection()`

## Success Criteria

- `cargo test` passes all tests with zero `#[ignore]` skips for storage
- `grep -ri "qdrant" src/` returns zero results
- `grep -ri "qdrant" CLAUDE.md` returns zero results
- `CLAUDE.md` accurately describes the LanceDB architecture
- No Docker dependency mentioned in setup instructions
