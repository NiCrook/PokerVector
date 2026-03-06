use std::collections::HashMap;

use rmcp::model::*;

use crate::search::{self, SearchParams};
use crate::stats;
use crate::types::{ActionType, Street};

use super::helpers::mcp_error;
use super::params::{
    GetHandAsReplayerParams, GetHandContextParams, GetHandHistoryParams, GetHandParams,
    GetTagsParams, QuizHandParams, RemoveTagParams, TagHandParams,
};
use super::PokerVectorMcp;

impl PokerVectorMcp {
    pub(crate) async fn tool_get_hand(
        &self,
        params: GetHandParams,
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

    pub(crate) async fn tool_get_hand_as_replayer(
        &self,
        params: GetHandAsReplayerParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hand = self
            .store
            .get_hand(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to retrieve hand: {}", e)))?;

        let hand = match hand {
            Some(h) => h,
            None => {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Hand {} not found",
                    params.hand_id
                ))]))
            }
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
                        ActionType::Shows {
                            cards, description, ..
                        } => {
                            let card_str: String = cards
                                .iter()
                                .map(|c| match c {
                                    Some(c) => c.to_string(),
                                    None => "?".to_string(),
                                })
                                .collect::<Vec<_>>()
                                .join(" ");
                            if let Some(d) = description {
                                format!("shows {} ({})", card_str, d)
                            } else {
                                format!("shows {}", card_str)
                            }
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

        let players_info: Vec<serde_json::Value> = hand
            .players
            .iter()
            .map(|p| {
                serde_json::json!({
                    "name": p.name,
                    "seat": p.seat,
                    "position": p.position.map(|pos| pos.to_string()),
                    "starting_stack": format!("{:.2}", p.stack.amount),
                    "is_hero": p.is_hero,
                })
            })
            .collect();

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

    pub(crate) async fn tool_quiz_hand(
        &self,
        params: QuizHandParams,
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

            // Find a hand where hero had a voluntary postflop action or preflop raise
            hands
                .into_iter()
                .find(|h| {
                    h.actions.iter().any(|a| {
                        a.player == *hero
                            && matches!(
                                a.action_type,
                                ActionType::Bet { .. }
                                    | ActionType::Raise { .. }
                                    | ActionType::Call { .. }
                                    | ActionType::Check
                            )
                            && (a.street != Street::Preflop
                                || matches!(a.action_type, ActionType::Raise { .. }))
                    })
                })
                .ok_or_else(|| mcp_error("No qualifying hand found for quiz"))?
        };

        // Parse target street
        let target_street =
            params
                .street
                .as_deref()
                .and_then(|s| match s.to_lowercase().as_str() {
                    "preflop" => Some(Street::Preflop),
                    "flop" => Some(Street::Flop),
                    "turn" => Some(Street::Turn),
                    "river" => Some(Street::River),
                    _ => None,
                });

        // Find hero's last voluntary action (the decision point)
        let voluntary = |at: &ActionType| {
            matches!(
                at,
                ActionType::Call { .. }
                    | ActionType::Bet { .. }
                    | ActionType::Raise { .. }
                    | ActionType::Check
                    | ActionType::Fold
            )
        };

        let decision_idx = if let Some(target) = target_street {
            hand.actions
                .iter()
                .rposition(|a| a.player == *hero && a.street == target && voluntary(&a.action_type))
        } else {
            hand.actions
                .iter()
                .rposition(|a| a.player == *hero && voluntary(&a.action_type))
        };

        let decision_idx =
            decision_idx.ok_or_else(|| mcp_error("No hero decision found in this hand"))?;

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
            if i >= decision_idx {
                break;
            }
            match &action.action_type {
                ActionType::PostSmallBlind { amount, .. }
                | ActionType::PostBigBlind { amount, .. }
                | ActionType::PostAnte { amount }
                | ActionType::PostBlind { amount }
                | ActionType::BringsIn { amount }
                | ActionType::Call { amount, .. }
                | ActionType::Bet { amount, .. } => {
                    pot_at_decision += amount.amount;
                    if action.player == *hero {
                        hero_invested += amount.amount;
                    }
                }
                ActionType::Raise { to, .. } => {
                    pot_at_decision += to.amount;
                    if action.player == *hero {
                        hero_invested += to.amount;
                    }
                }
                ActionType::UncalledBet { amount } => {
                    pot_at_decision -= amount.amount;
                    if action.player == *hero {
                        hero_invested -= amount.amount;
                    }
                }
                _ => {}
            }
        }

        // Hero stack at decision
        let hero_starting = hand
            .players
            .iter()
            .find(|p| p.name == *hero)
            .map(|p| p.stack.amount)
            .unwrap_or(0.0);
        let hero_stack = hero_starting - hero_invested;

        let bb = stats::big_blind_size(&hand);

        // Actions before the decision
        let actions_before: Vec<serde_json::Value> = hand.actions[..decision_idx]
            .iter()
            .map(|a| {
                serde_json::json!({
                    "street": format!("{}", a.street),
                    "player": if a.player == *hero { "Hero".to_string() } else { a.player.clone() },
                    "action": format!("{:?}", a.action_type),
                })
            })
            .collect();

        let cards: String = hand
            .hero_cards
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let board_visible: String = hand.board[..board_cards_at_street]
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");

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
        let subsequent: Vec<serde_json::Value> = hand.actions[decision_idx + 1..]
            .iter()
            .map(|a| {
                serde_json::json!({
                    "street": format!("{}", a.street),
                    "player": if a.player == *hero { "Hero".to_string() } else { a.player.clone() },
                    "action": format!("{:?}", a.action_type),
                })
            })
            .collect();

        let full_board: String = hand
            .board
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let total_invested = stats::hero_invested(&hand, hero);
        let total_collected = stats::hero_collected(&hand, hero);
        let profit_bb = if bb > 0.0 {
            (total_collected - total_invested) / bb
        } else {
            0.0
        };

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

    pub(crate) async fn tool_get_hand_history(
        &self,
        params: GetHandHistoryParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hand = self
            .store
            .get_hand(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to retrieve hand: {}", e)))?;
        match hand {
            Some(h) => Ok(CallToolResult::success(vec![Content::text(h.raw_text)])),
            None => Ok(CallToolResult::success(vec![Content::text(format!(
                "Hand {} not found",
                params.hand_id
            ))])),
        }
    }

    pub(crate) async fn tool_get_hand_context(
        &self,
        params: GetHandContextParams,
    ) -> Result<CallToolResult, ErrorData> {
        let window = params.window.unwrap_or(5);

        // Get the target hand to find its table
        let target = self
            .store
            .get_hand(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to retrieve hand: {}", e)))?
            .ok_or_else(|| mcp_error(&format!("Hand {} not found", params.hand_id)))?;

        // Scroll all hands from the same table
        let filter = format!(
            "stakes = '{}'",
            target.game_type.to_string().replace('\'', "''")
        );
        let mut hands = self
            .store
            .scroll_hands(Some(filter))
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        // Keep only same table and sort by timestamp
        hands.retain(|h| h.table_name == target.table_name);
        hands.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        // Find the target hand's position
        let pos = hands
            .iter()
            .position(|h| h.id == params.hand_id)
            .ok_or_else(|| mcp_error("Hand not found in table context"))?;

        let start = pos.saturating_sub(window);
        let end = (pos + window + 1).min(hands.len());

        let context: Vec<serde_json::Value> = hands[start..end]
            .iter()
            .map(|h| {
                let bb = stats::big_blind_size(h);
                let profit =
                    stats::hero_collected(h, &self.hero) - stats::hero_invested(h, &self.hero);
                let cards: String = h
                    .hero_cards
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(" ");
                serde_json::json!({
                    "hand_id": h.id,
                    "is_target": h.id == params.hand_id,
                    "timestamp": h.timestamp,
                    "hero_cards": cards,
                    "hero_position": h.hero_position.map(|p| p.to_string()),
                    "hero_result": format!("{:?}", h.result.hero_result),
                    "profit_bb": format!("{:.1}", if bb > 0.0 { profit / bb } else { 0.0 }),
                    "pot_type": stats::classify_pot_type(h),
                })
            })
            .collect();

        let response = serde_json::json!({
            "table_name": target.table_name,
            "target_hand_id": params.hand_id,
            "hands": context,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_tag_hand(
        &self,
        params: TagHandParams,
    ) -> Result<CallToolResult, ErrorData> {
        // Verify hand exists
        let exists = self
            .store
            .hand_exists(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to check hand: {}", e)))?;
        if !exists {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Hand {} not found",
                params.hand_id
            ))]));
        }

        // Get current tags
        let current = self
            .store
            .get_tags(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to get tags: {}", e)))?
            .unwrap_or_default();

        // Parse existing tags from sentinel format
        let mut tag_set: Vec<String> = if current.is_empty() {
            Vec::new()
        } else {
            current
                .trim_matches(',')
                .split(',')
                .map(|s| s.to_string())
                .collect()
        };

        // Add new tags (deduplicate)
        let mut added = Vec::new();
        for tag in &params.tags {
            let tag = tag.trim().to_lowercase();
            if !tag.is_empty() && !tag_set.iter().any(|t| t == &tag) {
                tag_set.push(tag.clone());
                added.push(tag);
            }
        }

        // Store in sentinel format: ,tag1,tag2,
        let new_tags = if tag_set.is_empty() {
            String::new()
        } else {
            format!(",{},", tag_set.join(","))
        };

        self.store
            .update_tags(params.hand_id, &new_tags)
            .await
            .map_err(|e| mcp_error(&format!("Failed to update tags: {}", e)))?;

        let response = serde_json::json!({
            "hand_id": params.hand_id,
            "added": added,
            "all_tags": tag_set,
        });
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_remove_tag(
        &self,
        params: RemoveTagParams,
    ) -> Result<CallToolResult, ErrorData> {
        let exists = self
            .store
            .hand_exists(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to check hand: {}", e)))?;
        if !exists {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Hand {} not found",
                params.hand_id
            ))]));
        }

        let current = self
            .store
            .get_tags(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to get tags: {}", e)))?
            .unwrap_or_default();

        let mut tag_set: Vec<String> = if current.is_empty() {
            Vec::new()
        } else {
            current
                .trim_matches(',')
                .split(',')
                .map(|s| s.to_string())
                .collect()
        };

        let removed: Vec<String> = if params.tags.is_empty() {
            // Remove all tags
            let all = tag_set.clone();
            tag_set.clear();
            all
        } else {
            let mut removed = Vec::new();
            for tag in &params.tags {
                let tag = tag.trim().to_lowercase();
                if let Some(pos) = tag_set.iter().position(|t| t == &tag) {
                    tag_set.remove(pos);
                    removed.push(tag);
                }
            }
            removed
        };

        let new_tags = if tag_set.is_empty() {
            String::new()
        } else {
            format!(",{},", tag_set.join(","))
        };

        self.store
            .update_tags(params.hand_id, &new_tags)
            .await
            .map_err(|e| mcp_error(&format!("Failed to update tags: {}", e)))?;

        let response = serde_json::json!({
            "hand_id": params.hand_id,
            "removed": removed,
            "remaining_tags": tag_set,
        });
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_tags(
        &self,
        params: GetTagsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let exists = self
            .store
            .hand_exists(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to check hand: {}", e)))?;
        if !exists {
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Hand {} not found",
                params.hand_id
            ))]));
        }

        let current = self
            .store
            .get_tags(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to get tags: {}", e)))?
            .unwrap_or_default();

        let tags: Vec<&str> = if current.is_empty() {
            Vec::new()
        } else {
            current.trim_matches(',').split(',').collect()
        };

        let response = serde_json::json!({
            "hand_id": params.hand_id,
            "tags": tags,
        });
        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
