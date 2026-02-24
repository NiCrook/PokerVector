pub mod params;
mod analysis;
mod helpers;
mod tools_export;
mod tools_hands;
mod tools_search;
mod tools_sessions;
mod tools_stats;
mod tools_spots;
mod tools_villains;

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::*,
    tool, tool_handler, tool_router, ServerHandler,
};
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
use crate::types::{ActionType, GameType, HeroResult, PokerVariant, Street};

use helpers::{mcp_error, dir_size, combo_label};
use params::*;

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

// Tool implementations

#[tool_router]
impl PokerVectorMcp {
    #[tool(description = "Search poker hand histories using natural language with optional filters. Returns matching hands ranked by relevance.")]
    async fn search_hands(
        &self,
        Parameters(params): Parameters<SearchHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_search_hands(params).await
    }

    #[tool(description = "Fetch full details of a specific hand by its numeric ID.")]
    async fn get_hand(
        &self,
        Parameters(params): Parameters<GetHandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_hand(params).await
    }

    #[tool(description = "Get aggregate player statistics (VPIP, PFR, 3-bet%, etc.) with optional filters. Computes stats across all matching hands.")]
    async fn get_stats(
        &self,
        Parameters(params): Parameters<GetStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_stats(params).await
    }

    #[tool(description = "List tracked opponents with hand counts and key stats (VPIP, PFR, aggression, etc.).")]
    async fn list_villains(
        &self,
        Parameters(params): Parameters<ListVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_list_villains(params).await
    }

    #[tool(description = "List detected cash game sessions. Groups hands by table and play period. Sessions are separated by 30+ minutes of inactivity across all tables.")]
    async fn list_sessions(
        &self,
        Parameters(params): Parameters<ListSessionsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_list_sessions(params).await
    }

    #[tool(description = "Get a detailed review of a specific cash game session. Returns aggregate stats, per-table breakdown, and notable hands (biggest wins/losses).")]
    async fn review_session(
        &self,
        Parameters(params): Parameters<ReviewSessionParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_review_session(params).await
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
            offset: None,
            from_date: None,
            to_date: None,
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
        self.tool_get_best_villains(params).await
    }

    #[tool(description = "List villains hero loses the most against. Returns opponents sorted by hero's net profit ascending (biggest losers first).")]
    async fn get_worst_villains(
        &self,
        Parameters(params): Parameters<WorstVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_worst_villains(params).await
    }

    #[tool(description = "Find hands with similar betting structure or narrative to a given hand ID. Default mode is 'action' (betting pattern similarity).")]
    async fn search_similar_hands(
        &self,
        Parameters(params): Parameters<SearchSimilarParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_search_similar_hands(params).await
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
        self.tool_auto_tag_hands(params).await
    }

    #[tool(description = "Find cooler hands — showdown hands where hero invested heavily and lost. Sorted by pot size descending.")]
    async fn get_coolers(
        &self,
        Parameters(params): Parameters<GetCoolersParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_coolers(params).await
    }

    #[tool(description = "Export hands as CSV or raw hand history text with optional filters. CSV includes hand_id, timestamp, variant, stakes, hero position, cards, board, pot type, result, profit in BB, and pot size.")]
    async fn export_hands(
        &self,
        Parameters(params): Parameters<ExportHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_export_hands(params).await
    }

    #[tool(description = "Get a hand formatted as a step-by-step replay with running pot and stack sizes at each action.")]
    async fn get_hand_as_replayer(
        &self,
        Parameters(params): Parameters<GetHandAsReplayerParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_hand_as_replayer(params).await
    }

    #[tool(description = "Generate a quiz from a hand — shows the hand up to a decision point and hides hero's action + outcome. Great for studying decision-making. The answer is included separately.")]
    async fn quiz_hand(
        &self,
        Parameters(params): Parameters<QuizHandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_quiz_hand(params).await
    }

    #[tool(description = "Find villains in the database with similar stats to a target profile. Useful for finding players who play like a specific archetype (e.g. loose-aggressive, nit, etc.).")]
    async fn get_similar_villains(
        &self,
        Parameters(params): Parameters<GetSimilarVillainsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_similar_villains(params).await
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
            offset: None,
            from_date: None,
            to_date: None,
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

    #[tool(description = "Return the original raw hand history text for a hand ID. Useful for copy-pasting into forums, solvers, or review tools.")]
    async fn get_hand_history(
        &self,
        Parameters(params): Parameters<GetHandHistoryParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_hand_history(params).await
    }

    #[tool(description = "Compare stats side-by-side for two players. Returns both stat profiles in a single response for easy comparison.")]
    async fn compare_stats(
        &self,
        Parameters(params): Parameters<CompareStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_compare_stats(params).await
    }

    #[tool(description = "Count hands matching filters without returning hand data. Fast, lightweight way to check how many hands match specific criteria.")]
    async fn count_hands(
        &self,
        Parameters(params): Parameters<CountHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_count_hands(params).await
    }

    #[tool(description = "Find hands where a specific villain went to showdown and revealed their holdings. Returns villain's cards, board, and outcome for each hand.")]
    async fn get_showdown_hands(
        &self,
        Parameters(params): Parameters<GetShowdownHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_showdown_hands(params).await
    }

    #[tool(description = "Get the surrounding hands from the same table as a given hand ID. Returns hands before and after for understanding table dynamics and momentum.")]
    async fn get_hand_context(
        &self,
        Parameters(params): Parameters<GetHandContextParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_hand_context(params).await
    }

    #[tool(description = "Power-user tool: query hands with a raw SQL WHERE clause against hand metadata columns. The LLM can construct arbitrary filters beyond what other tools support.")]
    async fn query_hands(
        &self,
        Parameters(params): Parameters<QueryHandsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_query_hands(params).await
    }

    #[tool(description = "Find hands where hero was all-in. Returns holdings, board, pot odds, and outcome for each all-in spot.")]
    async fn get_equity_spots(
        &self,
        Parameters(params): Parameters<GetEquitySpotsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_equity_spots(params).await
    }

    #[tool(description = "Get hero stats filtered to multiway pots (3+ players seeing the flop). Multiway play often differs drastically from heads-up.")]
    async fn get_multiway_stats(
        &self,
        Parameters(params): Parameters<GetMultiwayStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_multiway_stats(params).await
    }

    #[tool(description = "Get running profit/loss data points over time. Returns cumulative profit at each hand, suitable for graphing or trend analysis.")]
    async fn get_bankroll_graph(
        &self,
        Parameters(params): Parameters<GetBankrollGraphParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_bankroll_graph(params).await
    }

    #[tool(description = "Find hands where hero was in a squeeze-eligible spot (raise + cold call in front of hero preflop). Shows what hero did and the outcome.")]
    async fn get_squeeze_spots(
        &self,
        Parameters(params): Parameters<GetSqueezeSpotsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_squeeze_spots(params).await
    }

    #[tool(description = "Comprehensive villain report: stats, showdown hands, positional breakdown, and profit summary in one call. Saves orchestrating multiple tool calls.")]
    async fn get_villain_profile(
        &self,
        Parameters(params): Parameters<GetVillainProfileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_villain_profile(params).await
    }

    #[tool(description = "Hero vs villain broken down by hero position. Shows hands played and hero profit at each position to identify where hero has an edge or is exploited.")]
    async fn get_positional_matchups(
        &self,
        Parameters(params): Parameters<GetPositionalMatchupsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_positional_matchups(params).await
    }

    #[tool(description = "Automated leak detection. Compares hero's stats against healthy baseline ranges for 6-max or full-ring and flags potential leaks. Each leak includes the stat name, hero's value, the healthy range, a severity (minor/moderate/major), and an explanation.")]
    async fn find_leaks(
        &self,
        Parameters(params): Parameters<FindLeaksParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let table_size = params.table_size.as_deref().unwrap_or("6max");

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: params.stakes,
            result: None,
            game_type: params.game_type,
            variant: params.variant,
            betting_limit: params.betting_limit,
            limit: None,
            offset: None,
            from_date: params.from_date,
            to_date: params.to_date,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "leaks": [],
                    "total_hands": 0,
                    "message": "No hands found matching filters.",
                }))
                .unwrap(),
            )]));
        }

        let s = stats::calculate_stats(&hands, hero);

        if s.hands_played < 100 {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "leaks": [],
                    "total_hands": s.hands_played,
                    "message": "Not enough hands for reliable leak detection (need 100+).",
                    "stats": s,
                }))
                .unwrap(),
            )]));
        }

        let response = analysis::find_leaks_analysis(&s, table_size);
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Show the distribution of starting hands hero played from a given position. Groups by hand category (pocket pairs, suited broadways, offsuit broadways, suited connectors, etc.) and shows open/3bet/call/fold frequencies per combo. Hold'em only.")]
    async fn get_range_analysis(
        &self,
        Parameters(params): Parameters<GetRangeAnalysisParams>,
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
            variant: params.variant.or_else(|| Some("holdem".to_string())),
            betting_limit: None,
            limit: None,
            offset: None,
            from_date: params.from_date,
            to_date: params.to_date,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "position": params.position,
                    "total_hands": 0,
                    "message": "No hands found matching filters.",
                }))
                .unwrap(),
            )]));
        }

        let response = analysis::get_range_analysis_data(&hands, hero, &params.position);
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Analyze hero's performance on different board textures. Classifies flops by texture (monotone, two-tone, rainbow, paired, connected, high, low, dry, wet) and shows hands played, winrate, and c-bet frequency for each. Answers 'how do I perform on wet boards?' or 'should I c-bet more on dry flops?'")]
    async fn get_board_stats(
        &self,
        Parameters(params): Parameters<GetBoardStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: params.position,
            pot_type: None,
            villain: None,
            stakes: params.stakes,
            result: None,
            game_type: params.game_type,
            variant: params.variant,
            betting_limit: None,
            limit: None,
            offset: None,
            from_date: params.from_date,
            to_date: params.to_date,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "textures": [],
                    "total_hands": 0,
                    "message": "No hands found matching filters.",
                }))
                .unwrap(),
            )]));
        }

        let response = analysis::get_board_stats_analysis(&hands, hero);
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Analyze how a villain reacts to specific betting lines. Shows action-reaction patterns: how villain responds to c-bets, barrels, checks, and probes on each street. Answers questions like 'when I c-bet flop and villain calls, what does villain do facing a turn barrel?'")]
    async fn get_villain_tendencies(
        &self,
        Parameters(params): Parameters<GetVillainTendenciesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.tool_get_villain_tendencies(params).await
    }

    #[tool(description = "Analyze bet sizing patterns for a player by street. Shows distribution of bet/raise sizes as fractions of the pot (e.g. 25%, 33%, 50%, 66%, 75%, pot, overbet). Reveals sizing tells — e.g. 'villain bets small with draws and big with value'.")]
    async fn get_sizing_profile(
        &self,
        Parameters(params): Parameters<GetSizingProfileParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let player = params.player.as_deref().unwrap_or(&self.hero);

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: params.villain,
            stakes: params.stakes,
            result: None,
            game_type: params.game_type,
            variant: params.variant,
            betting_limit: None,
            limit: None,
            offset: None,
            from_date: params.from_date,
            to_date: params.to_date,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "streets": {},
                    "total_hands": 0,
                    "message": "No hands found matching filters.",
                }))
                .unwrap(),
            )]));
        }

        let response = analysis::get_sizing_profile_analysis(&hands, player);
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Per-street action frequencies for a player. Shows bet/raise/call/check/fold counts and percentages on each street (flop, turn, river). Goes deeper than aggregate stats to answer 'how often does villain fold to turn barrels?' or 'what is hero's river aggression?'")]
    async fn get_street_stats(
        &self,
        Parameters(params): Parameters<GetStreetStatsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let player = params.player.as_deref().unwrap_or(&self.hero);

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: params.villain,
            stakes: params.stakes,
            result: None,
            game_type: params.game_type,
            variant: params.variant,
            betting_limit: params.betting_limit,
            limit: None,
            offset: None,
            from_date: params.from_date,
            to_date: params.to_date,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "streets": {},
                    "total_hands": 0,
                    "message": "No hands found matching filters.",
                }))
                .unwrap(),
            )]));
        }

        let response = analysis::get_street_stats_analysis(&hands, player);
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Detect sessions where hero's play deviated significantly from their baseline. Flags potential tilt — VPIP spikes, aggression changes, or unusual loss patterns after big hands. Compares per-session stats against overall averages.")]
    async fn detect_tilt(
        &self,
        Parameters(params): Parameters<DetectTiltParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let threshold = params.threshold.unwrap_or(10.0);
        let min_hands = params.min_hands.unwrap_or(20) as usize;

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: params.stakes,
            result: None,
            game_type: params.game_type,
            variant: None,
            betting_limit: None,
            limit: None,
            offset: None,
            from_date: None,
            to_date: None,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "tilt_sessions": [],
                    "total_sessions": 0,
                    "message": "No hands found.",
                }))
                .unwrap(),
            )]));
        }

        let response = analysis::detect_tilt_analysis(hands, hero, threshold, min_hands);
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    #[tool(description = "Show how hero's stats change over time. Buckets hands by day, week, or month and computes key stats (VPIP, PFR, winrate, hands played, etc.) for each period. Useful for answering 'am I improving?' or 'how has my 3-bet% changed?'")]
    async fn get_trends(
        &self,
        Parameters(params): Parameters<GetTrendsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let period = params.period.as_deref().unwrap_or("week");

        if !matches!(period, "day" | "week" | "month") {
            return Err(mcp_error("Invalid period: must be 'day', 'week', or 'month'"));
        }

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: params.stakes,
            result: None,
            game_type: params.game_type,
            variant: params.variant,
            betting_limit: params.betting_limit,
            limit: None,
            offset: None,
            from_date: params.from_date,
            to_date: params.to_date,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&serde_json::json!({
                    "periods": [],
                    "total_hands": 0,
                }))
                .unwrap(),
            )]));
        }

        let response = analysis::get_trends_analysis(hands, hero, period);
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
                 get_hand_history to retrieve raw hand history text, \
                 compare_stats for side-by-side player stat comparison, \
                 count_hands for fast filtered hand counts, \
                 get_showdown_hands to see villain holdings at showdown, \
                 get_hand_context for surrounding table hands, \
                 query_hands for raw SQL WHERE filters, \
                 get_equity_spots for all-in hands with holdings and pot odds, \
                 get_multiway_stats for stats in multiway pots, \
                 get_bankroll_graph for cumulative profit over time, \
                 get_squeeze_spots for squeeze-eligible preflop situations, \
                 get_villain_profile for comprehensive single-villain reports, \
                 get_positional_matchups for hero vs villain by position, \
                 get_trends for stats over time (by day/week/month), \
                 find_leaks for automated leak detection against baseline ranges, \
                 detect_tilt to find sessions where play deviated from baseline, \
                 get_street_stats for per-street action frequencies (flop/turn/river), \
                 get_sizing_profile for bet sizing distribution analysis by street, \
                 get_villain_tendencies for action-reaction patterns against a specific villain, \
                 get_range_analysis for starting hand distribution by position, \
                 get_board_stats for hero performance by board texture (monotone/paired/wet/dry/etc.), \
                 and get_database_health for database diagnostics."
                    .to_string(),
            ),
        }
    }
}
