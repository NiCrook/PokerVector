# Action-Sequence Embeddings

**Category:** Advanced Embedding / Search
**Summary:** Embed the betting line itself (not just the natural language summary) for finding structurally similar hands.

## Current State

- `summarizer.rs` generates natural language summaries that are embedded via all-MiniLM-L6-v2
- Summaries include action descriptions but are narrative text, not structured action encodings
- Semantic search matches based on language similarity — "hero raises and gets 3-bet" matches other descriptions containing similar words
- The actual structure of the action sequence (bet sizes relative to pot, number of players, street-by-street action) is lost in the text embedding
- `Hand.actions: Vec<Action>` contains the full structured action sequence

## Problem

Two hands can have very similar betting structures but different natural language summaries (different player names, card values, stakes). Current embeddings miss structural similarity:
- "Hero opens to 3BB, gets 3-bet to 9BB, 4-bets to 22BB" is structurally identical regardless of stakes, cards, or player names
- "Hero c-bets 2/3 pot, gets check-raised, shoves" is a pattern that should match other c-bet/check-raise/shove lines

## Build Plan

### Step 1: Action Sequence Encoding

**File:** `src/action_encoder.rs` (new module)

Convert a hand's actions into a normalized, position-relative sequence string:

```rust
pub fn encode_action_sequence(hand: &Hand, player: &str) -> String
```

Output format — one line per street, actions normalized to BB and pot-relative sizing:

```
PRE: HERO_OPEN(3bb) V1_3BET(9bb) HERO_4BET(22bb) V1_CALL
FLOP[Ah7c2d]: HERO_CBET(0.67pot) V1_CALL
TURN[5s]: HERO_BET(0.75pot) V1_RAISE(2.5x) HERO_CALL
RIVER[Kd]: CHECK CHECK
RESULT: HERO_WIN(45bb)
```

Normalization rules:
- All bet sizes expressed as BB multiples (preflop) or pot fractions (postflop)
- Players anonymized: `HERO`, `V1`, `V2`, etc. (by position order)
- Board cards included (they affect action meaning)
- Result included (win/loss amount in BB)

### Step 2: Dual Embedding Strategy

**File:** `src/embedder.rs` (extend) and `src/storage.rs` (extend)

Two approaches to consider:

**Option A: Separate vector field in Qdrant**
- Store a second embedding per hand: `action_embedding` alongside the existing `summary_embedding`
- Embed the action sequence string using the same all-MiniLM-L6-v2 model
- Search either vector field depending on query type
- Qdrant supports named vectors per point

**Option B: Concatenated embedding input**
- Append the action sequence to the natural language summary before embedding
- Single vector captures both narrative and structural information
- Simpler storage but less flexible search

**Recommendation: Option A** — named vectors allow searching by narrative OR by action structure independently.

### Step 3: Qdrant Named Vectors

**File:** `src/storage.rs`

Migrate collection schema to support named vectors:
```rust
// Collection config with named vectors
VectorsConfig::Multi(HashMap::from([
    ("summary".to_string(), VectorParams::new(384, Distance::Cosine)),
    ("action".to_string(), VectorParams::new(384, Distance::Cosine)),
]))
```

Update `build_point()` to include both vectors. Update `search()` to accept a `vector_name` parameter.

**Migration path:** Existing collections have a single unnamed vector. Need a migration strategy:
1. Create new collection with named vectors
2. Re-import all hands with both embeddings
3. Or: add action embeddings as a separate collection and join results

### Step 4: Action-Based Search

**File:** `src/search.rs` (extend)

Add `search_by_action(store, embedder, action_query, filters, limit)`:
- Takes a natural language description of an action line (e.g., "hero 3-bets and faces a 4-bet shove")
- Embeds the query
- Searches the `action` vector field
- Returns structurally similar hands regardless of stakes/cards

Alternatively, allow searching by providing a hand ID — "find hands with a similar action sequence to hand #12345":
```rust
pub fn search_similar_actions(store, hand_id, limit) -> Vec<HandSearchResult>
```

### Step 5: Expose via MCP

**File:** `src/mcp.rs`

Extend `search_hands` with a `search_mode` parameter:
```rust
pub enum SearchMode {
    Semantic,   // current behavior — search summary vector
    Action,     // search action vector
    Both,       // search both and merge results
}
```

Or add a new tool `search_similar_hands`:
```rust
pub struct SearchSimilarParams {
    pub hand_id: u64,               // find hands similar to this one
    pub mode: Option<String>,       // "action", "semantic", "both"
    pub limit: Option<u64>,
}
```

## Dependencies

- Existing `embedder.rs` — same model, just embedding different text
- Existing `storage.rs` — needs named vector support
- `Hand.actions` — already fully populated by parser
- Qdrant named vectors feature (supported in current version)

## Migration Considerations

- Existing single-vector collections need migration to named vectors
- All existing hands need re-embedding with the action encoder
- This is a breaking change to the storage schema — consider versioning the collection name
- Import process needs to generate both embeddings per hand

## Testing

- Unit tests for `encode_action_sequence` with various hand types (SRP, 3-bet pot, multi-way, etc.)
- Verify normalization: same action at $0.01/$0.02 and $1/$2 should produce identical encodings
- Test named vector search returns structurally similar hands
- Compare action search results vs. semantic search results — should differ meaningfully
- Integration test: embed a hand, search for it by action, verify it appears in top results
