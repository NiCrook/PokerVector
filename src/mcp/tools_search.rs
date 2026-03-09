use rmcp::model::*;

use crate::search::{self, SearchParams};

use super::helpers::mcp_error;
use super::params::{SearchHandsParams, SearchSimilarParams};
use super::PokerVectorMcp;

impl PokerVectorMcp {
    pub(crate) async fn tool_search_hands(
        &self,
        params: SearchHandsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let mut embedder = self.embedder.lock().await;
        let mode = match params.search_mode.as_deref() {
            Some("action") => search::SearchMode::Action,
            _ => search::SearchMode::Semantic,
        };
        let search_params = SearchParams {
            query: params.query,
            mode,
            position: params.position,
            pot_type: params.pot_type,
            villain: params.villain,
            stakes: params.stakes,
            result: params.result,
            game_type: params.game_type,
            variant: params.variant,
            betting_limit: params.betting_limit,
            limit: params.limit,
            offset: params.offset,
            from_date: params.from_date,
            to_date: params.to_date,
            tag: params.tag,
        };
        let results = search::search_hands(&self.store, &mut *embedder, search_params)
            .await
            .map_err(|e| mcp_error(&format!("Search failed: {}", e)))?;
        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_search_similar_hands(
        &self,
        params: SearchSimilarParams,
    ) -> Result<CallToolResult, ErrorData> {
        let vector_name = match params.mode.as_deref() {
            Some("semantic") => "summary",
            _ => "action",
        };
        let limit = params.limit.unwrap_or(10);

        let results =
            search::search_similar_actions(&self.store, params.hand_id, vector_name, limit, None)
                .await
                .map_err(|e| mcp_error(&format!("Similar search failed: {}", e)))?;

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
