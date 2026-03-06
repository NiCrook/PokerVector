use std::collections::HashMap;

use rmcp::model::*;

use crate::search::{self, SearchParams};
use crate::stats;
use crate::types::{ActionType, PokerVariant, Street};

use super::analysis;
use super::helpers::{combo_label, mcp_error};
use super::params::{
    DetectTiltParams, FindLeaksParams, GetBoardStatsParams, GetPreflopChartParams,
    GetRangeAnalysisParams, GetSizingProfileParams, GetStreetStatsParams, GetTrendsParams,
    TableProfitabilityParams,
};
use super::PokerVectorMcp;

impl PokerVectorMcp {
    pub(crate) async fn tool_find_leaks(
        &self,
        params: FindLeaksParams,
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
            tag: None,
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

    pub(crate) async fn tool_detect_tilt(
        &self,
        params: DetectTiltParams,
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
            tag: None,
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

    pub(crate) async fn tool_get_trends(
        &self,
        params: GetTrendsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let period = params.period.as_deref().unwrap_or("week");

        if !matches!(period, "day" | "week" | "month") {
            return Err(mcp_error(
                "Invalid period: must be 'day', 'week', or 'month'",
            ));
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
            tag: None,
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

    pub(crate) async fn tool_get_street_stats(
        &self,
        params: GetStreetStatsParams,
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
            tag: None,
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

    pub(crate) async fn tool_get_sizing_profile(
        &self,
        params: GetSizingProfileParams,
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
            tag: None,
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

    pub(crate) async fn tool_get_board_stats(
        &self,
        params: GetBoardStatsParams,
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
            tag: None,
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

    pub(crate) async fn tool_get_range_analysis(
        &self,
        params: GetRangeAnalysisParams,
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
            tag: None,
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

    pub(crate) async fn tool_get_preflop_chart(
        &self,
        params: GetPreflopChartParams,
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
            tag: None,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        // Only holdem (already filtered, but double-check)
        let hands: Vec<_> = hands
            .into_iter()
            .filter(|h| h.variant == PokerVariant::Holdem)
            .collect();

        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({"error": "No Hold'em hands found for this position"}).to_string(),
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
            if hand.hero_cards.len() != 2 {
                continue;
            }
            let label = match combo_label(&hand.hero_cards) {
                Some(l) => l,
                None => continue,
            };

            // Classify hero's preflop action
            let mut raises_before_hero = 0u32;
            let mut hero_action_type: Option<&str> = None;

            for action in &hand.actions {
                if action.street != Street::Preflop {
                    continue;
                }
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
                Some("open") => {
                    entry.open += 1;
                    totals.open += 1;
                }
                Some("three_bet") => {
                    entry.three_bet += 1;
                    totals.three_bet += 1;
                }
                Some("call") => {
                    entry.call += 1;
                    totals.call += 1;
                }
                Some("fold") => {
                    entry.fold += 1;
                    totals.fold += 1;
                }
                Some("limp") => {
                    entry.limp += 1;
                    totals.limp += 1;
                }
                _ => {}
            }
        }

        // Convert to percentages
        let pct = |n: u32, d: u32| -> f64 {
            if d == 0 {
                0.0
            } else {
                n as f64 / d as f64 * 100.0
            }
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
            let oa: f64 = a["open_pct"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0.0);
            let ob: f64 = b["open_pct"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0.0);
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

    pub(crate) async fn tool_get_table_profitability(
        &self,
        params: TableProfitabilityParams,
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
            tag: None,
        };
        let filter = search::build_filter(&filter_params);

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        // Group hands
        let mut groups: HashMap<String, Vec<&crate::types::Hand>> = HashMap::new();
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
            let pa: f64 = a["net_profit"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0.0);
            let pb: f64 = b["net_profit"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0.0);
            pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
        });

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
