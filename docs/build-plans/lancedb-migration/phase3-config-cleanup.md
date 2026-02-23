# Phase 3: Config Cleanup

**Goal:** Remove the now-unused `QdrantConfig` from `config.rs` and update config tests. This is pure cleanup — the code already works after Phase 2.

**Revert:** `git revert` this commit restores the `QdrantConfig` struct (harmless dead code).

**Depends on:** Phase 2 (storage, search, and CLI all work with LanceDB)

## Changes to `src/config.rs`

### Remove `QdrantConfig`

Delete entirely:
- `pub struct QdrantConfig`
- `fn default_qdrant_url()`
- `fn default_collection()`
- `impl Default for QdrantConfig`

### Update `Config` struct

```rust
// Before:
pub struct Config {
    pub accounts: Vec<Account>,
    pub qdrant: QdrantConfig,
}

// After:
pub struct Config {
    pub accounts: Vec<Account>,
}
```

### Backward compatibility

Existing `config.toml` files with a `[qdrant]` section must still parse. serde/toml ignores unknown keys by default (no `deny_unknown_fields` is set on `Config`), so removing the `qdrant` field won't break deserialization of old config files.

### Update tests

- `test_config_round_trip` — remove `qdrant` field from test config, remove `QdrantConfig` assertions
- `test_load_missing_file` — remove `qdrant` defaults check
- `test_save_and_load` — remove `QdrantConfig` from test data
- `test_defaults_when_missing` — remove `qdrant` assertions

## Success Criteria

- `cargo build` succeeds
- All config tests pass
- Old `config.toml` files with `[qdrant]` section still parse without error
- No remaining references to `QdrantConfig` in `src/`
