# Build Plan: Action-Sequence Embeddings

**Source:** `docs/plans/action-sequence-embeddings.md`
**Goal:** Embed structured betting action sequences alongside narrative summaries so users can find structurally similar hands regardless of stakes, cards, or player names.

---

## Step 0: Switch Embedding Model to BGE-small-en-v1.5

**File:** `src/embedder.rs`

### Why

all-MiniLM-L6-v2 has a 256-token max sequence length. Action encodings for multi-street hands with heavy action can approach or exceed this limit, causing silent truncation of late streets and the result line. BGE-small-en-v1.5 doubles the limit to 512 tokens while keeping the same 384 dimensions.

### Model comparison

| | all-MiniLM-L6-v2 (current) | BGE-small-en-v1.5 (new) |
|---|---|---|
| Max tokens | 256 | 512 |
| Dimensions | 384 | 384 |
| Params | 22.7M | 33.4M |
| ONNX size | ~90 MB | ~133 MB |
| Tokenizer | WordPiece | WordPiece |
| Prefix required | No | No (optional query prefix, not needed) |

Same dims means the Qdrant vector config stays `384, Cosine` — no additional schema change beyond the named vectors migration already planned in Step 2.

### Changes

Update the model repo in `Embedder::new()`:

```rust
// Before:
let repo = api.model("sentence-transformers/all-MiniLM-L6-v2".to_string());
// After:
let repo = api.model("Xenova/bge-small-en-v1.5".to_string());
```

Use the **Xenova** export (not the BAAI repo) — it has confirmed ONNX files at `onnx/model.onnx` and `tokenizer.json`, matching the same paths used by all-MiniLM-L6-v2. The BAAI repo may only have PyTorch weights. The `tokenizer.json` is compatible with the `tokenizers` crate — same WordPiece format, no code changes needed beyond the repo name.

### Migration impact

This is a **breaking change** — embeddings from different models are incompatible. All existing hands must be re-embedded. Since the named vectors migration (Step 2) already requires a full re-import, this adds zero additional migration cost. Both changes land together.

### Testing

Update `src/embedder.rs` tests:
- `test_embed_produces_384_dim_vector` — should still pass (same dims)
- `test_embed_batch` — should still pass
- `test_similar_texts_have_higher_cosine_similarity` — should still pass (BGE scores higher on retrieval benchmarks)

All tests are `#[ignore]` (require model download), so they won't break CI. Run manually after the switch to verify.

### Update CLAUDE.md and MEMORY.md

After switching, update references from `all-MiniLM-L6-v2` to `BGE-small-en-v1.5` in:
- `CLAUDE.md` — embedder module description, Windows build notes
- Memory files — M2 Architecture section

---

## Step 1: Action Encoder Module

**New file:** `src/action_encoder.rs`
**Add `mod action_encoder` to:** `src/main.rs` and `src/lib.rs`

### 1a. Helper: Extract big blind amount

```rust
fn big_blind_amount(hand: &Hand) -> f64
```

Pull the BB from `hand.game_type` (Cash or Tournament). Returns `amount` as `f64`. Used to normalize all sizes to BB multiples.

### 1b. Helper: Pot and investment tracker

```rust
struct PotTracker {
    /// Total pot entering the current street (sum of all prior streets' contributions)
    pot_at_street_start: f64,
    /// Running pot within the current street (pot_at_street_start + current street bets)
    current_pot: f64,
    /// Hero's total investment across all streets (for net result calc)
    hero_invested: f64,
    /// Per-player investment in the current betting round (for raise sizing)
    current_round: HashMap<String, f64>,
    /// The current bet/raise amount to call on this street (for identifying raise targets)
    current_bet: f64,
}
```

**Initialization:**
- Seed `current_pot` with blinds + antes from `PostSmallBlind`, `PostBigBlind`, `PostAnte`, `PostBlind`, `BringsIn` actions
- Track each in `hero_invested` (if hero) and `current_round` — **except antes** (see below)

**Per-action updates:**
- `Call { amount }` — `amount` is the increment (additional chips put in). Add to player's `current_round` and `current_pot`. If hero, add to `hero_invested`
- `Bet { amount }` — `amount` is the total bet (player had 0 in the round). Add to `current_round[player]` and `current_pot`. Set `current_bet = amount`. If hero, add to `hero_invested`
- `Raise { amount, to }` — `amount` is the increment the player adds, `to` is the new total bet. Both are equivalent: `amount == to - current_round[player]`. Update `current_round[player] = to`, add `amount` to `current_pot`. Set `current_bet = to`. If hero, add `amount` to `hero_invested`
- `UncalledBet { amount }` — subtract from `current_pot`. If hero, subtract from `hero_invested`. This action is skipped in the encoding output but must be tracked because it affects pot size
- `PostSmallBlind/PostBigBlind/PostBlind` — add to `current_round[player]`, `current_pot`, and `hero_invested` (if hero). For `PostBigBlind`, also set `current_bet = amount` (the BB is the initial bet to call preflop)
- `PostAnte` — add to `current_pot` and `hero_invested` (if hero), but **NOT** to `current_round`. ACR's `Raise.amount` and `Call.amount` are computed excluding antes — verified against `cash_ante.txt` where `amount` = `to - current_round[player]` only holds when antes are excluded from `current_round`
- `BringsIn { amount }` — add to `current_round[player]`, `current_pot`, and `hero_invested` (if hero). Also set `current_bet = amount` — subsequent stud raises need this to compute the raise multiplier

**Street transitions:**
- When street changes: `pot_at_street_start = current_pot`, clear `current_round`, reset `current_bet = 0`

**Why this matters:** Postflop sizing is expressed as pot fractions. Without accurate pot tracking, `(0.67pot)` is wrong. And the hero net result requires knowing hero's total investment.

### 1c. Helper: Action classifier

```rust
enum ActionLabel {
    Open(f64),        // first voluntary raise preflop, size in bb
    Bet(f64),         // first bet on a postflop street, size as pot fraction
    Raise(f64),       // raise over a bet, size as multiplier of current bet
    ThreeBet(f64),    // re-raise preflop (2nd raise), size in bb
    FourBet(f64),     // 3rd raise preflop, size in bb
    FiveBetPlus(f64), // 4th+ raise preflop, size in bb
    CBet(f64),        // continuation bet (preflop aggressor bets first on flop/turn/river)
    Call(f64),        // call with size — bb (preflop) or pot fraction (postflop)
    Check,
    Fold,
    BringIn(f64),
}

struct ClassifiedAction {
    label: ActionLabel,
    all_in: bool,     // modifier flag, sourced from ActionType's all_in field
}
```

The `all_in` flag comes directly from the `ActionType` variants that carry it (`Call`, `Bet`, `Raise`, `PostSmallBlind`, `PostBigBlind`). When formatting, append `_AI` to the label if `all_in` is true.

**Classification logic:**

Track per-street state:
- `raise_count: u32` — number of voluntary raises on the current street. Blinds don't count. Resets to 0 each new street. Preflop: first raise = Open (1), second = 3-Bet (2), third = 4-Bet (3), fourth+ = 5-Bet+ (4+)
- `preflop_aggressor: Option<String>` — the last player to raise preflop. Used for c-bet detection on all postflop streets

**C-bet detection:** On a postflop street, if the `preflop_aggressor` makes the first bet (raise_count was 0 on this street), label it `CBet` instead of `Bet`. This is a simplification — a true c-bet also requires the bettor to have been the last raiser pre, which `preflop_aggressor` already tracks.

**Preflop raise ladder:**
1. `raise_count == 0` and action is `Raise` → `Open`, `raise_count = 1`
2. `raise_count == 1` and action is `Raise` → `ThreeBet`, `raise_count = 2`
3. `raise_count == 2` → `FourBet`, etc.

Note: "3-bet" in poker means the 3rd bet (blind is 1st, open is 2nd, 3-bet is 3rd). Our `raise_count` tracks raises specifically, so raise 1 = open, raise 2 = 3-bet. This matches standard poker terminology.

**Postflop raises:** Express as a multiplier of the bet being raised. The "bet being raised" is `current_bet` from the PotTracker. So `Raise { to }` on a postflop street where `current_bet > 0`: multiplier = `to / current_bet`.

**Stud raises:** Stud streets (3rd through 7th) are neither preflop nor postflop. Use the **multiplier convention** (`to / current_bet`), same as postflop. The bring-in sets `current_bet`, and subsequent completions/raises express as multipliers of that. In Fixed Limit stud, this typically produces uniform ~2x multipliers, which correctly captures the fixed-limit structure.

### 1d. Helper: Anonymize players

```rust
fn build_alias_map(hand: &Hand, hero: &str) -> HashMap<String, String>
```

- Hero → `"HERO"`
- Other active players → `"V1"`, `"V2"`, ... ordered by seat number (stable ordering)
- Only include players who are not sitting out

### 1e. Helper: Format board cards

```rust
fn format_board(board: &[Card], street: Street) -> String
```

Return the board cards visible at a given street:
- Flop: first 3 cards → `[Ah7c2d]`
- Turn: 4th card only → `[5s]` (flop already shown on its line)
- River: 5th card only → `[Kd]`

### 1f. Main encoder function

```rust
pub fn encode_action_sequence(hand: &Hand, hero: &str) -> String
```

Produces a multi-line string, one line per street:

```
PRE: HERO_OPEN(3bb) V1_3BET(9bb) HERO_4BET(22bb) V1_CALL(13bb)
FLOP[Ah7c2d]: HERO_CBET(0.67pot) V1_CALL(0.67pot)
TURN[5s]: HERO_CBET(0.75pot) V1_RAISE(2.5x) HERO_CALL(1.13pot)
RIVER[Kd]: CHECK CHECK
RESULT: HERO(+22.5bb)
```

**Action formatting per type:**
- `ActionLabel::Open(bb)` → `ALIAS_OPEN(Xbb)`
- `ActionLabel::ThreeBet(bb)` → `ALIAS_3BET(Xbb)`
- `ActionLabel::FourBet(bb)` → `ALIAS_4BET(Xbb)`
- `ActionLabel::FiveBetPlus(bb)` → `ALIAS_5BET(Xbb)`
- `ActionLabel::Bet(pot_frac)` → `ALIAS_BET(X.XXpot)`
- `ActionLabel::CBet(pot_frac)` → `ALIAS_CBET(X.XXpot)`
- `ActionLabel::Raise(multiplier)` → `ALIAS_RAISE(X.Xx)` (postflop only; preflop raises use the ladder labels above)
- `ActionLabel::Call(size)` → `ALIAS_CALL(Xbb)` preflop or `ALIAS_CALL(X.XXpot)` postflop. Size uses the **incremental cost** (ACR's `Call.amount` = additional chips the caller puts in): preflop `Call.amount / BB`, postflop `Call.amount / pot_at_street_start`. All postflop pot fractions (bets, calls, c-bets) reference `pot_at_street_start` — the pot entering the current street, before any actions on it. For a flat call of a bet with no prior investment, the call shows the same pot fraction as the bet (e.g., 0.67pot bet → 0.67pot call). For a call after a raise, the call shows the caller's incremental cost (e.g., bet 0.75pot, raise 2.5x, call 1.13pot)
- `ActionLabel::Check` → `CHECK` (no alias needed — who checks is less important than who bets)
- `ActionLabel::Fold` → `ALIAS_FOLD`
- `ActionLabel::BringIn(bb)` → `ALIAS_BRINGIN(X.Xbb)`
- All-in modifier (`ClassifiedAction.all_in == true`): append `_AI` before the size — `HERO_OPEN_AI(15bb)`, `V1_CALL_AI(22bb)`

**Number formatting precision:**
- BB amounts (preflop sizes, bring-ins): 1 decimal place, drop trailing `.0` — `3bb`, `2.5bb`, `22bb`
- Pot fractions (postflop bets, calls, c-bets): 2 decimal places, drop trailing `0`s — `0.67pot`, `0.5pot`, `1.13pot`
- Raise multipliers (postflop raises): 1 decimal place, drop trailing `.0` — `2.5x`, `3x`
- Use `format!("{:.1}", val).trim_end_matches('0').trim_end_matches('.')` pattern for clean output

**Actions to skip (tracked by PotTracker but not encoded):**
- `PostSmallBlind`, `PostBigBlind`, `PostAnte`, `PostBlind` — tracked for pot/investment, not encoded
- `UncalledBet` — tracked for pot adjustment, not encoded
- `Shows`, `DoesNotShow`, `Mucks`, `SitsOut`, `WaitsForBigBlind`, `Collected` — not encoded

**Result line (hero only):**
```
RESULT: HERO(+22.5bb)
```

Net BB = (amount collected from `hand.result.winners` for hero) - `hero_invested`, divided by BB.
- `HeroResult::Won` — look up hero in `winners`, sum all entries (may have multiple for side pots), subtract `hero_invested`, divide by BB
- `HeroResult::Folded` / `HeroResult::Lost` — net = `-hero_invested / bb`
- `HeroResult::SatOut` — no result line (hand is encoded as `SAT_OUT`)

Only hero's result is included. Villain results add tokens without improving structural similarity — the action sequence already captures the full betting pattern, and hero's net outcome is sufficient context.

For H/L hands (`hand.is_hi_lo == true`): hero may win HI, LO, or both. However, ACR's collected lines say "from main pot" for both halves — the `Winner.pot` field does NOT carry HI/LO info (verified against `omaha_hl.txt`). So H/L hands use the same result format as non-H/L: `RESULT: HERO(+Xbb)` with the total net amount. The action sequence captures the structural pattern; which half hero won is not meaningful for action similarity.

**Stud handling:**
- Streets are `ThirdStreet` through `SeventhStreet`
- No board cards — omit the `[cards]` bracket
- Use `3RD:`, `4TH:`, `5TH:`, `6TH:`, `7TH:` as street labels
- `BringsIn` action is encoded (it's the stud equivalent of a forced bet)

**Edge cases:**
- Bomb pots: prefix with `BOMB_POT\n` on the first line, skip preflop (there are no preflop actions, play starts on flop)
- Walks (everyone folds to BB preflop): `PRE: WALK\nRESULT: HERO(+Xbb)` — detect by checking if **Hero is the BB**, Hero won, and the only preflop actions (besides blind/ante posts) are folds (no Call, Bet, or Raise actions). If a walk occurs but Hero is NOT the BB, encode normally (Hero folded)
- Multi-way pots: all active players' actions included in seat order per street
- Hands where hero sat out: `SAT_OUT` — single line, no action encoding

### Tests for Step 1

Unit tests in `src/action_encoder.rs` (using hand-crafted `Hand` structs like `summarizer.rs` does):

- `test_encode_simple_srp` — open, call, c-bet, fold. Verify: preflop uses BB sizing, flop uses pot fractions, c-bet detected for preflop raiser
- `test_encode_3bet_pot` — open, 3-bet, call. Verify: raise_count labels correct (OPEN, 3BET)
- `test_encode_4bet_pot` — open, 3-bet, 4-bet, call. Verify the full ladder
- `test_encode_multiway` — 3+ players, verify all aliases present in seat order
- `test_encode_stakes_normalization` — construct two hands with identical action structure but $0.01/$0.02 vs $1/$2 stakes. Verify identical output
- `test_encode_bomb_pot` — verify `BOMB_POT` prefix, no preflop line
- `test_encode_stud` — verify stud street labels (`3RD:` etc.), no board cards, `BRINGIN` action
- `test_encode_all_in` — verify `_AI` suffix on open/call/raise
- `test_encode_walk` — verify `WALK` detection and encoding
- `test_encode_hi_lo` — verify H/L hand produces standard result format (no _HI/_LO suffixes — ACR doesn't distinguish in collected lines)
- `test_pot_tracker_accuracy` — construct a multi-street hand, verify pot amounts at each street boundary match expected values
- `test_postflop_raise_multiplier` — bet then raise, verify raise expressed as multiplier of the bet
- `test_uncalled_bet_adjusts_pot` — verify UncalledBet reduces pot for next street's sizing
- `test_sat_out` — verify `SAT_OUT` output
- `test_net_result_calculation` — verify net BB = collected - hero_invested for a won hand
- `test_call_sizing_preflop` — verify Call includes BB size: `CALL(3bb)` not bare `CALL`
- `test_call_sizing_postflop` — verify Call includes pot fraction: `CALL(0.67pot)`

Use parsed hands from `tests/fixtures/` via the parser for additional realistic integration tests in `tests/action_encoder_tests.rs`.

---

## Step 2: Qdrant Named Vectors Migration

**File:** `src/storage.rs`

### 2a. Update `ensure_collection` to use named vectors

Replace the single `VectorParamsBuilder::new(384, Distance::Cosine)` with a named vectors config using `VectorsConfigBuilder`:

```rust
use qdrant_client::qdrant::VectorsConfigBuilder;

let mut vectors_config = VectorsConfigBuilder::default();
vectors_config.add_named_vector_params(
    "summary",
    VectorParamsBuilder::new(384, Distance::Cosine),
);
vectors_config.add_named_vector_params(
    "action",
    VectorParamsBuilder::new(384, Distance::Cosine),
);

self.client
    .create_collection(
        CreateCollectionBuilder::new(&self.collection)
            .vectors_config(vectors_config),
    )
    .await
    .context("Failed to create collection")?;
```

**Migration strategy:** This is a breaking change. The collection schema changes from a single unnamed vector to named vectors.
1. `ensure_collection` creates new collections with named vectors
2. If the collection already exists, check whether it has named vectors by calling `collection_info` and inspecting the vectors config. If it has the old single-vector schema, print a clear error: `"Collection '{}' uses an old schema. Delete it and re-import: cargo run -- import"` and return an error
3. To delete the old collection, users run the Qdrant REST API (`curl -X DELETE localhost:6333/collections/poker_hands`) or we add a `--force-recreate` flag to the import command that drops and recreates the collection

### 2b. Update `build_point` signature

```rust
fn build_point(
    hand: &Hand,
    summary: &str,
    action_encoding: &str,
    summary_embedding: Vec<f32>,
    action_embedding: Vec<f32>,
) -> PointStruct
```

Construct the point with named vectors using the qdrant-client 1.16 API:

```rust
use qdrant_client::qdrant::{NamedVectors, Vector};

let vectors = NamedVectors::default()
    .add_vector("summary", Vector::new_dense(summary_embedding))
    .add_vector("action", Vector::new_dense(action_embedding));

// Also store action_encoding in payload for debugging/display
payload["action_encoding"] = json!(action_encoding);

PointStruct::new(hand.id, vectors, payload_map)
```

### 2c. Update `upsert_hand` and `upsert_hands_batch` signatures

Bundle both embeddings in a struct to keep signatures clean:

```rust
pub struct HandEmbeddings {
    pub summary: Vec<f32>,
    pub action: Vec<f32>,
}
```

Updated signatures:

```rust
pub async fn upsert_hand(
    &self,
    hand: &Hand,
    summary: &str,
    action_encoding: &str,
    embeddings: HandEmbeddings,
) -> Result<()>

pub async fn upsert_hands_batch(
    &self,
    items: Vec<(&Hand, &str, &str, HandEmbeddings)>,  // (hand, summary, action_encoding, embeddings)
) -> Result<()>
```

### 2d. Update `search` to accept a vector name

```rust
pub async fn search(
    &self,
    vector_name: &str,      // "summary" or "action"
    query_embedding: Vec<f32>,
    limit: u64,
    filter: Option<Filter>,
) -> Result<Vec<SearchResult>>
```

Use `.vector_name()` on the builder:

```rust
let mut builder = SearchPointsBuilder::new(&self.collection, query_embedding, limit)
    .vector_name(vector_name)
    .with_payload(true)
    .params(SearchParamsBuilder::default().exact(false));
```

### 2e. Add `get_hand_vector` method

```rust
pub async fn get_hand_vector(
    &self,
    hand_id: u64,
    vector_name: &str,
) -> Result<Option<Vec<f32>>>
```

Uses `GetPointsBuilder` with `.with_vectors(true)` — the current `get_hand` only uses `.with_payload(true)`, and vectors are NOT returned by default.

**Extracting the named vector from the response** requires matching through several layers:

```rust
use qdrant_client::qdrant::vectors::VectorsOptions;

let point = result.result.first()?;
let vectors = point.vectors.as_ref()?;
match &vectors.vectors_options {
    Some(VectorsOptions::Vectors(named)) => {
        let vector = named.vectors.get(vector_name)?;
        Some(vector.data.clone())  // Vec<f32>
    }
    _ => None,
}
```

The path is: `RetrievedPoint.vectors` (`Option<Vectors>`) → `.vectors_options` (`Option<VectorsOptions>`) → match `VectorsOptions::Vectors(NamedVectors)` → `.vectors` (`HashMap<String, Vector>`) → `.data` (`Vec<f32>`). Verify these exact types compile against qdrant-client 1.16 before relying on them — the protobuf-generated types can be surprising.

### Tests for Step 2

- `test_ensure_collection_named_vectors` — `#[ignore]` (requires Qdrant), create collection, verify it has two named vectors via `collection_info`
- Update existing `test_ensure_collection` for the new schema

---

## Step 3: Import Pipeline Update

**File:** `src/main.rs` (`import_one` function)

### 3a. Generate both embeddings during import

In the batch processing loop, after generating summaries:

```rust
let summaries: Vec<String> = to_process
    .iter()
    .map(|h| summarizer::summarize(h))
    .collect();
let action_encodings: Vec<String> = to_process
    .iter()
    .map(|h| action_encoder::encode_action_sequence(h, hero))
    .collect();

let summary_refs: Vec<&str> = summaries.iter().map(|s| s.as_str()).collect();
let action_refs: Vec<&str> = action_encodings.iter().map(|s| s.as_str()).collect();

let summary_embeddings = embedder.embed_batch(&summary_refs)?;
let action_embeddings = embedder.embed_batch(&action_refs)?;
```

### 3b. Update batch construction

```rust
let batch: Vec<_> = to_process.into_iter()
    .zip(summaries.iter())
    .zip(action_encodings.iter())
    .zip(summary_embeddings.into_iter().zip(action_embeddings.into_iter()))
    .map(|(((hand, summary), action_enc), (sum_emb, act_emb))| {
        (hand, summary.as_str(), action_enc.as_str(), storage::HandEmbeddings {
            summary: sum_emb,
            action: act_emb,
        })
    })
    .collect();

store.upsert_hands_batch(batch).await?;
```

**Performance note:** This doubles the embedding calls per batch. Each `embed_batch` processes 32 texts, we now make two calls instead of one. For the current dataset (~18 files) this is negligible. For large imports, an optimization would be to interleave summaries and action encodings into a single `embed_batch` call of 64 texts and split the results — but this is a future optimization, not needed now.

---

## Step 4: Search Module Update

**File:** `src/search.rs`

### 4a. Add search mode enum

```rust
#[derive(Debug, Clone, Copy, Default)]
pub enum SearchMode {
    #[default]
    Semantic,
    Action,
}
```

### 4b. Add `mode` field to `SearchParams`

```rust
pub struct SearchParams {
    pub query: String,
    pub mode: SearchMode,   // new field
    pub position: Option<String>,
    pub pot_type: Option<String>,
    pub villain: Option<String>,
    pub stakes: Option<String>,
    pub result: Option<String>,
    pub game_type: Option<String>,
    pub variant: Option<String>,
    pub betting_limit: Option<String>,
    pub limit: Option<u64>,
}
```

**Call site updates required:**
- `src/mcp.rs` `search_hands` handler — add `mode` field when constructing `SearchParams`
- `src/search.rs` tests (`test_build_filter_empty`, `test_build_filter_position`, `test_build_filter_multiple`) — add `mode: SearchMode::default()` to each `SearchParams` construction

### 4c. Update `search_hands`

Route to the appropriate vector name based on mode:

```rust
let vector_name = match params.mode {
    SearchMode::Semantic => "summary",
    SearchMode::Action => "action",
};

let results = store
    .search(vector_name, query_embedding, limit, filter)
    .await
    .context("Search failed")?;
```

The query text is always embedded with the same BGE-small-en-v1.5 model — the difference is which stored vector it's compared against.

### 4d. Add `search_similar_actions` (by hand ID)

```rust
pub async fn search_similar_actions(
    store: &VectorStore,
    hand_id: u64,
    vector_name: &str,  // "action" or "summary"
    limit: u64,
    filter: Option<Filter>,
) -> Result<Vec<HandSearchResult>>
```

1. Call `store.get_hand_vector(hand_id, vector_name)` to retrieve the source hand's embedding
2. Search with that embedding against the same named vector
3. Exclude the source hand from results using a `must_not` filter condition:
   ```rust
   use qdrant_client::qdrant::HasIdCondition;

   let exclude = Condition::has_id(vec![PointId::from(hand_id)]);
   // Merge with existing filter: add exclude to must_not
   let mut combined_filter = filter.unwrap_or_default();
   combined_filter.must_not.push(exclude.into());
   ```
4. Return results as `Vec<HandSearchResult>`

---

## Step 5: MCP Tool Updates

**File:** `src/mcp.rs`

### 5a. Add `search_mode` to `SearchHandsParams`

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchHandsParams {
    // ... existing fields ...
    #[schemars(description = "Search mode: 'semantic' (default, matches narrative descriptions) or 'action' (matches betting line structure)")]
    pub search_mode: Option<String>,
}
```

Map in the `search_hands` tool handler:

```rust
let mode = match params.search_mode.as_deref() {
    Some("action") => search::SearchMode::Action,
    _ => search::SearchMode::Semantic,
};
```

Pass to `SearchParams { mode, ... }`.

### 5b. Add `search_similar_hands` tool

New parameter struct:

```rust
#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSimilarParams {
    #[schemars(description = "Hand ID to find similar hands for")]
    pub hand_id: u64,
    #[schemars(description = "Similarity mode: 'action' (default, matches betting structure), 'semantic' (matches narrative)")]
    pub mode: Option<String>,
    #[schemars(description = "Max results (default 10)")]
    pub limit: Option<u64>,
}
```

Tool handler:
1. Map mode string to vector name (`"action"` → `"action"`, `"semantic"` → `"summary"`, default `"action"`)
2. Call `search::search_similar_actions(store, hand_id, vector_name, limit, None)`
3. Return results in the same JSON format as `search_hands`

Register with `#[tool_handler]` macro, add to `#[tool_router]`. This brings the tool count from 6 to 7.

---

## Step 6: Integration Testing & Docs

### 6a. Integration tests

**File:** `tests/action_encoder_tests.rs` (new)

Parse real hands from `tests/fixtures/` via the parser, encode their action sequences, verify:
- Output is deterministic (same hand → same encoding every time)
- Stud hands produce stud-format encodings (`3RD:` labels, no board)
- Bomb pots produce `BOMB_POT` prefix

### 6b. Embedding similarity test (`#[ignore]`, requires model)

- Parse two hands with similar action structures but different stakes/cards
- Encode both, embed both with `Embedder`
- Verify cosine similarity > 0.7 (conservative threshold — structured text may not embed as tightly as natural language)

### 6c. Qdrant integration test (`#[ignore]`, requires Qdrant)

- Create a test collection with named vectors
- Import a few hands with both embeddings
- Search by action vector, verify results return
- Search by summary vector, verify results return
- Test `search_similar_actions` by hand ID
- Clean up test collection

### 6d. Update CLAUDE.md

After the feature is complete, update:
- **Key modules** list — add `src/action_encoder.rs` with description
- **Architecture / Full pipeline** — mention dual embedding (summary + action)
- **MCP tools list** — add `search_similar_hands`, note `search_mode` param on `search_hands`

---

## File Change Summary

| File | Change |
|------|--------|
| `src/embedder.rs` | Switch model from all-MiniLM-L6-v2 to BGE-small-en-v1.5 (512 token limit) |
| `src/action_encoder.rs` | **New** — action sequence encoding with PotTracker, ActionLabel classification |
| `src/main.rs` | Add `mod action_encoder`, update `import_one` for dual embeddings |
| `src/lib.rs` | Add `pub mod action_encoder` |
| `src/storage.rs` | Named vectors via `VectorsConfigBuilder`, update `build_point`/`upsert_*`/`search`, add `HandEmbeddings` struct, add `get_hand_vector` |
| `src/search.rs` | Add `SearchMode`, update `SearchParams` + all construction sites, add `search_similar_actions` |
| `src/mcp.rs` | Add `search_mode` to `SearchHandsParams`, add `search_similar_hands` tool |
| `tests/action_encoder_tests.rs` | **New** — integration tests with real parsed hands |
| `CLAUDE.md` | Update architecture, modules, and MCP tools documentation |

## Order of Operations

0. **Step 0** (model switch) — one-line change in `embedder.rs`, verify ONNX path. Do this first since everything needs re-embedding anyway.
1. **Step 1** (action encoder) — standalone, no dependencies on other changes, fully testable in isolation. This is the hardest step due to pot tracking and action classification logic. Can be developed in parallel with Step 0.
2. **Step 2** (storage migration) — can be developed in parallel with Step 1.
3. **Step 3** (import pipeline) — depends on Steps 0 + 1 + 2.
4. **Step 4** (search updates) — depends on Step 2. Can be developed in parallel with Step 3.
5. **Step 5** (MCP tools) — depends on Steps 3 + 4.
6. **Step 6** (testing + docs) — depends on all above.

## Risks / Open Questions

1. **Embedding quality for structured text** — BGE-small-en-v1.5 is trained on natural language. The action encoding format (`PRE: HERO_OPEN(3bb) V1_3BET(9bb)`) is synthetic/structured. It may not embed well for fine-grained similarity. Mitigation: the encoding uses English-like tokens (OPEN, CALL, RAISE, FOLD, CHECK, CBET) that the model's vocabulary should handle. BGE-small scores higher than MiniLM on retrieval benchmarks, which may help. If similarity quality is poor after testing (Step 6b), fall back to Option B from the plan doc — concatenate the action encoding with the natural language summary into a single embedding. This would be a simpler change: no named vectors needed, just a richer input to the existing single embedding.

2. **Import time doubles** — Two `embed_batch` calls per chunk. Negligible for current dataset. Future optimization: interleave into a single batch of 64 texts.

3. **Breaking migration** — Existing users must delete their Qdrant collection and re-import. The `ensure_collection` schema check (Step 2a) ensures they get a clear error message rather than a confusing runtime failure.

4. **Pot tracking edge cases** — All-in side pots create divergent effective pots per player. The PotTracker uses a single `current_pot` which is the main pot total. For sizing purposes, this is close enough — the encoding captures "bet X relative to pot Y" which is what matters for structural similarity. Exact side pot accounting is unnecessary for the embedding use case.

5. **C-bet detection simplification** — We only detect c-bets for the preflop aggressor betting first on a postflop street. We don't detect delayed c-bets (checking one street, betting the next), donk bets, or probe bets. These could be added later but aren't needed for v1.

6. **Token length limit (mitigated)** — Resolved by switching to BGE-small-en-v1.5 (512 tokens) in Step 0. A typical hand encodes to ~100-150 tokens. Even extreme raise-wars (~200+ tokens) fit comfortably within 512. No longer a practical concern.
