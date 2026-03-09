use anyhow::{Context, Result};

use crate::embedder::Embedder;
use crate::storage::{SearchResult, VectorStore};

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
    pub offset: Option<u64>,
    pub from_date: Option<String>,
    pub to_date: Option<String>,
    pub tag: Option<String>,
}

fn sanitize(value: &str) -> String {
    value.replace('\'', "''")
}

/// Build a SQL WHERE filter from search parameters.
pub fn build_filter(params: &SearchParams) -> Option<String> {
    let mut conditions = Vec::new();

    if let Some(ref pos) = params.position {
        conditions.push(format!("hero_position = '{}'", sanitize(pos)));
    }
    if let Some(ref pt) = params.pot_type {
        conditions.push(format!("pot_type = '{}'", sanitize(pt)));
    }
    if let Some(ref villain) = params.villain {
        conditions.push(format!("opponent_names LIKE '%,{},%'", sanitize(villain)));
    }
    if let Some(ref stakes) = params.stakes {
        conditions.push(format!("stakes = '{}'", sanitize(stakes)));
    }
    if let Some(ref result) = params.result {
        conditions.push(format!("hero_result = '{}'", sanitize(result)));
    }
    if let Some(ref gt) = params.game_type {
        conditions.push(format!("game_type = '{}'", sanitize(gt)));
    }
    if let Some(ref v) = params.variant {
        conditions.push(format!("variant = '{}'", sanitize(v)));
    }
    if let Some(ref bl) = params.betting_limit {
        conditions.push(format!("betting_limit = '{}'", sanitize(bl)));
    }
    if let Some(ref from) = params.from_date {
        conditions.push(format!("timestamp >= '{}'", sanitize(from)));
    }
    if let Some(ref to) = params.to_date {
        conditions.push(format!("timestamp <= '{}'", sanitize(to)));
    }
    if let Some(ref tag) = params.tag {
        conditions.push(format!("tags LIKE '%,{},%'", sanitize(tag)));
    }

    if conditions.is_empty() {
        None
    } else {
        Some(conditions.join(" AND "))
    }
}

/// Search hands using semantic similarity with optional filters.
pub async fn search_hands(
    store: &VectorStore,
    embedder: &mut Embedder,
    params: SearchParams,
) -> Result<Vec<SearchResult>> {
    let limit = params.limit.unwrap_or(10);
    let offset = params.offset.unwrap_or(0);
    let filter = build_filter(&params);

    let vector_name = match params.mode {
        SearchMode::Semantic => "summary",
        SearchMode::Action => "action",
    };

    let query_embedding = embedder
        .embed(&params.query)
        .context("Failed to embed search query")?;

    // Fetch limit+offset results then skip the first `offset`
    let fetch_count = limit + offset;
    let mut results = store
        .search(vector_name, query_embedding, fetch_count, filter)
        .await
        .context("Search failed")?;

    if offset > 0 {
        let skip = (offset as usize).min(results.len());
        results = results.split_off(skip);
    }
    results.truncate(limit as usize);

    Ok(results)
}

/// Search for hands similar to a given hand by its stored vector.
pub async fn search_similar_actions(
    store: &VectorStore,
    hand_id: u64,
    vector_name: &str,
    limit: u64,
    filter: Option<String>,
) -> Result<Vec<SearchResult>> {
    let embedding = store
        .get_hand_vector(hand_id, vector_name)
        .await
        .context("Failed to get hand vector")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Hand {} not found or has no '{}' vector",
                hand_id,
                vector_name
            )
        })?;

    // Exclude the source hand from results
    let exclude = format!("id != {}", hand_id);
    let combined = match filter {
        Some(f) => format!("{} AND {}", f, exclude),
        None => exclude,
    };

    let results = store
        .search(vector_name, embedding, limit, Some(combined))
        .await
        .context("Similar search failed")?;

    Ok(results)
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
            offset: None,
            from_date: None,
            to_date: None,
            tag: None,
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
            offset: None,
            from_date: None,
            to_date: None,
            tag: None,
        };
        let filter = build_filter(&params);
        assert_eq!(filter, Some("hero_position = 'BTN'".to_string()));
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
            offset: None,
            from_date: None,
            to_date: None,
            tag: None,
        };
        let filter = build_filter(&params).unwrap();
        assert!(filter.contains("hero_position = 'BTN'"));
        assert!(filter.contains("pot_type = '3bet'"));
        assert!(filter.contains("opponent_names LIKE '%,Fish,%'"));
        assert!(filter.contains("stakes = '$0.01/$0.02'"));
        assert!(filter.contains("hero_result = 'won'"));
        assert!(filter.contains(" AND "));
    }

    #[test]
    fn test_sanitize_single_quotes() {
        let params = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::default(),
            position: None,
            pot_type: None,
            villain: Some("O'Brien".to_string()),
            stakes: None,
            result: None,
            game_type: None,
            variant: None,
            betting_limit: None,
            limit: None,
            offset: None,
            from_date: None,
            to_date: None,
            tag: None,
        };
        let filter = build_filter(&params).unwrap();
        assert!(filter.contains("O''Brien"));
    }

    #[test]
    fn test_villain_uses_like_with_commas() {
        let params = SearchParams {
            query: "test".to_string(),
            mode: SearchMode::default(),
            position: None,
            pot_type: None,
            villain: Some("Fish".to_string()),
            stakes: None,
            result: None,
            game_type: None,
            variant: None,
            betting_limit: None,
            limit: None,
            offset: None,
            from_date: None,
            to_date: None,
            tag: None,
        };
        let filter = build_filter(&params).unwrap();
        assert_eq!(filter, "opponent_names LIKE '%,Fish,%'");
    }
}
