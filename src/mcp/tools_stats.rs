use std::collections::HashMap;

use rmcp::model::*;

use crate::search::{self, SearchParams};
use crate::stats;

use super::helpers::mcp_error;
use super::params::{
    BestVillainsParams, CompareStatsParams, GetPoolStatsParams, GetStatsParams,
    ListVillainsParams, WorstVillainsParams,
};
use super::PokerVectorMcp;

impl PokerVectorMcp {
    pub(crate) async fn tool_get_stats(
        &self,
        params: GetStatsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);

        // Build a filter from optional params
        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: params.position,
            pot_type: params.pot_type,
            villain: params.villain.clone(),
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

        let player_stats = stats::calculate_stats(&hands, hero);

        // If villain filter was used, add positional breakdown for the matchup
        let response = if let Some(ref villain) = params.villain {
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
                .map(|(pos, (count, pbb))| {
                    serde_json::json!({
                        "position": pos,
                        "hands": count,
                        "hero_profit_bb": format!("{:.1}", pbb),
                        "hero_bb_per_100": format!("{:.1}", if *count > 0 { pbb / *count as f64 * 100.0 } else { 0.0 }),
                    })
                })
                .collect();

            serde_json::json!({
                "stats": player_stats,
                "villain_matchup": {
                    "villain": villain,
                    "positional_breakdown": positional,
                },
            })
        } else {
            serde_json::to_value(&player_stats)
                .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?
        };

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_list_villains(
        &self,
        params: ListVillainsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let min_hands = params.min_hands.unwrap_or(10);

        // Build date filter if provided
        let filter = if params.from_date.is_some() || params.to_date.is_some() {
            let filter_params = SearchParams {
                query: String::new(),
                mode: search::SearchMode::default(),
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
                from_date: params.from_date,
                to_date: params.to_date,
                tag: None,
            };
            search::build_filter(&filter_params)
        } else {
            None
        };

        let hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let villains = stats::list_villains(&hands, hero, min_hands);
        let json = serde_json::to_string_pretty(&villains)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_best_villains(
        &self,
        params: BestVillainsParams,
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

    pub(crate) async fn tool_get_worst_villains(
        &self,
        params: WorstVillainsParams,
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

    pub(crate) async fn tool_compare_stats(
        &self,
        params: CompareStatsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let player_a = params.player_a.as_deref().unwrap_or(&self.hero);
        let player_b = &params.player_b;

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

        let stats_a = stats::calculate_stats(&hands, player_a);
        let stats_b = stats::calculate_stats(&hands, player_b);

        let response = serde_json::json!({
            "player_a": { "name": player_a, "stats": stats_a },
            "player_b": { "name": player_b, "stats": stats_b },
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_pool_stats(
        &self,
        params: GetPoolStatsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let min_hands = params.min_hands.unwrap_or(30);

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

        let pool_stats = stats::calculate_pool_stats(&hands, hero, min_hands);

        let json = serde_json::to_string_pretty(&pool_stats)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
