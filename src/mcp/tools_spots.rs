use std::collections::HashSet;

use rmcp::model::*;

use crate::search::{self, SearchParams};
use crate::stats;
use crate::summarizer;
use crate::types::{ActionType, HeroResult, Street};

use super::helpers::mcp_error;
use super::params::{
    AutoTagHandsParams, GetBluffCandidatesParams, GetCoolersParams, GetEquitySpotsParams,
    GetMultiwayStatsParams, GetSqueezeSpotsParams,
};
use super::PokerVectorMcp;

impl PokerVectorMcp {
    pub(crate) async fn tool_auto_tag_hands(
        &self,
        params: AutoTagHandsParams,
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
                        && matches!(
                            a.action_type,
                            ActionType::Bet { .. } | ActionType::Raise { .. }
                        )
                        && matches!(a.street, Street::River | Street::Turn | Street::Flop)
                });
                if hero_bet_last_street
                    && invested_bb > min_pot_bb / 2.0
                    && big_bluffs.len() < limit
                {
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

    pub(crate) async fn tool_get_coolers(
        &self,
        params: GetCoolersParams,
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

            if went_to_showdown
                && hand.result.hero_result == HeroResult::Lost
                && invested_bb > min_pot_bb
            {
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
                let summary = summarizer::summarize(hand);
                coolers.push((
                    invested_bb,
                    serde_json::json!({
                        "hand_id": hand.id,
                        "stakes": hand.game_type.to_string(),
                        "hero_cards": cards,
                        "board": board,
                        "invested_bb": format!("{:.1}", invested_bb),
                        "profit_bb": format!("{:.1}", profit_bb),
                        "summary": summary,
                    }),
                ));
            }
        }

        // Sort by invested BB descending (biggest pots first)
        coolers.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        let results: Vec<serde_json::Value> =
            coolers.into_iter().take(limit).map(|(_, v)| v).collect();

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_equity_spots(
        &self,
        params: GetEquitySpotsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(20) as usize;
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
            tag: None,
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

            let hero_allin = hand.actions.iter().any(|a| {
                a.player == *hero
                    && match &a.action_type {
                        ActionType::Call { all_in, .. }
                        | ActionType::Bet { all_in, .. }
                        | ActionType::Raise { all_in, .. } => *all_in,
                        _ => false,
                    }
            });
            if !hero_allin {
                continue;
            }

            // Find the street hero went all-in on
            let allin_action = hand
                .actions
                .iter()
                .find(|a| {
                    a.player == *hero
                        && match &a.action_type {
                            ActionType::Call { all_in, .. }
                            | ActionType::Bet { all_in, .. }
                            | ActionType::Raise { all_in, .. } => *all_in,
                            _ => false,
                        }
                })
                .unwrap();

            let bb = stats::big_blind_size(hand);
            let invested = stats::hero_invested(hand, hero);
            let collected = stats::hero_collected(hand, hero);
            let profit = collected - invested;
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

            // Opponents who showed
            let opponent_hands: Vec<serde_json::Value> = hand
                .actions
                .iter()
                .filter(|a| {
                    a.player != *hero && matches!(a.action_type, ActionType::Shows { .. })
                })
                .map(|a| {
                    if let ActionType::Shows {
                        cards, description, ..
                    } = &a.action_type
                    {
                        let cs: String = cards
                            .iter()
                            .map(|c| match c {
                                Some(c) => c.to_string(),
                                None => "?".to_string(),
                            })
                            .collect::<Vec<_>>()
                            .join(" ");
                        serde_json::json!({ "player": a.player, "cards": cs, "description": description })
                    } else {
                        serde_json::json!({})
                    }
                })
                .collect();

            results.push(serde_json::json!({
                "hand_id": hand.id,
                "stakes": format!("{}", hand.game_type),
                "allin_street": format!("{}", allin_action.street),
                "hero_cards": cards,
                "board": board,
                "pot_size": format!("{:.2}", pot_size),
                "hero_invested": format!("{:.2}", invested),
                "pot_odds": if pot_size > 0.0 { format!("{:.0}%", invested / pot_size * 100.0) } else { "N/A".to_string() },
                "hero_result": format!("{:?}", hand.result.hero_result),
                "profit_bb": format!("{:.1}", if bb > 0.0 { profit / bb } else { 0.0 }),
                "opponent_hands": opponent_hands,
            }));
        }

        let json = serde_json::to_string_pretty(&results)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_multiway_stats(
        &self,
        params: GetMultiwayStatsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hero = params.hero.as_deref().unwrap_or(&self.hero);
        let min_players = params.min_players.unwrap_or(3) as usize;

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

        // Filter to hands where min_players or more saw the flop
        let multiway: Vec<_> = hands
            .into_iter()
            .filter(|h| {
                let mut flop_players = HashSet::new();
                for a in &h.actions {
                    if a.street == Street::Flop {
                        flop_players.insert(&a.player);
                    }
                }
                flop_players.len() >= min_players
            })
            .collect();

        let total_multiway = multiway.len();
        let player_stats = stats::calculate_stats(&multiway, hero);

        let response = serde_json::json!({
            "multiway_hands": total_multiway,
            "min_players_at_flop": min_players,
            "stats": player_stats,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_squeeze_spots(
        &self,
        params: GetSqueezeSpotsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let limit = params.limit.unwrap_or(20) as usize;
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
            tag: None,
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

            // Look for raise + call before hero acts preflop
            let mut saw_raise = false;
            let mut saw_cold_call = false;
            let mut hero_action: Option<&str> = None;

            for a in &hand.actions {
                if a.street != Street::Preflop {
                    continue;
                }
                if a.player == *hero {
                    if saw_raise && saw_cold_call {
                        hero_action = Some(match &a.action_type {
                            ActionType::Raise { .. } => "squeeze",
                            ActionType::Call { .. } => "call",
                            ActionType::Fold => "fold",
                            _ => continue,
                        });
                    }
                    break;
                }
                match &a.action_type {
                    ActionType::Raise { .. } | ActionType::Bet { .. } => {
                        if saw_raise {
                            // 3bet before hero = not a squeeze spot
                            break;
                        }
                        saw_raise = true;
                    }
                    ActionType::Call { .. } => {
                        if saw_raise {
                            saw_cold_call = true;
                        }
                    }
                    _ => {}
                }
            }

            if let Some(action) = hero_action {
                let bb = stats::big_blind_size(hand);
                let profit =
                    stats::hero_collected(hand, hero) - stats::hero_invested(hand, hero);
                let cards: String = hand
                    .hero_cards
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");

                results.push(serde_json::json!({
                    "hand_id": hand.id,
                    "stakes": format!("{}", hand.game_type),
                    "hero_position": hand.hero_position.map(|p| p.to_string()),
                    "hero_cards": cards,
                    "hero_action": action,
                    "hero_result": format!("{:?}", hand.result.hero_result),
                    "profit_bb": format!("{:.1}", if bb > 0.0 { profit / bb } else { 0.0 }),
                }));
            }
        }

        // Summary counts
        let squeezed = results
            .iter()
            .filter(|r| r["hero_action"] == "squeeze")
            .count();
        let called = results
            .iter()
            .filter(|r| r["hero_action"] == "call")
            .count();
        let folded = results
            .iter()
            .filter(|r| r["hero_action"] == "fold")
            .count();

        let response = serde_json::json!({
            "total_spots": results.len(),
            "squeezed": squeezed,
            "called": called,
            "folded": folded,
            "hands": results,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_bluff_candidates(
        &self,
        params: GetBluffCandidatesParams,
    ) -> Result<CallToolResult, ErrorData> {
        let min_pot_bb = params.min_pot_bb.unwrap_or(3.0);
        let limit = params.limit.unwrap_or(20) as usize;
        let hero = &self.hero;

        let filter_params = SearchParams {
            query: String::new(),
            mode: search::SearchMode::default(),
            position: None,
            pot_type: None,
            villain: None,
            stakes: params.stakes,
            result: Some("folded".to_string()),
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

        let mut candidates: Vec<(u32, serde_json::Value)> = Vec::new();

        for hand in &hands {
            let bb = stats::big_blind_size(hand);
            if bb <= 0.0 {
                continue;
            }

            // Find the street hero folded on and what action hero was facing
            let fold_action = match hand.actions.iter().find(|a| {
                a.player.as_str() == hero && matches!(a.action_type, ActionType::Fold)
            }) {
                Some(a) => a,
                None => continue,
            };

            let fold_street = fold_action.street;
            // Only postflop folds
            if matches!(fold_street, Street::Preflop | Street::ThirdStreet) {
                continue;
            }

            // Track pot and find the villain bet hero was facing
            let mut pot = 0.0;
            let mut villain_bet_amount = 0.0;
            let mut villain_name = String::new();
            let mut active_on_street = HashSet::new();

            for a in &hand.actions {
                if a.street == fold_street {
                    active_on_street.insert(a.player.as_str());
                }

                // Stop when we reach hero's fold
                if std::ptr::eq(a, fold_action) {
                    break;
                }

                match &a.action_type {
                    ActionType::PostSmallBlind { amount, .. }
                    | ActionType::PostBigBlind { amount, .. }
                    | ActionType::PostAnte { amount }
                    | ActionType::PostBlind { amount }
                    | ActionType::BringsIn { amount }
                    | ActionType::Call { amount, .. }
                    | ActionType::Bet { amount, .. } => {
                        pot += amount.amount;
                        // Track last bet/raise on fold street facing hero
                        if a.street == fold_street
                            && a.player.as_str() != hero
                            && matches!(
                                a.action_type,
                                ActionType::Bet { .. } | ActionType::Raise { .. }
                            )
                        {
                            villain_bet_amount = amount.amount;
                            villain_name = a.player.clone();
                        }
                    }
                    ActionType::Raise { to, .. } => {
                        pot += to.amount;
                        if a.street == fold_street && a.player.as_str() != hero {
                            villain_bet_amount = to.amount;
                            villain_name = a.player.clone();
                        }
                    }
                    ActionType::UncalledBet { amount } => {
                        pot -= amount.amount;
                    }
                    _ => {}
                }
            }

            let pot_bb = pot / bb;
            if pot_bb < min_pot_bb {
                continue;
            }

            if villain_bet_amount <= 0.0 {
                continue;
            }

            // Compute bluff signals
            let mut score: u32 = 0;
            let mut reasons: Vec<&str> = Vec::new();

            // 1. Villain bet small (< 50% pot before the bet)
            let pot_before_bet = pot - villain_bet_amount;
            let bet_fraction = if pot_before_bet > 0.0 {
                villain_bet_amount / pot_before_bet
            } else {
                1.0
            };
            if bet_fraction < 0.5 {
                score += 3;
                reasons.push("villain bet small (< 50% pot)");
            } else if bet_fraction < 0.75 {
                score += 1;
                reasons.push("villain bet medium (50-75% pot)");
            }

            // 2. Heads-up on that street
            let players_on_street = active_on_street.len();
            if players_on_street <= 2 {
                score += 2;
                reasons.push("heads-up on this street");
            }

            // 3. Hero was in position
            if !villain_name.is_empty() {
                let hero_pos = hand.hero_position;
                let villain_pos = hand
                    .players
                    .iter()
                    .find(|p| p.name == villain_name)
                    .and_then(|p| p.position);
                if let (Some(hp), Some(vp)) = (hero_pos, villain_pos) {
                    if crate::stats::position_order_pos(hp) > crate::stats::position_order_pos(vp) {
                        score += 2;
                        reasons.push("hero was in position");
                    }
                }
            }

            // 4. Late street fold (turn/river folds are bigger missed opportunities)
            if matches!(fold_street, Street::River | Street::SeventhStreet) {
                score += 1;
                reasons.push("folded on the river");
            }

            // Only include if at least 2 signals triggered
            if score < 2 {
                continue;
            }

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

            candidates.push((
                score,
                serde_json::json!({
                    "hand_id": hand.id,
                    "stakes": format!("{}", hand.game_type),
                    "hero_position": hand.hero_position.map(|p| p.to_string()),
                    "hero_cards": cards,
                    "board": board,
                    "fold_street": format!("{}", fold_street),
                    "villain": villain_name,
                    "villain_bet_fraction": format!("{:.0}%", bet_fraction * 100.0),
                    "pot_bb_at_fold": format!("{:.1}", pot_bb),
                    "players_on_street": players_on_street,
                    "bluff_score": score,
                    "reasons": reasons,
                }),
            ));
        }

        // Sort by score descending, then by pot size descending
        candidates.sort_by(|a, b| {
            b.0.cmp(&a.0).then_with(|| {
                let pot_a: f64 = a.1["pot_bb_at_fold"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                let pot_b: f64 = b.1["pot_bb_at_fold"]
                    .as_str()
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0.0);
                pot_b
                    .partial_cmp(&pot_a)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
        });

        let results: Vec<serde_json::Value> = candidates
            .into_iter()
            .take(limit)
            .map(|(_, v)| v)
            .collect();

        let response = serde_json::json!({
            "total_candidates": results.len(),
            "min_pot_bb": min_pot_bb,
            "hands": results,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
