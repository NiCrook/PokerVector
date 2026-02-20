use anyhow::{Context, Result};
use qdrant_client::qdrant::{Condition, Filter, PointId};

use crate::embedder::Embedder;
use crate::storage::VectorStore;

#[derive(Debug, Clone, Copy, Default)]
pub enum SearchMode {
    #[default]
    Semantic,
    Action,
}

pub struct SearchParams {
    pub query: String,
    pub mode: SearchMode,
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

#[derive(serde::Serialize)]
pub struct HandSearchResult {
    pub hand_id: u64,
    pub score: f32,
    pub summary: String,
    pub hero_position: String,
    pub hero_cards: String,
    pub stakes: String,
    pub hero_result: String,
    pub pot_type: String,
}

/// Build a Qdrant filter from search parameters.
pub fn build_filter(params: &SearchParams) -> Option<Filter> {
    let mut conditions = Vec::new();

    if let Some(ref pos) = params.position {
        conditions.push(Condition::matches("hero_position", pos.clone()));
    }
    if let Some(ref pt) = params.pot_type {
        conditions.push(Condition::matches("pot_type", pt.clone()));
    }
    if let Some(ref villain) = params.villain {
        conditions.push(Condition::matches("opponent_names", villain.clone()));
    }
    if let Some(ref stakes) = params.stakes {
        conditions.push(Condition::matches("stakes", stakes.clone()));
    }
    if let Some(ref result) = params.result {
        conditions.push(Condition::matches("hero_result", result.clone()));
    }
    if let Some(ref gt) = params.game_type {
        conditions.push(Condition::matches("game_type", gt.clone()));
    }
    if let Some(ref v) = params.variant {
        conditions.push(Condition::matches("variant", v.clone()));
    }
    if let Some(ref bl) = params.betting_limit {
        conditions.push(Condition::matches("betting_limit", bl.clone()));
    }

    if conditions.is_empty() {
        None
    } else {
        Some(Filter::must(conditions))
    }
}

/// Search hands using semantic similarity with optional filters.
pub async fn search_hands(
    store: &VectorStore,
    embedder: &mut Embedder,
    params: SearchParams,
) -> Result<Vec<HandSearchResult>> {
    let limit = params.limit.unwrap_or(10);
    let filter = build_filter(&params);

    let vector_name = match params.mode {
        SearchMode::Semantic => "summary",
        SearchMode::Action => "action",
    };

    let query_embedding = embedder
        .embed(&params.query)
        .context("Failed to embed search query")?;

    let results = store
        .search(vector_name, query_embedding, limit, filter)
        .await
        .context("Search failed")?;

    Ok(results
        .into_iter()
        .map(|r| {
            let get_str = |key: &str| -> String {
                r.payload
                    .get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };

            HandSearchResult {
                hand_id: r.hand_id,
                score: r.score,
                summary: r.summary,
                hero_position: get_str("hero_position"),
                hero_cards: get_str("hero_cards"),
                stakes: get_str("stakes"),
                hero_result: get_str("hero_result"),
                pot_type: get_str("pot_type"),
            }
        })
        .collect())
}

/// Search for hands similar to a given hand by its stored vector.
pub async fn search_similar_actions(
    store: &VectorStore,
    hand_id: u64,
    vector_name: &str,
    limit: u64,
    filter: Option<Filter>,
) -> Result<Vec<HandSearchResult>> {
    let embedding = store
        .get_hand_vector(hand_id, vector_name)
        .await
        .context("Failed to get hand vector")?
        .ok_or_else(|| anyhow::anyhow!("Hand {} not found or has no '{}' vector", hand_id, vector_name))?;

    // Exclude the source hand from results
    let exclude = Condition::has_id(vec![PointId::from(hand_id)]);
    let mut combined_filter = filter.unwrap_or_default();
    combined_filter.must_not.push(exclude.into());

    let results = store
        .search(vector_name, embedding, limit, Some(combined_filter))
        .await
        .context("Similar search failed")?;

    Ok(results
        .into_iter()
        .map(|r| {
            let get_str = |key: &str| -> String {
                r.payload
                    .get(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };

            HandSearchResult {
                hand_id: r.hand_id,
                score: r.score,
                summary: r.summary,
                hero_position: get_str("hero_position"),
                hero_cards: get_str("hero_cards"),
                stakes: get_str("stakes"),
                hero_result: get_str("hero_result"),
                pot_type: get_str("pot_type"),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_filter_empty() {
        let params = SearchParams {
            query: "hero raises".to_string(),
            mode: SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: None,
            result: None,
            game_type: None,
            variant: None,
            betting_limit: None,
            limit: None,
        };
        assert!(build_filter(&params).is_none());
    }

    #[test]
    fn test_build_filter_position() {
        let params = SearchParams {
            query: "hero raises".to_string(),
            mode: SearchMode::default(),
            position: Some("BTN".to_string()),
            pot_type: None,
            villain: None,
            stakes: None,
            result: None,
            game_type: None,
            variant: None,
            betting_limit: None,
            limit: None,
        };
        let filter = build_filter(&params);
        assert!(filter.is_some());
        let f = filter.unwrap();
        assert_eq!(f.must.len(), 1);
    }

    #[test]
    fn test_build_filter_multiple() {
        let params = SearchParams {
            query: "hero raises".to_string(),
            mode: SearchMode::default(),
            position: Some("BTN".to_string()),
            pot_type: Some("3bet".to_string()),
            villain: Some("Fish".to_string()),
            stakes: Some("$0.01/$0.02".to_string()),
            result: Some("won".to_string()),
            game_type: None,
            variant: None,
            betting_limit: None,
            limit: Some(5),
        };
        let filter = build_filter(&params);
        assert!(filter.is_some());
        let f = filter.unwrap();
        assert_eq!(f.must.len(), 5);
    }
}
