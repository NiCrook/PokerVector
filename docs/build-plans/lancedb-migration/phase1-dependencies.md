# Phase 1: Dependencies

**Goal:** Swap Qdrant for LanceDB in `Cargo.toml` and get the project compiling on Windows with stubs. This is the risk gate — if Arrow conflicts with `ort`, we find out here before touching real code.

**Revert:** `git revert` this commit restores the original `Cargo.toml` and stub files.

## Changes

### `Cargo.toml`

```toml
# Remove
qdrant-client = "1.13"

# Add (check crates.io for latest compatible versions — arrow-* must match what lancedb requires)
lancedb = "<latest>"
arrow-array = "<matching>"
arrow-schema = "<matching>"
futures = "0.3"  # needed for TryStreamExt::try_collect() on LanceDB query streams
```

Run `cargo tree -i arrow-schema` after adding deps to check for conflicts with `ort`. May also need additional `arrow-*` subcrates (e.g. `arrow-cast`) depending on what conversions are needed — discover during implementation.

## Steps

1. Remove `qdrant-client` from `[dependencies]`
2. Add `lancedb`, `arrow-array`, `arrow-schema`, `futures`
3. Stub out `storage.rs` and `search.rs` so the project compiles:
   - `storage.rs`: keep all public types and method signatures, replace bodies with `todo!()`
   - `search.rs`: keep all public types and method signatures, replace Qdrant filter types with `String` equivalents, replace bodies with `todo!()`
   - **Also stub `main.rs` call sites** — both files' signatures change together (filter types, `new()` params, `ensure_collection()` → `ensure_table()`), so `main.rs` must be updated to match the new signatures with temporary hardcoded values
   - `mcp.rs` will compile without changes because it only depends on the public API of storage/search via pass-through types (`None` works for both `Option<Filter>` and `Option<String>`)
   - **Mark storage/search tests `#[ignore]`** — stub bodies use `todo!()` which panics at runtime, so existing unit tests in `storage.rs` and `search.rs` must be `#[ignore]`'d until Phase 2 replaces the stubs
4. `cargo build` — verify no version conflicts between `arrow-*` and `ort`
5. If conflicts: pin arrow versions to match what `lancedb` requires, check `cargo tree -d` for duplicates

## Risk Checks

- **Arrow version conflicts:** `cargo tree -i arrow-schema` — does `ort` pull in a conflicting version?
- **CRT conflicts on Windows:** `cargo build` with MSVC — any `/MD` vs `/MT` link errors?
- **`Send + Sync`:** Verify `lancedb::Connection` is `Send + Sync` (required because `mcp.rs` wraps `VectorStore` in `Arc`). Write a trivial compile test:
  ```rust
  fn assert_send_sync<T: Send + Sync>() {}
  assert_send_sync::<lancedb::Connection>();
  ```

## Success Criteria

- `cargo build` succeeds (with `todo!()` stubs)
- No CRT conflicts on Windows
- `cargo test` passes (storage/search tests are `#[ignore]`'d; parser, summarizer, encoder tests all pass)
- `lancedb::Connection` is confirmed `Send + Sync`
