use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::embedder::Embedder;
use crate::search::{self, SearchParams};
use crate::sessions;
use crate::stats;
use crate::storage::VectorStore;
use crate::summarizer;

#[derive(Clone)]
pub struct PokerVectorMcp {
    store: Arc<VectorStore>,
    embedder: Arc<Mutex<Embedder>>,
    hero: String,
    tool_router: ToolRouter<Self>,
}

impl PokerVectorMcp {
    pub fn new(store: VectorStore, embedder: Embedder, hero: String) -> Self {
        Self {
            store: Arc::new(store),
            embedder: Arc::new(Mutex::new(embedder)),
            hero,
            tool_router: Self::tool_router(),
        }
    }
}

// Parameter structs

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchHandsParams {
    #[schemars(description = "Natural language search query (e.g. 'hero 3-bets from the button')")]
    pub query: String,
    #[schemars(description = "Search mode: 'semantic' (default, matches narrative descriptions) or 'action' (matches betting line structure)")]
    pub search_mode: Option<String>,
    #[schemars(description = "Filter by hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter by pot type: SRP, 3bet, 4bet, limp, walk")]
    pub pot_type: Option<String>,
    #[schemars(description = "Filter by villain name")]
    pub villain: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by result: won, lost, folded")]
    pub result: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
    #[schemars(description = "Max results to return (default 10)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct SearchSimilarParams {
    #[schemars(description = "Hand ID to find similar hands for")]
    pub hand_id: u64,
    #[schemars(description = "Similarity mode: 'action' (default, matches betting structure), 'semantic' (matches narrative)")]
    pub mode: Option<String>,
    #[schemars(description = "Max results (default 10)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetHandParams {
    #[schemars(description = "The hand ID to retrieve")]
    pub hand_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetStatsParams {
    #[schemars(description = "Player name to compute stats for (defaults to hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter by villain name")]
    pub villain: Option<String>,
    #[schemars(description = "Filter by pot type: SRP, 3bet, 4bet, limp, walk")]
    pub pot_type: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Filter by variant: holdem, omaha, five_card_omaha, seven_card_stud")]
    pub variant: Option<String>,
    #[schemars(description = "Filter by betting limit: no_limit, pot_limit, fixed_limit")]
    pub betting_limit: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListVillainsParams {
    #[schemars(description = "Minimum number of hands to include a villain (default 10)")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ListSessionsParams {
    #[schemars(description = "Max sessions to return (default 20, most recent first)")]
    pub limit: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReviewSessionParams {
    #[schemars(description = "Session ID from list_sessions output")]
    pub session_id: u32,
}

// Tool implementations

#[tool_router]
impl PokerVectorMcp {
    #[tool(description = "Search poker hand histories using natural language with optional filters. Returns matching hands ranked by relevance.")]
    async fn search_hands(
        &self,
        Parameters(params): Parameters<SearchHandsParams>,
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
        };
        let results = search::search_hands(&self.store, &mut *embedder, search_params)
            .await
            .map_err(|e| mcp_error(&format!("Search failed: {}", e)))?;
        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Fetch full details of a specific hand by its numeric ID.")]
    async fn get_hand(
        &self,
        Parameters(params): Parameters<GetHandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hand = self
            .store
            .get_hand(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to retrieve hand: {}", e)))?;
        match hand {
            Some(h) => {
                let json = serde_json::to_string_pretty(&h)
                    .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
                Ok(CallToolResult::success(vec![Content::text(json)]))
            }
            None => Ok(CallToolResult::success(vec![Content::text(format!(
                "Hand {} not found",
                params.hand_id
            ))])),
        }
    }

    #[tool(description = "Get aggregate player statistics (VPIP, PFR, 3-bet%, etc.) with optional filters. Computes stats across all matching hands.")]
    async fn get_stats(
        &self,
        Parameters(params): Parameters<GetStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);

        // Build a filter from optional params
        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: params.position,
            pot_type: params.pot_type,
            villain: params.villain,
            stakes: params.stakes,
            result: None,
            game_type: params.game_type,
            variant: params.variant,
            betting_limit: params.betting_limit,
            limit: None,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let player_stats = stats::calculate_stats(&hands, hero);
        let json = serde_json::to_string_pretty(&player_stats)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List tracked opponents with hand counts and key stats (VPIP, PFR, aggression, etc.).")]
    async fn list_villains(
        &self,
        Parameters(params): Parameters<ListVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let min_hands = params.min_hands.unwrap_or(10);

        let hands = self
            .store
            .scroll_hands(None)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let villains = stats::list_villains(&hands, hero, min_hands);
        let json = serde_json::to_string_pretty(&villains)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List detected cash game sessions. Groups hands by table and play period. Sessions are separated by 30+ minutes of inactivity across all tables.")]
    async fn list_sessions(
        &self,
        Parameters(params): Parameters<ListSessionsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(20) as usize;

        // Scroll all cash hands
        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: None,
            result: None,
            game_type: Some("cash".to_string()),
            variant: None,
            betting_limit: None,
            limit: None,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let all_sessions = sessions::detect_sessions(hands, &self.hero);
        let sessions: Vec<_> = all_sessions.into_iter().take(limit).collect();

        // Build a summary view (without full hand data)
        let summary: Vec<serde_json::Value> = sessions
            .iter()
            .map(|s| {
                let table_names: Vec<&str> = s.tables.iter().map(|t| t.table_name.as_str()).collect();
                serde_json::json!({
                    "session_id": s.session_id,
                    "start_time": s.start_time,
                    "end_time": s.end_time,
                    "duration_minutes": s.duration_minutes,
                    "table_count": s.tables.len(),
                    "tables": table_names,
                    "total_hands": s.total_hands,
                    "net_profit": format!("{:.2}", s.net_profit),
                    "net_profit_bb": format!("{:.1}", s.net_profit_bb),
                })
            })
            .collect();

        let json = serde_json::to_string_pretty(&summary)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get a detailed review of a specific cash game session. Returns aggregate stats, per-table breakdown, and notable hands (biggest wins/losses).")]
    async fn review_session(
        &self,
        Parameters(params): Parameters<ReviewSessionParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // Scroll all cash hands
        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: None,
            result: None,
            game_type: Some("cash".to_string()),
            variant: None,
            betting_limit: None,
            limit: None,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let all_sessions = sessions::detect_sessions(hands, &self.hero);

        let session = all_sessions
            .iter()
            .find(|s| s.session_id == params.session_id)
            .ok_or_else(|| mcp_error(&format!("Session {} not found", params.session_id)))?;

        // Generate summaries for notable hand lookup
        let summaries: Vec<(u64, String)> = session
            .tables
            .iter()
            .flat_map(|t| t.hands.iter())
            .map(|h| (h.id, summarizer::summarize(h)))
            .collect();

        let review = sessions::review_session(session, &self.hero, &summaries);

        // Build response without embedding full hand objects
        let table_summaries: Vec<serde_json::Value> = review
            .session
            .tables
            .iter()
            .map(|t| {
                serde_json::json!({
                    "table_name": t.table_name,
                    "stakes": t.stakes,
                    "hand_count": t.hand_count,
                    "start_time": t.start_time,
                    "end_time": t.end_time,
                    "net_profit": format!("{:.2}", t.net_profit),
                })
            })
            .collect();

        let response = serde_json::json!({
            "session_id": review.session.session_id,
            "start_time": review.session.start_time,
            "end_time": review.session.end_time,
            "duration_minutes": review.session.duration_minutes,
            "total_hands": review.session.total_hands,
            "net_profit": format!("{:.2}", review.session.net_profit),
            "net_profit_bb": format!("{:.1}", review.session.net_profit_bb),
            "tables": table_summaries,
            "stats": review.stats,
            "notable_hands": review.notable_hands,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Find hands with similar betting structure or narrative to a given hand ID. Default mode is 'action' (betting pattern similarity).")]
    async fn search_similar_hands(
        &self,
        Parameters(params): Parameters<SearchSimilarParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let vector_name = match params.mode.as_deref() {
            Some("semantic") => "summary",
            _ => "action",
        };
        let limit = params.limit.unwrap_or(10);

        let results = search::search_similar_actions(
            &self.store,
            params.hand_id,
            vector_name,
            limit,
            None,
        )
        .await
        .map_err(|e| mcp_error(&format!("Similar search failed: {}", e)))?;

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}

#[tool_handler]
impl ServerHandler for PokerVectorMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::LATEST,
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "pokervector".to_string(),
                title: Some("PokerVector".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: Some("Poker hand history engine — search, stats, and analysis".to_string()),
                icons: None,
                website_url: None,
            },
            instructions: Some(
                "PokerVector: query your poker hand histories. Use search_hands for semantic or action-pattern search, \
                 get_hand for full hand details, get_stats for aggregate statistics, \
                 list_villains for opponent summaries, list_sessions to see cash game sessions, \
                 review_session for detailed session analysis, \
                 and search_similar_hands to find structurally similar hands by ID."
                    .to_string(),
            ),
        }
    }
}

fn mcp_error(msg: &str) -> ErrorData {
    ErrorData {
        code: ErrorCode::INTERNAL_ERROR,
        message: msg.to_string().into(),
        data: None,
    }
}
