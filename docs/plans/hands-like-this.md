# "Hands Like This" Search

**Category:** Advanced Embedding / Search
**Summary:** Given a hand, find the most similar hands you've played before.

## Current State

- `search_hands` takes a text query, embeds it, and does ANN search in Qdrant
- `get_hand` retrieves a single hand by ID
- Each hand has a stored embedding (384-dim) in Qdrant
- Each hand has a stored `summary` in the payload
- No way to search by reference hand — must formulate a text query

## Problem

When reviewing a hand, the natural question is "when have I been in this spot before?" This requires:
1. Finding the hand's existing embedding
2. Using it as the query vector
3. Optionally filtering by specific aspects (same position, same pot type, etc.)

Currently, the user would have to describe the hand in words and search — losing precision.

## Build Plan

### Step 1: Search by Hand ID

**File:** `src/search.rs` (extend)

```rust
pub struct SimilarHandsParams {
    pub hand_id: u64,
    pub limit: Option<u64>,
    pub position: Option<String>,
    pub pot_type: Option<String>,
    pub variant: Option<String>,
    pub result: Option<String>,
    pub exclude_self: bool,         // default true
}

pub async fn search_similar_hands(
    store: &VectorStore,
    params: SimilarHandsParams,
) -> Result<Vec<HandSearchResult>>
```

Implementation:
1. Retrieve the reference hand's point from Qdrant by ID (using `get_points` with `with_vectors: true`)
2. Extract its embedding vector
3. Use that vector as the search query with `store.search(vector, limit, filter)`
4. Exclude the reference hand itself from results (filter by point ID != hand_id)
5. Apply any additional filters

This requires extending `VectorStore` to support retrieving vectors:
```rust
pub async fn get_hand_vector(&self, hand_id: u64) -> Result<Vec<f32>>
```

### Step 2: Similarity Breakdown

**File:** `src/search.rs`

When returning similar hands, include context about *why* they're similar:

```rust
pub struct SimilarHandResult {
    pub hand: HandSearchResult,
    pub similarity_score: f32,          // cosine similarity (0–1)
    pub shared_features: Vec<String>,   // what's in common
}
```

`shared_features` compares metadata between the reference hand and each result:
- Same position → "Same position (BTN)"
- Same pot type → "Same pot type (3-bet)"
- Same result → "Same result (won)"
- Similar pot size → "Similar pot size (~25 BB)"
- Same street reached → "Both went to river"
- Same number of players → "Both 3-way"

### Step 3: Multi-Aspect Similarity

**File:** `src/search.rs`

Allow users to specify *which aspects* of similarity matter most:

```rust
pub enum SimilarityFocus {
    Overall,        // use stored embedding as-is
    Action,         // use action sequence embedding (from action-sequence-embeddings plan)
    Situation,      // filter to same position + pot type, then semantic search
}
```

- `Overall` — default, uses the existing summary embedding
- `Action` — uses the action sequence embedding (requires action-sequence-embeddings plan)
- `Situation` — applies tight filters first (same position, pot type, variant), then searches within that subset

### Step 4: Expose via MCP

**File:** `src/mcp.rs`

New tool `find_similar_hands`:
```rust
pub struct FindSimilarHandsParams {
    pub hand_id: u64,
    pub limit: Option<u64>,         // default 10
    pub focus: Option<String>,      // "overall", "action", "situation"
    pub position: Option<String>,
    pub pot_type: Option<String>,
    pub variant: Option<String>,
    pub hero: Option<String>,
}
```

Returns `Vec<SimilarHandResult>` ordered by similarity score.

Usage example via MCP client:
1. User asks "how did I play this hand?" → LLM calls `get_hand(12345)`
2. User asks "when have I been in a similar spot?" → LLM calls `find_similar_hands(hand_id=12345)`
3. LLM compares the hands and provides analysis

### Step 5: "Hands Like This" from Search Results

After any `search_hands` call, the MCP client can take a result and call `find_similar_hands` to drill deeper. This creates a natural exploration flow:
1. Broad semantic search → find interesting hand
2. "Hands like this" → find structural matches
3. Review cluster of similar hands → identify patterns

No additional implementation needed — this is a usage pattern enabled by the tool.

## Dependencies

- Existing `VectorStore` — needs `get_hand_vector()` method
- Existing `search_hands` — similar result format
- Action-sequence-embeddings plan (optional, for `Action` focus mode)
- Qdrant `get_points` with `with_vectors` parameter

## Testing

- Unit test: search similar to a known hand, verify the hand itself is excluded
- Unit test: verify `shared_features` correctly identifies common metadata
- Test with filters: similar hands in the same position should all share that position
- Test similarity scores are in [0, 1] range and sorted descending
- Integration test: pick a hand from `PolarFox/` data, find similar hands, verify results are reasonable
- Edge case: hand ID that doesn't exist → clear error message
