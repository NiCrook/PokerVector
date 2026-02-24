use rmcp::model::*;

use crate::search::{self, SearchParams};
use crate::stats;
use crate::types::{GameType, HeroResult};

use super::helpers::mcp_error;
use super::params::{
    CountHandsParams, ExportHandsParams, GetBankrollGraphParams, QueryHandsParams,
};
use super::PokerVectorMcp;

impl PokerVectorMcp {
    pub(crate) async fn tool_export_hands(
        &self,
        params: ExportHandsParams,
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
                let profit_bb = if bb > 0.0 {
                    (collected - invested) / bb
                } else {
                    0.0
                };
                let pot_size = hand.pot.map(|p| p.amount).unwrap_or(0.0);
                let cards: String = hand
                    .hero_cards
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                let board: String = hand
                    .board
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                let pos = hand
                    .hero_position
                    .map(|p| p.to_string())
                    .unwrap_or_default();
                let stakes = match &hand.game_type {
                    GameType::Cash {
                        small_blind,
                        big_blind,
                        ..
                    } => format!("{}/{}", small_blind, big_blind),
                    GameType::Tournament {
                        level,
                        small_blind,
                        big_blind,
                        ..
                    } => format!("L{} {}/{}", level, small_blind, big_blind),
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
                    hand.id,
                    hand.timestamp,
                    hand.variant,
                    hand.betting_limit,
                    stakes,
                    pos,
                    cards,
                    board,
                    pot_type,
                    result,
                    profit_bb,
                    pot_size
                ));
            }
            Ok(CallToolResult::success(vec![Content::text(csv)]))
        }
    }

    pub(crate) async fn tool_count_hands(
        &self,
        params: CountHandsParams,
    ) -> Result<CallToolResult, ErrorData> {
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
            offset: None,
            from_date: None,
            to_date: None,
        };
        let filter = search::build_filter(&filter_params);

        let count = self
            .store
            .count_filtered(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to count hands: {}", e)))?;

        let response = serde_json::json!({ "count": count });
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_query_hands(
        &self,
        params: QueryHandsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(50) as usize;

        let hands = self
            .store
            .scroll_hands(Some(params.filter))
            .await
            .map_err(|e| mcp_error(&format!("Query failed: {}", e)))?;

        let hands: Vec<_> = hands.into_iter().take(limit).collect();
        let hero = &self.hero;

        let results: Vec<serde_json::Value> = hands
            .iter()
            .map(|h| {
                let bb = stats::big_blind_size(h);
                let profit = stats::hero_collected(h, hero) - stats::hero_invested(h, hero);
                let cards: String = h
                    .hero_cards
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                let board: String = h
                    .board
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                serde_json::json!({
                    "hand_id": h.id,
                    "timestamp": h.timestamp,
                    "variant": format!("{}", h.variant),
                    "stakes": format!("{}", h.game_type),
                    "hero_position": h.hero_position.map(|p| p.to_string()),
                    "hero_cards": cards,
                    "board": board,
                    "hero_result": format!("{:?}", h.result.hero_result),
                    "profit_bb": format!("{:.1}", if bb > 0.0 { profit / bb } else { 0.0 }),
                    "pot_type": stats::classify_pot_type(h),
                })
            })
            .collect();

        let response = serde_json::json!({
            "total_matching": results.len(),
            "hands": results,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_bankroll_graph(
        &self,
        params: GetBankrollGraphParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = &self.hero;

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

        let mut hands = self
            .store
            .scroll_hands(filter)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        hands.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        let mut cumulative = 0.0f64;
        let mut cumulative_bb = 0.0f64;
        let mut points: Vec<serde_json::Value> = Vec::new();

        for (i, hand) in hands.iter().enumerate() {
            let bb = stats::big_blind_size(hand);
            let profit = stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
            cumulative += profit;
            if bb > 0.0 {
                cumulative_bb += profit / bb;
            }

            // Emit every hand for small datasets, sample for large ones
            let emit = hands.len() <= 500
                || i % (hands.len() / 500).max(1) == 0
                || i == hands.len() - 1;
            if emit {
                points.push(serde_json::json!({
                    "hand_number": i + 1,
                    "timestamp": hand.timestamp,
                    "cumulative_profit": format!("{:.2}", cumulative),
                    "cumulative_profit_bb": format!("{:.1}", cumulative_bb),
                }));
            }
        }

        let response = serde_json::json!({
            "total_hands": hands.len(),
            "total_profit": format!("{:.2}", cumulative),
            "total_profit_bb": format!("{:.1}", cumulative_bb),
            "data_points": points.len(),
            "points": points,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
