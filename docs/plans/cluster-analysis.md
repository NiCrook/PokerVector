# Cluster Analysis

**Category:** Advanced Embedding / Search
**Summary:** Group similar hands to find patterns you didn't think to search for.

## Current State

- All hands are embedded as 384-dimensional vectors in Qdrant
- Search is query-driven — user must know what to search for
- No unsupervised pattern discovery exists
- Qdrant supports scroll (full scan) which can retrieve all embeddings

## Problem

Query-based search requires the user to articulate what they're looking for. Clustering reveals patterns the user hasn't thought to search for:
- "You lose most of your money in 3-bet pots from the blinds"
- "Your biggest wins come from river value bets in single-raised pots"
- "There's a cluster of hands where you c-bet and fold to a raise — this happens 40 times"

## Build Plan

### Step 1: Embedding Retrieval

**File:** `src/cluster.rs` (new module)

Retrieve all embeddings from Qdrant for clustering:
```rust
pub fn retrieve_embeddings(store: &VectorStore, filter: Option<Filter>) -> Result<Vec<(u64, Vec<f32>, Hand)>>
```

Use `scroll_hands` with `with_vectors: true` to get embeddings alongside hand data. This may require extending `VectorStore::scroll_hands()` to optionally return vectors.

### Step 2: K-Means Clustering

**File:** `src/cluster.rs`

Implement simple k-means clustering (no external ML crate needed for basic k-means):

```rust
pub struct Cluster {
    pub cluster_id: u32,
    pub centroid: Vec<f32>,
    pub hand_ids: Vec<u64>,
    pub size: u32,
    pub label: String,              // auto-generated from common features
    pub common_features: ClusterFeatures,
}

pub struct ClusterFeatures {
    pub dominant_position: Option<String>,
    pub dominant_pot_type: Option<String>,
    pub dominant_result: Option<String>,
    pub avg_pot_size_bb: f64,
    pub dominant_variant: Option<String>,
    pub dominant_action_pattern: Option<String>,
}

pub struct ClusterResult {
    pub clusters: Vec<Cluster>,
    pub total_hands: u32,
    pub silhouette_score: f64,      // clustering quality metric
}

pub fn cluster_hands(
    embeddings: &[(u64, Vec<f32>, Hand)],
    k: usize,
) -> ClusterResult
```

K-means implementation:
1. Initialize centroids with k-means++ (select spread-out initial centers)
2. Assign each point to nearest centroid (cosine similarity)
3. Recompute centroids as mean of assigned points
4. Repeat until convergence (max 100 iterations)
5. Compute silhouette score for quality assessment

### Step 3: Automatic Cluster Labeling

**File:** `src/cluster.rs`

After clustering, analyze the hands in each cluster to generate a human-readable label:

```rust
pub fn label_cluster(hands: &[Hand], hero: &str) -> (String, ClusterFeatures)
```

Labeling logic:
1. Find the mode of key features: position, pot_type, hero_result, variant
2. Compute average pot size in BB
3. Identify the most common action pattern (using action line summarization from showdown-analysis plan)
4. Generate label like: "3-bet pots from BTN, hero wins (23 hands)" or "SRP from BB, hero folds to c-bet (41 hands)"

### Step 4: Optimal K Selection

**File:** `src/cluster.rs`

Automatically determine good k using the elbow method:
```rust
pub fn find_optimal_k(embeddings: &[(u64, Vec<f32>)], max_k: usize) -> usize
```

Run k-means for k = 2..max_k, compute within-cluster sum of squares (WCSS) for each, find the "elbow" point where adding more clusters gives diminishing returns.

Default `max_k` = min(20, sqrt(n_hands)).

### Step 5: Cluster Insights

**File:** `src/cluster.rs`

For each cluster, compute actionable insights:
```rust
pub struct ClusterInsight {
    pub cluster_id: u32,
    pub insight_type: String,       // "leak", "strength", "pattern"
    pub description: String,
    pub avg_profit_bb: f64,
    pub frequency: u32,
}
```

Insights derived from:
- **Losing clusters** — clusters where average profit is significantly negative → potential leaks
- **Winning clusters** — clusters where average profit is significantly positive → strengths to maintain
- **Large clusters** — frequently occurring situations → important patterns to review
- **Outlier clusters** — very small clusters with extreme results → unusual spots worth examining

### Step 6: Expose via MCP

**File:** `src/mcp.rs`

New tool `cluster_hands`:
```rust
pub struct ClusterHandsParams {
    pub k: Option<u32>,             // number of clusters (auto if omitted)
    pub position: Option<String>,   // pre-filter by position
    pub game_type: Option<String>,  // pre-filter by game type
    pub hero: Option<String>,
}
```

Returns `ClusterResult` with labeled clusters and insights.

## Dependencies

- Existing `VectorStore` — needs scroll with vectors
- No external ML crate needed — k-means is simple enough to implement
- Action encoder (from action-sequence-embeddings plan) for cluster labeling, but can use simpler heuristics initially

## Performance Considerations

- K-means on 10,000 hands with 384-dim vectors should run in seconds
- For very large datasets (100K+ hands), consider:
  - Mini-batch k-means (process subsets)
  - Dimensionality reduction (PCA to 50 dims before clustering)
  - Pre-filtering to relevant subsets
- Qdrant scroll with vectors will transfer significant data — do this once, cache in memory

## Testing

- Unit test k-means with synthetic 2D data and known cluster structure
- Unit test cluster labeling with hands that have clear groupings
- Unit test optimal k selection with known elbow point
- Test with all hands in one cluster (k=1) and each hand in its own cluster (k=n)
- Integration test with `PolarFox/` data — verify clusters are non-trivial and labels are reasonable
