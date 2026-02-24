use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::action_encoder;
use crate::config;
use crate::embedder::Embedder;
use crate::importer;
use crate::parsers;
use crate::search::{self, SearchParams};
use crate::sessions;
use crate::stats;
use crate::storage::{HandEmbeddings, VectorStore};
use crate::summarizer;
use crate::types::{ActionType, Card, GameType, HeroResult, PokerVariant, Rank, Street};

#[derive(Clone)]
pub struct PokerVectorMcp {
    store: Arc<VectorStore>,
    embedder: Arc<Mutex<Embedder>>,
    hero: String,
    accounts: Vec<config::Account>,
    tool_router: ToolRouter<Self>,
}

impl PokerVectorMcp {
    pub fn new(store: VectorStore, embedder: Embedder, hero: String, accounts: Vec<config::Account>) -> Self {
        Self {
            store: Arc::new(store),
            embedder: Arc::new(Mutex::new(embedder)),
            hero,
            accounts,
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
pub struct TableProfitabilityParams {
    #[schemars(description = "Group by 'stakes' (default) or 'table'")]
    pub group_by: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
    #[schemars(description = "Minimum hands per group to include in results (default 1)")]
    pub min_hands: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct BestVillainsParams {
    #[schemars(description = "Minimum hands played against villain (default 10)")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Max results to return (default 10)")]
    pub limit: Option<u64>,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WorstVillainsParams {
    #[schemars(description = "Minimum hands played against villain (default 10)")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Max results to return (default 10)")]
    pub limit: Option<u64>,
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

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct WatchDirectoryParams {
    #[schemars(description = "Override path to import from (default: all configured accounts)")]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetLastImportParams {}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct AutoTagHandsParams {
    #[schemars(description = "Big blind threshold for 'big' tags (default 20)")]
    pub min_pot_bb: Option<f64>,
    #[schemars(description = "Max hands per category (default 10)")]
    pub limit: Option<u64>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetCoolersParams {
    #[schemars(description = "Minimum pot size in big blinds (default 30)")]
    pub min_pot_bb: Option<f64>,
    #[schemars(description = "Max results to return (default 20)")]
    pub limit: Option<u64>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ExportHandsParams {
    #[schemars(description = "Export format: 'csv' (default) or 'raw' (original hand history text)")]
    pub format: Option<String>,
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
    #[schemars(description = "Max hands to export (default 100)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetHandAsReplayerParams {
    #[schemars(description = "The hand ID to replay")]
    pub hand_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct QuizHandParams {
    #[schemars(description = "Specific hand ID to quiz on (optional — picks a random qualifying hand if omitted)")]
    pub hand_id: Option<u64>,
    #[schemars(description = "Filter by hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: Option<String>,
    #[schemars(description = "Filter by pot type: SRP, 3bet, 4bet, limp, walk")]
    pub pot_type: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Target street for the quiz decision (default: auto-detect last hero decision street)")]
    pub street: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetSimilarVillainsParams {
    #[schemars(description = "Target VPIP percentage (0-100)")]
    pub vpip: f64,
    #[schemars(description = "Target PFR percentage (0-100)")]
    pub pfr: f64,
    #[schemars(description = "Target 3-bet percentage (0-100, optional)")]
    pub three_bet: Option<f64>,
    #[schemars(description = "Target aggression factor (optional, typically 0-5)")]
    pub aggression_factor: Option<f64>,
    #[schemars(description = "Minimum hands to qualify (default 20)")]
    pub min_hands: Option<u64>,
    #[schemars(description = "Max results to return (default 5)")]
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetPreflopChartParams {
    #[schemars(description = "Hero position: BTN, CO, HJ, LJ, SB, BB")]
    pub position: String,
    #[schemars(description = "Hero name override (defaults to configured hero)")]
    pub hero: Option<String>,
    #[schemars(description = "Filter by stakes (e.g. '$0.01/$0.02')")]
    pub stakes: Option<String>,
    #[schemars(description = "Filter by game type: cash or tournament")]
    pub game_type: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct ReimportHandParams {
    #[schemars(description = "The hand ID to re-parse and re-embed")]
    pub hand_id: u64,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
pub struct GetDatabaseHealthParams {}

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

    #[tool(description = "Show profitability grouped by stakes or table. Returns hands played, net profit, and bb/100 for each group.")]
    async fn get_table_profitability(
        &self,
        Parameters(params): Parameters<TableProfitabilityParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let group_by_table = params.group_by.as_deref() == Some("table");
        let min_hands = params.min_hands.unwrap_or(1);

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: None,
            result: None,
            game_type: params.game_type,
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

        // Group hands
        let mut groups: std::collections::HashMap<String, Vec<&crate::types::Hand>> =
            std::collections::HashMap::new();
        for hand in &hands {
            let key = if group_by_table {
                hand.table_name.clone()
            } else {
                hand.game_type.to_string()
            };
            groups.entry(key).or_default().push(hand);
        }

        let hero = &self.hero;
        let mut results: Vec<serde_json::Value> = groups
            .into_iter()
            .filter(|(_, h)| h.len() as u64 >= min_hands)
            .map(|(key, group_hands)| {
                let count = group_hands.len() as u64;
                let mut net_profit = 0.0f64;
                let mut net_profit_bb = 0.0f64;
                for hand in &group_hands {
                    let profit =
                        stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
                    net_profit += profit;
                    let bb = stats::big_blind_size(hand);
                    if bb > 0.0 {
                        net_profit_bb += profit / bb;
                    }
                }
                let bb_per_100 = if count > 0 {
                    net_profit_bb / count as f64 * 100.0
                } else {
                    0.0
                };
                serde_json::json!({
                    "group": key,
                    "hands": count,
                    "net_profit": format!("{:.2}", net_profit),
                    "net_profit_bb": format!("{:.1}", net_profit_bb),
                    "bb_per_100": format!("{:.1}", bb_per_100),
                })
            })
            .collect();

        results.sort_by(|a, b| {
            let pa: f64 = a["net_profit"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let pb: f64 = b["net_profit"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
        });

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List villains hero profits the most against. Returns opponents sorted by hero's net profit descending.")]
    async fn get_best_villains(
        &self,
        Parameters(params): Parameters<BestVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let min_hands = params.min_hands.unwrap_or(10);
        let limit = params.limit.unwrap_or(10) as usize;

        let hands = self
            .store
            .scroll_hands(None)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let villains = stats::list_villains(&hands, hero, min_hands);
        // Already sorted by net_profit descending
        let best: Vec<_> = villains.into_iter().take(limit).collect();

        let json = serde_json::to_string_pretty(&best)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "List villains hero loses the most against. Returns opponents sorted by hero's net profit ascending (biggest losers first).")]
    async fn get_worst_villains(
        &self,
        Parameters(params): Parameters<WorstVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let min_hands = params.min_hands.unwrap_or(10);
        let limit = params.limit.unwrap_or(10) as usize;

        let hands = self
            .store
            .scroll_hands(None)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let mut villains = stats::list_villains(&hands, hero, min_hands);
        villains.reverse(); // Now ascending by net_profit (worst first)
        let worst: Vec<_> = villains.into_iter().take(limit).collect();

        let json = serde_json::to_string_pretty(&worst)
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

    #[tool(description = "Import new hand histories from configured account directories (or a specific path). Updates the database with any new hands found.")]
    async fn watch_directory(
        &self,
        Parameters(params): Parameters<WatchDirectoryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let mut embedder = self.embedder.lock().await;

        let mut total_imported = 0u64;
        let mut total_skipped = 0u64;
        let mut total_errors = 0u64;
        let mut accounts_checked = 0u64;

        if let Some(path_str) = params.path {
            let path = std::path::PathBuf::from(&path_str);
            let result = importer::import_directory(&path, &self.hero, &mut *embedder, &self.store)
                .await
                .map_err(|e| mcp_error(&format!("Import failed: {}", e)))?;
            total_imported += result.imported;
            total_skipped += result.skipped;
            total_errors += result.errors;
            accounts_checked = 1;
        } else {
            if self.accounts.is_empty() {
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({
                        "error": "No accounts configured. Run `pokervector scan` or `pokervector add-account` first."
                    }).to_string()
                )]));
            }
            for account in &self.accounts {
                let result = importer::import_directory(&account.path, &account.hero, &mut *embedder, &self.store)
                    .await
                    .map_err(|e| mcp_error(&format!("Import failed for {}: {}", account.hero, e)))?;
                total_imported += result.imported;
                total_skipped += result.skipped;
                total_errors += result.errors;
                accounts_checked += 1;
            }
        }

        // Update import log in config
        let timestamp = chrono::Utc::now().to_rfc3339();
        if let Ok(mut cfg) = config::load_config() {
            cfg.last_import = Some(config::ImportLog {
                timestamp: timestamp.clone(),
                hands_imported: total_imported,
                hands_skipped: total_skipped,
                errors: total_errors,
            });
            let _ = config::save_config(&cfg);
        }

        let response = serde_json::json!({
            "imported": total_imported,
            "skipped": total_skipped,
            "errors": total_errors,
            "accounts_checked": accounts_checked,
            "timestamp": timestamp,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get information about the last import operation and total hands in the database.")]
    async fn get_last_import(
        &self,
        Parameters(_params): Parameters<GetLastImportParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let cfg = config::load_config()
            .map_err(|e| mcp_error(&format!("Failed to load config: {}", e)))?;

        let total_hands = self
            .store
            .count()
            .await
            .map_err(|e| mcp_error(&format!("Failed to count hands: {}", e)))?;

        let response = serde_json::json!({
            "last_import": cfg.last_import,
            "total_hands_in_db": total_hands,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Automatically classify hands into categories: cooler, hero_call, big_bluff, big_win, big_loss. Returns tagged hands grouped by category.")]
    async fn auto_tag_hands(
        &self,
        Parameters(params): Parameters<AutoTagHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let min_pot_bb = params.min_pot_bb.unwrap_or(20.0);
        let limit = params.limit.unwrap_or(10) as usize;
        let hero = &self.hero;

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: None,
            result: None,
            game_type: params.game_type,
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

        let mut coolers: Vec<serde_json::Value> = Vec::new();
        let mut hero_calls: Vec<serde_json::Value> = Vec::new();
        let mut big_bluffs: Vec<serde_json::Value> = Vec::new();
        let mut big_wins: Vec<serde_json::Value> = Vec::new();
        let mut big_losses: Vec<serde_json::Value> = Vec::new();

        for hand in &hands {
            let bb = stats::big_blind_size(hand);
            if bb <= 0.0 {
                continue;
            }
            let invested = stats::hero_invested(hand, hero);
            let collected = stats::hero_collected(hand, hero);
            let profit = collected - invested;
            let profit_bb = profit / bb;
            let invested_bb = invested / bb;
            let went_to_showdown = hand.result.hero_result == HeroResult::Won
                || hand.result.hero_result == HeroResult::Lost;
            let hero_won = hand.result.hero_result == HeroResult::Won;
            let hero_lost = hand.result.hero_result == HeroResult::Lost;

            let hand_summary = || {
                let cards: String = hand.hero_cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
                let board: String = hand.board.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
                serde_json::json!({
                    "hand_id": hand.id,
                    "stakes": hand.game_type.to_string(),
                    "hero_cards": cards,
                    "board": board,
                    "profit_bb": format!("{:.1}", profit_bb),
                    "pot_bb": format!("{:.1}", invested_bb + profit_bb.max(0.0)),
                })
            };

            // Cooler: went to showdown, hero invested a lot, hero lost
            if went_to_showdown && hero_lost && invested_bb > min_pot_bb {
                if coolers.len() < limit {
                    coolers.push(hand_summary());
                }
            }

            // Hero call: hero called on river, went to showdown, hero won
            if went_to_showdown && hero_won {
                let hero_called_river = hand.actions.iter().any(|a| {
                    a.player.as_str() == hero
                        && a.street == Street::River
                        && matches!(a.action_type, ActionType::Call { .. })
                });
                if hero_called_river && hero_calls.len() < limit {
                    hero_calls.push(hand_summary());
                }
            }

            // Big bluff: hero bet/raised on last street, no showdown, hero won, pot > threshold
            if hero_won && !went_to_showdown {
                let hero_bet_last_street = hand.actions.iter().rev().any(|a| {
                    a.player.as_str() == hero
                        && matches!(a.action_type, ActionType::Bet { .. } | ActionType::Raise { .. })
                        && matches!(a.street, Street::River | Street::Turn | Street::Flop)
                });
                if hero_bet_last_street && invested_bb > min_pot_bb / 2.0 && big_bluffs.len() < limit {
                    big_bluffs.push(hand_summary());
                }
            }

            // Big win
            if profit_bb > min_pot_bb && big_wins.len() < limit {
                big_wins.push(hand_summary());
            }

            // Big loss
            if profit_bb < -min_pot_bb && big_losses.len() < limit {
                big_losses.push(hand_summary());
            }
        }

        let response = serde_json::json!({
            "cooler": { "count": coolers.len(), "hands": coolers },
            "hero_call": { "count": hero_calls.len(), "hands": hero_calls },
            "big_bluff": { "count": big_bluffs.len(), "hands": big_bluffs },
            "big_win": { "count": big_wins.len(), "hands": big_wins },
            "big_loss": { "count": big_losses.len(), "hands": big_losses },
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Find cooler hands — showdown hands where hero invested heavily and lost. Sorted by pot size descending.")]
    async fn get_coolers(
        &self,
        Parameters(params): Parameters<GetCoolersParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let min_pot_bb = params.min_pot_bb.unwrap_or(30.0);
        let limit = params.limit.unwrap_or(20) as usize;
        let hero = &self.hero;

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: None,
            result: None,
            game_type: params.game_type,
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

        let mut coolers: Vec<(f64, serde_json::Value)> = Vec::new();

        for hand in &hands {
            let bb = stats::big_blind_size(hand);
            if bb <= 0.0 {
                continue;
            }
            let invested = stats::hero_invested(hand, hero);
            let collected = stats::hero_collected(hand, hero);
            let profit = collected - invested;
            let invested_bb = invested / bb;
            let profit_bb = profit / bb;
            let went_to_showdown = hand.result.hero_result == HeroResult::Won
                || hand.result.hero_result == HeroResult::Lost;

            if went_to_showdown && hand.result.hero_result == HeroResult::Lost && invested_bb > min_pot_bb {
                let cards: String = hand.hero_cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
                let board: String = hand.board.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
                let summary = summarizer::summarize(hand);
                coolers.push((invested_bb, serde_json::json!({
                    "hand_id": hand.id,
                    "stakes": hand.game_type.to_string(),
                    "hero_cards": cards,
                    "board": board,
                    "invested_bb": format!("{:.1}", invested_bb),
                    "profit_bb": format!("{:.1}", profit_bb),
                    "summary": summary,
                })));
            }
        }

        // Sort by invested BB descending (biggest pots first)
        coolers.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let results: Vec<serde_json::Value> = coolers.into_iter().take(limit).map(|(_, v)| v).collect();

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Export hands as CSV or raw hand history text with optional filters. CSV includes hand_id, timestamp, variant, stakes, hero position, cards, board, pot type, result, profit in BB, and pot size.")]
    async fn export_hands(
        &self,
        Parameters(params): Parameters<ExportHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(100) as usize;
        let format = params.format.as_deref().unwrap_or("csv");

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: params.position,
            pot_type: params.pot_type,
            villain: params.villain,
            stakes: params.stakes,
            result: params.result,
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

        let hands: Vec<_> = hands.into_iter().take(limit).collect();
        let hero = &self.hero;

        if format == "raw" {
            let raw: String = hands
                .iter()
                .map(|h| h.raw_text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n");
            Ok(CallToolResult::success(vec![Content::text(raw)]))
        } else {
            let mut csv = String::from("hand_id,timestamp,variant,betting_limit,stakes,hero_position,hero_cards,board,pot_type,hero_result,profit_bb,pot_size\n");
            for hand in &hands {
                let bb = stats::big_blind_size(hand);
                let invested = stats::hero_invested(hand, hero);
                let collected = stats::hero_collected(hand, hero);
                let profit_bb = if bb > 0.0 { (collected - invested) / bb } else { 0.0 };
                let pot_size = hand.pot.map(|p| p.amount).unwrap_or(0.0);
                let cards: String = hand.hero_cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
                let board: String = hand.board.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
                let pos = hand.hero_position.map(|p| p.to_string()).unwrap_or_default();
                let stakes = match &hand.game_type {
                    GameType::Cash { small_blind, big_blind, .. } => format!("{}/{}", small_blind, big_blind),
                    GameType::Tournament { level, small_blind, big_blind, .. } => format!("L{} {}/{}", level, small_blind, big_blind),
                };
                let pot_type = stats::classify_pot_type(hand);
                let result = match hand.result.hero_result {
                    HeroResult::Won => "won",
                    HeroResult::Lost => "lost",
                    HeroResult::Folded => "folded",
                    HeroResult::SatOut => "sat_out",
                };
                csv.push_str(&format!(
                    "{},{},{:?},{:?},{},{},{},{},{},{},{:.1},{:.2}\n",
                    hand.id, hand.timestamp, hand.variant, hand.betting_limit,
                    stakes, pos, cards, board, pot_type, result, profit_bb, pot_size
                ));
            }
            Ok(CallToolResult::success(vec![Content::text(csv)]))
        }
    }

    #[tool(description = "Get a hand formatted as a step-by-step replay with running pot and stack sizes at each action.")]
    async fn get_hand_as_replayer(
        &self,
        Parameters(params): Parameters<GetHandAsReplayerParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hand = self
            .store
            .get_hand(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to retrieve hand: {}", e)))?;

        let hand = match hand {
            Some(h) => h,
            None => return Ok(CallToolResult::success(vec![Content::text(format!("Hand {} not found", params.hand_id))])),
        };

        // Initialize player stacks
        let mut stacks: HashMap<String, f64> = hand
            .players
            .iter()
            .map(|p| (p.name.clone(), p.stack.amount))
            .collect();
        let mut pot = 0.0f64;
        let mut steps: Vec<serde_json::Value> = Vec::new();
        let mut step_num = 0u32;

        for action in &hand.actions {
            let player = &action.player;
            let (action_str, amount) = match &action.action_type {
                ActionType::PostSmallBlind { amount, .. } => ("post_sb", amount.amount),
                ActionType::PostBigBlind { amount, .. } => ("post_bb", amount.amount),
                ActionType::PostAnte { amount } => ("post_ante", amount.amount),
                ActionType::PostBlind { amount } => ("post_blind", amount.amount),
                ActionType::BringsIn { amount } => ("brings_in", amount.amount),
                ActionType::Fold => ("fold", 0.0),
                ActionType::Check => ("check", 0.0),
                ActionType::Call { amount, .. } => ("call", amount.amount),
                ActionType::Bet { amount, .. } => ("bet", amount.amount),
                ActionType::Raise { to, .. } => ("raise", to.amount),
                ActionType::UncalledBet { amount } => {
                    // Return uncalled bet to player
                    pot -= amount.amount;
                    if let Some(stack) = stacks.get_mut(player) {
                        *stack += amount.amount;
                    }
                    step_num += 1;
                    steps.push(serde_json::json!({
                        "step": step_num,
                        "street": format!("{}", action.street),
                        "player": player,
                        "action": "uncalled_bet_returned",
                        "amount": format!("{:.2}", amount.amount),
                        "pot_after": format!("{:.2}", pot),
                        "player_stack_after": format!("{:.2}", stacks.get(player).copied().unwrap_or(0.0)),
                    }));
                    continue;
                }
                ActionType::Collected { amount, .. } => {
                    // Add winnings to player stack
                    if let Some(stack) = stacks.get_mut(player) {
                        *stack += amount.amount;
                    }
                    step_num += 1;
                    steps.push(serde_json::json!({
                        "step": step_num,
                        "street": format!("{}", action.street),
                        "player": player,
                        "action": "collected",
                        "amount": format!("{:.2}", amount.amount),
                        "pot_after": format!("{:.2}", 0.0),
                        "player_stack_after": format!("{:.2}", stacks.get(player).copied().unwrap_or(0.0)),
                    }));
                    continue;
                }
                ActionType::Shows { .. } | ActionType::DoesNotShow | ActionType::Mucks => {
                    step_num += 1;
                    let desc = match &action.action_type {
                        ActionType::Shows { cards, description, .. } => {
                            let card_str: String = cards.iter().map(|c| match c { Some(c) => c.to_string(), None => "?".to_string() }).collect::<Vec<_>>().join(" ");
                            if let Some(d) = description { format!("shows {} ({})", card_str, d) } else { format!("shows {}", card_str) }
                        }
                        ActionType::DoesNotShow => "does_not_show".to_string(),
                        ActionType::Mucks => "mucks".to_string(),
                        _ => unreachable!(),
                    };
                    steps.push(serde_json::json!({
                        "step": step_num,
                        "street": format!("{}", action.street),
                        "player": player,
                        "action": desc,
                        "amount": "0.00",
                        "pot_after": format!("{:.2}", pot),
                        "player_stack_after": format!("{:.2}", stacks.get(player).copied().unwrap_or(0.0)),
                    }));
                    continue;
                }
                ActionType::SitsOut | ActionType::WaitsForBigBlind => continue,
            };

            // Deduct from player stack, add to pot
            if amount > 0.0 {
                if let Some(stack) = stacks.get_mut(player) {
                    *stack -= amount;
                }
                pot += amount;
            }

            step_num += 1;
            steps.push(serde_json::json!({
                "step": step_num,
                "street": format!("{}", action.street),
                "player": player,
                "action": action_str,
                "amount": format!("{:.2}", amount),
                "pot_after": format!("{:.2}", pot),
                "player_stack_after": format!("{:.2}", stacks.get(player).copied().unwrap_or(0.0)),
            }));
        }

        let players_info: Vec<serde_json::Value> = hand.players.iter().map(|p| {
            serde_json::json!({
                "name": p.name,
                "seat": p.seat,
                "position": p.position.map(|pos| pos.to_string()),
                "starting_stack": format!("{:.2}", p.stack.amount),
                "is_hero": p.is_hero,
            })
        }).collect();

        let cards: String = hand.hero_cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
        let board: String = hand.board.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");

        let response = serde_json::json!({
            "hand_id": hand.id,
            "variant": format!("{}", hand.variant),
            "betting_limit": format!("{}", hand.betting_limit),
            "stakes": format!("{}", hand.game_type),
            "table_name": hand.table_name,
            "timestamp": hand.timestamp,
            "players": players_info,
            "hero_cards": cards,
            "board": board,
            "steps": steps,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Generate a quiz from a hand — shows the hand up to a decision point and hides hero's action + outcome. Great for studying decision-making. The answer is included separately.")]
    async fn quiz_hand(
        &self,
        Parameters(params): Parameters<QuizHandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = &self.hero;

        let hand = if let Some(hand_id) = params.hand_id {
            self.store
                .get_hand(hand_id)
                .await
                .map_err(|e| mcp_error(&format!("Failed to retrieve hand: {}", e)))?
                .ok_or_else(|| mcp_error(&format!("Hand {} not found", hand_id)))?
        } else {
            // Find a qualifying hand
            let filter_params = SearchParams {
                query: String::new(),
                mode: search::SearchMode::default(),
                position: params.position.clone(),
                pot_type: params.pot_type.clone(),
                villain: None,
                stakes: params.stakes.clone(),
                result: None,
                game_type: None,
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

            // Find a hand where hero had a voluntary postflop action or preflop raise
            hands.into_iter().find(|h| {
                h.actions.iter().any(|a| {
                    a.player == *hero && matches!(
                        a.action_type,
                        ActionType::Bet { .. } | ActionType::Raise { .. } | ActionType::Call { .. } | ActionType::Check
                    ) && (a.street != Street::Preflop || matches!(a.action_type, ActionType::Raise { .. }))
                })
            }).ok_or_else(|| mcp_error("No qualifying hand found for quiz"))?
        };

        // Parse target street
        let target_street = params.street.as_deref().and_then(|s| match s.to_lowercase().as_str() {
            "preflop" => Some(Street::Preflop),
            "flop" => Some(Street::Flop),
            "turn" => Some(Street::Turn),
            "river" => Some(Street::River),
            _ => None,
        });

        // Find hero's last voluntary action (the decision point)
        let voluntary = |at: &ActionType| matches!(at,
            ActionType::Call { .. } | ActionType::Bet { .. } | ActionType::Raise { .. } |
            ActionType::Check | ActionType::Fold
        );

        let decision_idx = if let Some(target) = target_street {
            hand.actions.iter().rposition(|a| a.player == *hero && a.street == target && voluntary(&a.action_type))
        } else {
            hand.actions.iter().rposition(|a| a.player == *hero && voluntary(&a.action_type))
        };

        let decision_idx = decision_idx
            .ok_or_else(|| mcp_error("No hero decision found in this hand"))?;

        let decision_street = hand.actions[decision_idx].street;

        // Board cards up to the decision street
        let board_cards_at_street = match decision_street {
            Street::Preflop => 0,
            Street::Flop => 3.min(hand.board.len()),
            Street::Turn => 4.min(hand.board.len()),
            Street::River | Street::Showdown => hand.board.len(),
            _ => hand.board.len(), // stud streets: show all
        };

        // Compute pot at decision point
        let mut pot_at_decision = 0.0f64;
        let mut hero_invested = 0.0f64;
        for (i, action) in hand.actions.iter().enumerate() {
            if i >= decision_idx { break; }
            match &action.action_type {
                ActionType::PostSmallBlind { amount, .. }
                | ActionType::PostBigBlind { amount, .. }
                | ActionType::PostAnte { amount }
                | ActionType::PostBlind { amount }
                | ActionType::BringsIn { amount }
                | ActionType::Call { amount, .. }
                | ActionType::Bet { amount, .. } => {
                    pot_at_decision += amount.amount;
                    if action.player == *hero { hero_invested += amount.amount; }
                }
                ActionType::Raise { to, .. } => {
                    pot_at_decision += to.amount;
                    if action.player == *hero { hero_invested += to.amount; }
                }
                ActionType::UncalledBet { amount } => {
                    pot_at_decision -= amount.amount;
                    if action.player == *hero { hero_invested -= amount.amount; }
                }
                _ => {}
            }
        }

        // Hero stack at decision
        let hero_starting = hand.players.iter().find(|p| p.name == *hero).map(|p| p.stack.amount).unwrap_or(0.0);
        let hero_stack = hero_starting - hero_invested;

        let bb = stats::big_blind_size(&hand);

        // Actions before the decision (anonymize or keep names)
        let actions_before: Vec<serde_json::Value> = hand.actions[..decision_idx].iter().map(|a| {
            serde_json::json!({
                "street": format!("{}", a.street),
                "player": if a.player == *hero { "Hero".to_string() } else { a.player.clone() },
                "action": format!("{:?}", a.action_type),
            })
        }).collect();

        let cards: String = hand.hero_cards.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
        let board_visible: String = hand.board[..board_cards_at_street].iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");

        // Quiz portion
        let quiz = serde_json::json!({
            "hand_id": hand.id,
            "variant": format!("{}", hand.variant),
            "stakes": format!("{}", hand.game_type),
            "hero_position": hand.hero_position.map(|p| p.to_string()),
            "hero_cards": cards,
            "board": board_visible,
            "decision_street": format!("{}", decision_street),
            "pot_at_decision": format!("{:.2}", pot_at_decision),
            "pot_at_decision_bb": if bb > 0.0 { format!("{:.1}", pot_at_decision / bb) } else { "N/A".to_string() },
            "hero_stack": format!("{:.2}", hero_stack),
            "hero_stack_bb": if bb > 0.0 { format!("{:.1}", hero_stack / bb) } else { "N/A".to_string() },
            "actions_before": actions_before,
        });

        // Answer portion
        let hero_action = &hand.actions[decision_idx];
        let subsequent: Vec<serde_json::Value> = hand.actions[decision_idx+1..].iter().map(|a| {
            serde_json::json!({
                "street": format!("{}", a.street),
                "player": if a.player == *hero { "Hero".to_string() } else { a.player.clone() },
                "action": format!("{:?}", a.action_type),
            })
        }).collect();

        let full_board: String = hand.board.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ");
        let total_invested = stats::hero_invested(&hand, hero);
        let total_collected = stats::hero_collected(&hand, hero);
        let profit_bb = if bb > 0.0 { (total_collected - total_invested) / bb } else { 0.0 };

        let answer = serde_json::json!({
            "hero_action": format!("{:?}", hero_action.action_type),
            "subsequent_actions": subsequent,
            "full_board": full_board,
            "result": format!("{:?}", hand.result.hero_result),
            "profit_bb": format!("{:.1}", profit_bb),
        });

        let response = serde_json::json!({
            "quiz": quiz,
            "answer": answer,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Find villains in the database with similar stats to a target profile. Useful for finding players who play like a specific archetype (e.g. loose-aggressive, nit, etc.).")]
    async fn get_similar_villains(
        &self,
        Parameters(params): Parameters<GetSimilarVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let min_hands = params.min_hands.unwrap_or(20);
        let limit = params.limit.unwrap_or(5) as usize;

        let hands = self
            .store
            .scroll_hands(None)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let villains = stats::list_villains(&hands, &self.hero, min_hands);

        let mut scored: Vec<(f64, &stats::VillainSummary)> = villains
            .iter()
            .map(|v| {
                let mut dist = (v.vpip / 100.0 - params.vpip / 100.0).powi(2)
                    + (v.pfr / 100.0 - params.pfr / 100.0).powi(2);
                if let Some(tb) = params.three_bet {
                    dist += (v.three_bet_pct / 100.0 - tb / 100.0).powi(2);
                }
                if let Some(af) = params.aggression_factor {
                    dist += (v.aggression_factor / 5.0 - af / 5.0).powi(2);
                }
                (dist.sqrt(), v)
            })
            .collect();

        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let results: Vec<serde_json::Value> = scored.into_iter().take(limit).map(|(dist, v)| {
            serde_json::json!({
                "name": v.name,
                "hands": v.hands,
                "distance": format!("{:.3}", dist),
                "vpip": format!("{:.1}", v.vpip),
                "pfr": format!("{:.1}", v.pfr),
                "three_bet_pct": format!("{:.1}", v.three_bet_pct),
                "aggression_factor": format!("{:.2}", v.aggression_factor),
                "net_profit": format!("{:.2}", v.net_profit),
                "wwsf": format!("{:.1}", v.wwsf),
            })
        }).collect();

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Build a preflop hand chart for hero at a given position. Shows open/3bet/call/fold/limp frequencies for each starting hand combo. Hold'em only.")]
    async fn get_preflop_chart(
        &self,
        Parameters(params): Parameters<GetPreflopChartParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: Some(params.position.clone()),
            pot_type: None,
            villain: None,
            stakes: params.stakes,
            result: None,
            game_type: params.game_type,
            variant: Some("holdem".to_string()),
            betting_limit: None,
            limit: None,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        // Only holdem (already filtered, but double-check)
        let hands: Vec<_> = hands.into_iter().filter(|h| h.variant == PokerVariant::Holdem).collect();

        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({"error": "No Hold'em hands found for this position"}).to_string()
            )]));
        }

        #[derive(Default)]
        struct ComboStats {
            total: u32,
            open: u32,
            three_bet: u32,
            call: u32,
            fold: u32,
            limp: u32,
        }

        let mut combos: HashMap<String, ComboStats> = HashMap::new();
        let mut totals = ComboStats::default();

        for hand in &hands {
            if hand.hero_cards.len() != 2 { continue; }
            let label = match combo_label(&hand.hero_cards) {
                Some(l) => l,
                None => continue,
            };

            // Classify hero's preflop action
            let mut raises_before_hero = 0u32;
            let mut hero_action_type: Option<&str> = None;

            for action in &hand.actions {
                if action.street != Street::Preflop { continue; }
                if action.player == *hero {
                    match &action.action_type {
                        ActionType::Raise { .. } | ActionType::Bet { .. } => {
                            if raises_before_hero == 0 {
                                hero_action_type = Some("open");
                            } else {
                                hero_action_type = Some("three_bet");
                            }
                            break;
                        }
                        ActionType::Call { .. } => {
                            if raises_before_hero == 0 {
                                hero_action_type = Some("limp");
                            } else {
                                hero_action_type = Some("call");
                            }
                            break;
                        }
                        ActionType::Fold => {
                            hero_action_type = Some("fold");
                            break;
                        }
                        ActionType::Check => {
                            // BB checking = not a voluntary action in this context
                            hero_action_type = Some("check");
                            break;
                        }
                        _ => {}
                    }
                } else {
                    match &action.action_type {
                        ActionType::Raise { .. } | ActionType::Bet { .. } => {
                            raises_before_hero += 1;
                        }
                        _ => {}
                    }
                }
            }

            let entry = combos.entry(label).or_default();
            entry.total += 1;
            totals.total += 1;
            match hero_action_type {
                Some("open") => { entry.open += 1; totals.open += 1; }
                Some("three_bet") => { entry.three_bet += 1; totals.three_bet += 1; }
                Some("call") => { entry.call += 1; totals.call += 1; }
                Some("fold") => { entry.fold += 1; totals.fold += 1; }
                Some("limp") => { entry.limp += 1; totals.limp += 1; }
                _ => {}
            }
        }

        // Convert to percentages
        let pct = |n: u32, d: u32| -> f64 {
            if d == 0 { 0.0 } else { n as f64 / d as f64 * 100.0 }
        };

        let mut combo_results: Vec<serde_json::Value> = combos
            .iter()
            .map(|(label, cs)| {
                serde_json::json!({
                    "combo": label,
                    "total": cs.total,
                    "open_pct": format!("{:.0}", pct(cs.open, cs.total)),
                    "three_bet_pct": format!("{:.0}", pct(cs.three_bet, cs.total)),
                    "call_pct": format!("{:.0}", pct(cs.call, cs.total)),
                    "fold_pct": format!("{:.0}", pct(cs.fold, cs.total)),
                    "limp_pct": format!("{:.0}", pct(cs.limp, cs.total)),
                })
            })
            .collect();

        // Sort by open % descending (strongest hands first)
        combo_results.sort_by(|a, b| {
            let oa: f64 = a["open_pct"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            let ob: f64 = b["open_pct"].as_str().unwrap_or("0").parse().unwrap_or(0.0);
            ob.partial_cmp(&oa).unwrap_or(std::cmp::Ordering::Equal)
        });

        let response = serde_json::json!({
            "position": params.position,
            "total_hands": totals.total,
            "summary": {
                "open_pct": format!("{:.1}", pct(totals.open, totals.total)),
                "three_bet_pct": format!("{:.1}", pct(totals.three_bet, totals.total)),
                "call_pct": format!("{:.1}", pct(totals.call, totals.total)),
                "fold_pct": format!("{:.1}", pct(totals.fold, totals.total)),
                "limp_pct": format!("{:.1}", pct(totals.limp, totals.total)),
            },
            "combos": combo_results,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Re-parse and re-embed a hand from its raw text. Useful after parser improvements to update a specific hand without full reimport.")]
    async fn reimport_hand(
        &self,
        Parameters(params): Parameters<ReimportHandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hand = self
            .store
            .get_hand(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to retrieve hand: {}", e)))?;

        let hand = match hand {
            Some(h) => h,
            None => return Ok(CallToolResult::success(vec![Content::text(format!("Hand {} not found", params.hand_id))])),
        };

        let hero_name = hand.hero.as_deref().unwrap_or(&self.hero);
        let raw_text = &hand.raw_text;

        // Re-parse
        let results = parsers::parse_auto(raw_text, hero_name);
        let new_hand = results
            .into_iter()
            .find_map(|r| r.ok())
            .ok_or_else(|| mcp_error("Failed to re-parse hand from raw text"))?;

        // Re-summarize and re-encode
        let summary = summarizer::summarize(&new_hand);
        let action_enc = action_encoder::encode_action_sequence(&new_hand, hero_name);

        // Re-embed
        let mut embedder = self.embedder.lock().await;
        let vectors = embedder
            .embed_batch(&[&summary, &action_enc])
            .map_err(|e| mcp_error(&format!("Embedding failed: {}", e)))?;

        let embeddings = HandEmbeddings {
            summary: vectors[0].clone(),
            action: vectors[1].clone(),
        };

        // Upsert
        self.store
            .upsert_hand(&new_hand, &summary, &action_enc, embeddings)
            .await
            .map_err(|e| mcp_error(&format!("Upsert failed: {}", e)))?;

        let response = serde_json::json!({
            "hand_id": new_hand.id,
            "status": "reimported",
            "summary": summary,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Get database health diagnostics: total hands, variant/stakes breakdowns, date range, data quality checks, and storage size.")]
    async fn get_database_health(
        &self,
        Parameters(_params): Parameters<GetDatabaseHealthParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hands = self
            .store
            .scroll_hands(None)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let total = hands.len();
        let mut cash_count = 0u64;
        let mut tournament_count = 0u64;
        let mut variant_counts: HashMap<String, u64> = HashMap::new();
        let mut stakes_counts: HashMap<String, u64> = HashMap::new();
        let mut heroes: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut min_ts: Option<&str> = None;
        let mut max_ts: Option<&str> = None;
        let mut missing_hero = 0u64;
        let mut missing_cards = 0u64;

        for hand in &hands {
            match &hand.game_type {
                GameType::Cash { .. } => cash_count += 1,
                GameType::Tournament { .. } => tournament_count += 1,
            }
            *variant_counts.entry(format!("{}", hand.variant)).or_default() += 1;
            let stakes = match &hand.game_type {
                GameType::Cash { small_blind, big_blind, .. } => format!("{}/{}", small_blind, big_blind),
                GameType::Tournament { .. } => "tournament".to_string(),
            };
            *stakes_counts.entry(stakes).or_default() += 1;

            if let Some(ref h) = hand.hero {
                heroes.insert(h.clone());
            } else {
                missing_hero += 1;
            }
            if hand.hero_cards.is_empty() {
                missing_cards += 1;
            }

            let ts = hand.timestamp.as_str();
            min_ts = Some(match min_ts { Some(m) if m < ts => m, _ => ts });
            max_ts = Some(match max_ts { Some(m) if m > ts => m, _ => ts });
        }

        // Calculate storage size
        let data_dir = config::data_dir();
        let storage_bytes = dir_size(&data_dir);
        let storage_mb = storage_bytes as f64 / (1024.0 * 1024.0);

        let response = serde_json::json!({
            "total_hands": total,
            "cash_hands": cash_count,
            "tournament_hands": tournament_count,
            "variants": variant_counts,
            "stakes": stakes_counts,
            "date_range": {
                "earliest": min_ts.unwrap_or("N/A"),
                "latest": max_ts.unwrap_or("N/A"),
            },
            "heroes": heroes.into_iter().collect::<Vec<_>>(),
            "data_quality": {
                "hands_missing_hero": missing_hero,
                "hands_missing_cards": missing_cards,
            },
            "storage_mb": format!("{:.1}", storage_mb),
        });

        let json = serde_json::to_string_pretty(&response)
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
                 search_similar_hands to find structurally similar hands by ID, \
                 get_table_profitability to see profit by stakes or table, \
                 get_best_villains / get_worst_villains for opponent profitability, \
                 watch_directory to import new hand histories, \
                 get_last_import for import status, \
                 auto_tag_hands to classify hands by archetype (cooler, hero call, bluff, big win/loss), \
                 get_coolers to find showdown hands where hero invested heavily and lost, \
                 export_hands to export hands as CSV or raw text, \
                 get_hand_as_replayer for step-by-step hand replay with running pot/stacks, \
                 quiz_hand to generate a decision quiz from a hand, \
                 get_similar_villains to find opponents matching a stat profile, \
                 get_preflop_chart to build a preflop hand chart by position (Hold'em only), \
                 reimport_hand to re-parse and re-embed a specific hand, \
                 and get_database_health for database diagnostics."
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

fn rank_order(rank: Rank) -> u8 {
    match rank {
        Rank::Two => 2,
        Rank::Three => 3,
        Rank::Four => 4,
        Rank::Five => 5,
        Rank::Six => 6,
        Rank::Seven => 7,
        Rank::Eight => 8,
        Rank::Nine => 9,
        Rank::Ten => 10,
        Rank::Jack => 11,
        Rank::Queen => 12,
        Rank::King => 13,
        Rank::Ace => 14,
    }
}

fn combo_label(cards: &[Card]) -> Option<String> {
    if cards.len() != 2 { return None; }
    let (c1, c2) = (&cards[0], &cards[1]);
    let r1 = rank_order(c1.rank);
    let r2 = rank_order(c2.rank);
    let (high, low) = if r1 >= r2 { (c1, c2) } else { (c2, c1) };
    if high.rank == low.rank {
        Some(format!("{}{}", high.rank, low.rank))
    } else if high.suit == low.suit {
        Some(format!("{}{}s", high.rank, low.rank))
    } else {
        Some(format!("{}{}o", high.rank, low.rank))
    }
}

fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = entry.file_type();
            if let Ok(ft) = ft {
                if ft.is_file() {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                } else if ft.is_dir() {
                    total += dir_size(&entry.path());
                }
            }
        }
    }
    total
}
