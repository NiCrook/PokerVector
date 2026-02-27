use std::collections::HashMap;

use rmcp::model::*;

use crate::search::{self, SearchParams};
use crate::stats;
use crate::types::{ActionType, HeroResult};

use super::analysis;
use super::helpers::mcp_error;
use super::params::{
    GetPositionalMatchupsParams, GetShowdownHandsParams, GetSimilarVillainsParams,
    GetVillainProfileParams, GetVillainTendenciesParams,
};
use super::PokerVectorMcp;

impl PokerVectorMcp {
    pub(crate) async fn tool_get_similar_villains(
        &self,
        params: GetSimilarVillainsParams,
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
        let results: Vec<serde_json::Value> = scored
            .into_iter()
            .take(limit)
            .map(|(dist, v)| {
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
            })
            .collect();

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_showdown_hands(
        &self,
        params: GetShowdownHandsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(20) as usize;
        let villain = &params.villain;

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: Some(villain.clone()),
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

        let mut results: Vec<serde_json::Value> = Vec::new();
        for hand in &hands {
            if results.len() >= limit {
                break;
            }

            // Find villain's Shows action
            let shown = hand.actions.iter().find(|a| {
                a.player == *villain && matches!(a.action_type, ActionType::Shows { .. })
            });

            if let Some(action) = shown {
                if let ActionType::Shows {
                    cards, description, ..
                } = &action.action_type
                {
                    let card_str: String = cards
                        .iter()
                        .map(|c| match c {
                            Some(c) => c.to_string(),
                            None => "?".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let board: String = hand
                        .board
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let hero_cards: String = hand
                        .hero_cards
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    let bb = stats::big_blind_size(hand);
                    let profit =
                        stats::hero_collected(hand, &self.hero) - stats::hero_invested(hand, &self.hero);
                    let profit_bb = if bb > 0.0 { profit / bb } else { 0.0 };

                    results.push(serde_json::json!({
                        "hand_id": hand.id,
                        "stakes": format!("{}", hand.game_type),
                        "villain_cards": card_str,
                        "villain_hand_description": description,
                        "hero_cards": hero_cards,
                        "board": board,
                        "hero_result": format!("{:?}", hand.result.hero_result),
                        "hero_profit_bb": format!("{:.1}", profit_bb),
                        "timestamp": hand.timestamp,
                    }));
                }
            }
        }

        let response = serde_json::json!({
            "villain": villain,
            "showdown_hands": results.len(),
            "hands": results,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_villain_profile(
        &self,
        params: GetVillainProfileParams,
    ) -> Result<CallToolResult, ErrorData> {
        let villain = &params.villain;
        let hero = &self.hero;

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: Some(villain.clone()),
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

        // Villain stats
        let villain_stats = stats::calculate_stats(&hands, villain);

        // Showdown hands (up to 10)
        let mut showdowns: Vec<serde_json::Value> = Vec::new();
        for hand in &hands {
            if showdowns.len() >= 10 {
                break;
            }
            if let Some(action) = hand.actions.iter().find(|a| {
                a.player == *villain && matches!(a.action_type, ActionType::Shows { .. })
            }) {
                if let ActionType::Shows {
                    cards, description, ..
                } = &action.action_type
                {
                    let cs: String = cards
                        .iter()
                        .map(|c| match c {
                            Some(c) => c.to_string(),
                            None => "?".to_string(),
                        })
                        .collect::<Vec<_>>()
                        .join(" ");
                    let board: String = hand
                        .board
                        .iter()
                        .map(|c| c.to_string())
                        .collect::<Vec<_>>()
                        .join(" ");
                    showdowns.push(serde_json::json!({
                        "hand_id": hand.id,
                        "villain_cards": cs,
                        "description": description,
                        "board": board,
                        "pot_type": stats::classify_pot_type(hand),
                    }));
                }
            }
        }

        // Positional breakdown: hero profit vs this villain by hero position
        let mut pos_data: HashMap<String, (u64, f64)> = HashMap::new();
        for hand in &hands {
            let pos = hand
                .hero_position
                .map(|p| p.to_string())
                .unwrap_or_else(|| "?".to_string());
            let bb = stats::big_blind_size(hand);
            let profit = stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
            let profit_bb = if bb > 0.0 { profit / bb } else { 0.0 };
            let entry = pos_data.entry(pos).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += profit_bb;
        }
        let positional: Vec<serde_json::Value> = pos_data
            .iter()
            .map(|(pos, (count, profit_bb))| {
                serde_json::json!({
                    "position": pos,
                    "hands": count,
                    "hero_profit_bb": format!("{:.1}", profit_bb),
                    "hero_bb_per_100": format!("{:.1}", if *count > 0 { profit_bb / *count as f64 * 100.0 } else { 0.0 }),
                })
            })
            .collect();

        // Overall profit
        let mut total_profit_bb = 0.0f64;
        for hand in &hands {
            let bb = stats::big_blind_size(hand);
            let profit = stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
            if bb > 0.0 {
                total_profit_bb += profit / bb;
            }
        }

        let mut response = serde_json::json!({
            "villain": villain,
            "total_hands": hands.len(),
            "hero_profit_bb": format!("{:.1}", total_profit_bb),
            "stats": villain_stats,
            "showdown_hands": showdowns,
            "positional_breakdown": positional,
        });

        // Optionally include pool stats for comparison
        if params.compare_to_pool.unwrap_or(false) {
            let pool_min_hands = params.pool_min_hands.unwrap_or(30);
            // Need all hands (not just villain-filtered) for pool stats
            let all_hands = self
                .store
                .scroll_hands(None)
                .await
                .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;
            let pool = crate::stats::calculate_pool_stats(&all_hands, hero, pool_min_hands);
            response.as_object_mut().unwrap().insert(
                "pool_comparison".to_string(),
                serde_json::to_value(&pool)
                    .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?,
            );
        }

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_positional_matchups(
        &self,
        params: GetPositionalMatchupsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let villain = &params.villain;
        let hero = &self.hero;

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: Some(villain.clone()),
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

        // Group by hero position
        let mut by_pos: HashMap<String, Vec<&crate::types::Hand>> = HashMap::new();
        for hand in &hands {
            let pos = hand
                .hero_position
                .map(|p| p.to_string())
                .unwrap_or_else(|| "?".to_string());
            by_pos.entry(pos).or_default().push(hand);
        }

        let mut positions: Vec<serde_json::Value> = by_pos
            .iter()
            .map(|(pos, group)| {
                let count = group.len() as u64;
                let mut profit_bb = 0.0f64;
                let mut won = 0u64;
                let mut lost = 0u64;
                for hand in group {
                    let bb = stats::big_blind_size(hand);
                    let profit =
                        stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
                    if bb > 0.0 {
                        profit_bb += profit / bb;
                    }
                    match hand.result.hero_result {
                        HeroResult::Won => won += 1,
                        HeroResult::Lost => lost += 1,
                        _ => {}
                    }
                }
                serde_json::json!({
                    "hero_position": pos,
                    "hands": count,
                    "won": won,
                    "lost": lost,
                    "hero_profit_bb": format!("{:.1}", profit_bb),
                    "hero_bb_per_100": format!("{:.1}", if count > 0 { profit_bb / count as f64 * 100.0 } else { 0.0 }),
                })
            })
            .collect();

        positions.sort_by(|a, b| {
            let pa: f64 = a["hero_profit_bb"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0.0);
            let pb: f64 = b["hero_profit_bb"]
                .as_str()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0.0);
            pb.partial_cmp(&pa).unwrap_or(std::cmp::Ordering::Equal)
        });

        let response = serde_json::json!({
            "villain": villain,
            "total_hands": hands.len(),
            "positions": positions,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_villain_tendencies(
        &self,
        params: GetVillainTendenciesParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let villain = &params.villain;

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: Some(villain.clone()),
            stakes: params.stakes,
            result: None,
            game_type: params.game_type,
            variant: params.variant,
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
                    "villain": villain,
                    "total_hands": 0,
                    "message": "No hands found with this villain.",
                }))
                .unwrap(),
            )]));
        }

        let response = analysis::get_villain_tendencies_analysis(&hands, hero, villain);
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
