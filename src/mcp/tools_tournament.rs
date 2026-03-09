use rmcp::model::*;

use crate::stats;
use crate::types::*;

use super::helpers::mcp_error;
use super::params::{
    GetBubblePlayParams, GetEffectiveStacksParams, GetPushFoldReviewParams,
    GetTournamentStackStatsParams, GetTournamentSummaryParams,
};
use super::PokerVectorMcp;

// --- Shared helpers ---

async fn fetch_tournament_hands(
    store: &crate::storage::VectorStore,
    tournament_id: u64,
) -> Result<Vec<Hand>, ErrorData> {
    let filter = Some(format!(
        "game_type = 'tournament' AND tournament_id = {}",
        tournament_id
    ));
    let mut hands = store
        .scroll_hands(filter)
        .await
        .map_err(|e| mcp_error(&format!("Failed to fetch tournament hands: {}", e)))?;
    hands.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(hands)
}

fn compute_m_ratio(hand: &Hand, hero: &str) -> Option<f64> {
    let (sb, bb, ante) = match &hand.game_type {
        GameType::Tournament {
            small_blind,
            big_blind,
            ante,
            ..
        } => (
            small_blind.amount,
            big_blind.amount,
            ante.as_ref().map(|a| a.amount).unwrap_or(0.0),
        ),
        _ => return None,
    };
    let hero_player = hand.players.iter().find(|p| p.name == hero)?;
    let active = hand.players.iter().filter(|p| !p.is_sitting_out).count() as f64;
    let orbit_cost = sb + bb + ante * active;
    if orbit_cost <= 0.0 {
        return None;
    }
    Some(hero_player.stack.amount / orbit_cost)
}

fn stack_in_bb(hand: &Hand, hero: &str) -> Option<f64> {
    let bb = stats::big_blind_size(hand);
    if bb <= 0.0 {
        return None;
    }
    let hero_player = hand.players.iter().find(|p| p.name == hero)?;
    Some(hero_player.stack.amount / bb)
}

fn classify_hero_preflop_action(hand: &Hand, hero: &str) -> &'static str {
    let preflop_street = if hand.variant == PokerVariant::SevenCardStud {
        Street::ThirdStreet
    } else {
        Street::Preflop
    };

    for action in &hand.actions {
        if action.street != preflop_street || action.player != hero {
            continue;
        }
        match &action.action_type {
            ActionType::Fold => return "fold",
            ActionType::Raise { all_in: true, .. } | ActionType::Bet { all_in: true, .. } => {
                return "shove"
            }
            ActionType::Raise { .. } | ActionType::Bet { .. } => return "raise",
            ActionType::Call { .. } => return "call/limp",
            ActionType::Check => return "check",
            _ => {}
        }
    }
    "none"
}

fn is_late_position(pos: Option<Position>) -> bool {
    matches!(pos, Some(Position::BTN | Position::CO | Position::SB))
}

// --- Tool implementations ---

impl PokerVectorMcp {
    pub(crate) async fn tool_get_tournament_summary(
        &self,
        params: GetTournamentSummaryParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hands = fetch_tournament_hands(&self.store, params.tournament_id).await?;
        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "error": "No hands found for this tournament",
                    "tournament_id": params.tournament_id
                })
                .to_string(),
            )]));
        }

        let hero = &self.hero;
        let hand_count = hands.len();
        let start_time = hands.first().map(|h| h.timestamp.as_str()).unwrap_or("");
        let end_time = hands.last().map(|h| h.timestamp.as_str()).unwrap_or("");

        let starting_stack = hands
            .first()
            .and_then(|h| h.players.iter().find(|p| p.name.as_str() == hero))
            .map(|p| p.stack.amount)
            .unwrap_or(0.0);
        let ending_stack = hands
            .last()
            .and_then(|h| h.players.iter().find(|p| p.name.as_str() == hero))
            .map(|p| p.stack.amount)
            .unwrap_or(0.0);

        // Blind levels
        let mut levels: Vec<String> = hands
            .iter()
            .filter_map(|h| match &h.game_type {
                GameType::Tournament {
                    level,
                    small_blind,
                    big_blind,
                    ..
                } => Some(format!("L{} {}/{}", level, small_blind, big_blind)),
                _ => None,
            })
            .collect();
        levels.dedup();

        // Biggest wins/losses by BB
        let mut hand_results: Vec<(u64, f64, String)> = hands
            .iter()
            .filter_map(|h| {
                let bb = stats::big_blind_size(h);
                if bb <= 0.0 {
                    return None;
                }
                let invested = stats::hero_invested(h, hero);
                let collected = stats::hero_collected(h, hero);
                let profit_bb = (collected - invested) / bb;
                let ts = h.timestamp.clone();
                Some((h.id, profit_bb, ts))
            })
            .collect();

        hand_results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let biggest_wins: Vec<serde_json::Value> = hand_results
            .iter()
            .filter(|(_, p, _)| *p > 0.0)
            .take(3)
            .map(|(id, p, ts)| {
                serde_json::json!({ "hand_id": id, "profit_bb": format!("{:.1}", p), "timestamp": ts })
            })
            .collect();
        let biggest_losses: Vec<serde_json::Value> = hand_results
            .iter()
            .rev()
            .filter(|(_, p, _)| *p < 0.0)
            .take(3)
            .map(|(id, p, ts)| {
                serde_json::json!({ "hand_id": id, "profit_bb": format!("{:.1}", p), "timestamp": ts })
            })
            .collect();

        // Bustout detection: last hand, hero invested everything, collected nothing
        let last_hand = hands.last().unwrap();
        let busted_out = {
            let invested = stats::hero_invested(last_hand, hero);
            let collected = stats::hero_collected(last_hand, hero);
            invested > 0.0 && collected == 0.0
        };

        let response = serde_json::json!({
            "tournament_id": params.tournament_id,
            "hand_count": hand_count,
            "start_time": start_time,
            "end_time": end_time,
            "starting_stack": starting_stack,
            "ending_stack": ending_stack,
            "blind_levels": levels,
            "biggest_wins": biggest_wins,
            "biggest_losses": biggest_losses,
            "busted_out": busted_out,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_tournament_stack_stats(
        &self,
        params: GetTournamentStackStatsParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hands = fetch_tournament_hands(&self.store, params.tournament_id).await?;
        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "error": "No hands found for this tournament",
                    "tournament_id": params.tournament_id
                })
                .to_string(),
            )]));
        }

        let hero = &self.hero;

        let data_points: Vec<serde_json::Value> = hands
            .iter()
            .filter_map(|h| {
                let stack_bb = stack_in_bb(h, hero)?;
                let m = compute_m_ratio(h, hero)?;
                let level = match &h.game_type {
                    GameType::Tournament {
                        level,
                        small_blind,
                        big_blind,
                        ..
                    } => format!("L{} {}/{}", level, small_blind, big_blind),
                    _ => String::new(),
                };
                let stack = h
                    .players
                    .iter()
                    .find(|p| p.name.as_str() == hero)?
                    .stack
                    .amount;
                Some(serde_json::json!({
                    "hand_id": h.id,
                    "timestamp": h.timestamp,
                    "level": level,
                    "stack": stack,
                    "stack_bb": format!("{:.1}", stack_bb),
                    "m_ratio": format!("{:.1}", m),
                }))
            })
            .collect();

        // Summary stats
        let m_values: Vec<f64> = hands
            .iter()
            .filter_map(|h| compute_m_ratio(h, hero))
            .collect();

        let (min_m, max_m, avg_m, push_fold_count) = if m_values.is_empty() {
            (0.0, 0.0, 0.0, 0usize)
        } else {
            let min = m_values.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = m_values.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            let avg = m_values.iter().sum::<f64>() / m_values.len() as f64;
            let pf = m_values.iter().filter(|&&m| m < 10.0).count();
            (min, max, avg, pf)
        };

        let response = serde_json::json!({
            "tournament_id": params.tournament_id,
            "data_points": data_points,
            "summary": {
                "total_hands": hands.len(),
                "min_m": format!("{:.1}", min_m),
                "max_m": format!("{:.1}", max_m),
                "avg_m": format!("{:.1}", avg_m),
                "hands_in_push_fold": push_fold_count,
            }
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_push_fold_review(
        &self,
        params: GetPushFoldReviewParams,
    ) -> Result<CallToolResult, ErrorData> {
        let m_threshold = params.m_threshold.unwrap_or(10.0);
        let hero = &self.hero;

        let hands = if let Some(tid) = params.tournament_id {
            fetch_tournament_hands(&self.store, tid).await?
        } else {
            let filter = Some("game_type = 'tournament'".to_string());
            let mut h = self
                .store
                .scroll_hands(filter)
                .await
                .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;
            h.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            h
        };

        let mut entries: Vec<serde_json::Value> = Vec::new();
        let mut action_counts: std::collections::HashMap<&str, u32> =
            std::collections::HashMap::new();
        let mut flagged_count = 0u32;

        for hand in &hands {
            let m = match compute_m_ratio(hand, hero) {
                Some(m) if m < m_threshold => m,
                _ => continue,
            };

            let action = classify_hero_preflop_action(hand, hero);
            *action_counts.entry(action).or_insert(0) += 1;

            // Flag questionable plays
            let mut flags: Vec<&str> = Vec::new();
            let pos = hand.hero_position;

            if action == "fold" && m < 6.0 && is_late_position(pos) {
                flags.push("folding late position with very low M");
            }
            if action == "call/limp" && m < 8.0 {
                flags.push("limping/calling with low M — consider shove or fold");
            }
            if action == "raise" && m < 5.0 {
                flags.push("non-all-in raise with M < 5 — should be shove or fold");
            }

            if !flags.is_empty() {
                flagged_count += 1;
            }

            let tid = match &hand.game_type {
                GameType::Tournament { tournament_id, .. } => *tournament_id,
                _ => 0,
            };

            entries.push(serde_json::json!({
                "hand_id": hand.id,
                "tournament_id": tid,
                "timestamp": hand.timestamp,
                "m_ratio": format!("{:.1}", m),
                "position": pos.map(|p| p.to_string()).unwrap_or_default(),
                "action": action,
                "flags": flags,
            }));
        }

        let distribution: serde_json::Value = action_counts
            .iter()
            .map(|(k, v)| (k.to_string(), serde_json::json!(v)))
            .collect::<serde_json::Map<String, serde_json::Value>>()
            .into();

        let response = serde_json::json!({
            "m_threshold": m_threshold,
            "total_low_m_hands": entries.len(),
            "flagged_hands": flagged_count,
            "action_distribution": distribution,
            "hands": entries,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_bubble_play(
        &self,
        params: GetBubblePlayParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hands = fetch_tournament_hands(&self.store, params.tournament_id).await?;
        if hands.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                serde_json::json!({
                    "error": "No hands found for this tournament",
                    "tournament_id": params.tournament_id
                })
                .to_string(),
            )]));
        }

        let hero = &self.hero;

        // Bubble heuristic: hero M < 15 AND 2+ other players also M < 15
        let mut pre_bubble: Vec<&Hand> = Vec::new();
        let mut bubble: Vec<&Hand> = Vec::new();

        for hand in &hands {
            let hero_m = compute_m_ratio(hand, hero).unwrap_or(f64::MAX);

            if hero_m >= 15.0 {
                pre_bubble.push(hand);
                continue;
            }

            // Count other players with low M
            let low_m_others = hand
                .players
                .iter()
                .filter(|p| !p.is_hero && !p.is_sitting_out)
                .filter(|p| {
                    compute_m_ratio(hand, &p.name)
                        .map(|m| m < 15.0)
                        .unwrap_or(false)
                })
                .count();

            if low_m_others >= 2 {
                bubble.push(hand);
            } else {
                pre_bubble.push(hand);
            }
        }

        let pre_bubble_stats = if !pre_bubble.is_empty() {
            let owned: Vec<Hand> = pre_bubble.into_iter().cloned().collect();
            Some(stats::calculate_stats(&owned, hero))
        } else {
            None
        };

        let bubble_stats = if !bubble.is_empty() {
            let owned: Vec<Hand> = bubble.into_iter().cloned().collect();
            Some(stats::calculate_stats(&owned, hero))
        } else {
            None
        };

        let format_phase = |stats: &Option<stats::PlayerStats>, label: &str| -> serde_json::Value {
            match stats {
                Some(s) => serde_json::json!({
                    "phase": label,
                    "hands": s.hands_played,
                    "vpip": format!("{:.1}", s.vpip),
                    "pfr": format!("{:.1}", s.pfr),
                    "steal_pct": format!("{:.1}", s.steal_pct),
                    "fold_to_steal_bb": format!("{:.1}", s.fold_to_steal_bb),
                }),
                None => serde_json::json!({
                    "phase": label,
                    "hands": 0,
                }),
            }
        };

        let pre_bubble_json = format_phase(&pre_bubble_stats, "pre_bubble");
        let bubble_json = format_phase(&bubble_stats, "bubble");

        // Compute tightening deltas
        let delta = match (&pre_bubble_stats, &bubble_stats) {
            (Some(pre), Some(bub)) => serde_json::json!({
                "vpip_delta": format!("{:.1}", bub.vpip - pre.vpip),
                "steal_delta": format!("{:.1}", bub.steal_pct - pre.steal_pct),
                "pfr_delta": format!("{:.1}", bub.pfr - pre.pfr),
            }),
            _ => serde_json::json!(null),
        };

        let response = serde_json::json!({
            "tournament_id": params.tournament_id,
            "pre_bubble": pre_bubble_json,
            "bubble": bubble_json,
            "tightening_delta": delta,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_effective_stacks(
        &self,
        params: GetEffectiveStacksParams,
    ) -> Result<CallToolResult, ErrorData> {
        let min_pot_bb = params.min_pot_bb.unwrap_or(10.0);
        let hero = &self.hero;

        let hands = if let Some(tid) = params.tournament_id {
            fetch_tournament_hands(&self.store, tid).await?
        } else {
            let filter = Some("game_type = 'tournament'".to_string());
            let mut h = self
                .store
                .scroll_hands(filter)
                .await
                .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;
            h.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
            h
        };

        let mut entries: Vec<(f64, serde_json::Value)> = Vec::new();

        for hand in &hands {
            let bb = stats::big_blind_size(hand);
            if bb <= 0.0 {
                continue;
            }
            let pot_bb = hand.pot.map(|p| p.amount / bb).unwrap_or(0.0);
            if pot_bb < min_pot_bb {
                continue;
            }

            let hero_stack_bb = match stack_in_bb(hand, hero) {
                Some(s) => s,
                None => continue,
            };

            // Find involved opponents (those who put money in beyond blinds/antes)
            let involved: std::collections::HashSet<&str> = hand
                .actions
                .iter()
                .filter(|a| {
                    a.player.as_str() != hero
                        && matches!(
                            a.action_type,
                            ActionType::Call { .. }
                                | ActionType::Bet { .. }
                                | ActionType::Raise { .. }
                        )
                })
                .map(|a| a.player.as_str())
                .collect();

            if involved.is_empty() {
                continue;
            }

            // Villain effective stack = min stack among involved opponents
            let villain_eff_bb = hand
                .players
                .iter()
                .filter(|p| involved.contains(p.name.as_str()))
                .filter_map(|p| {
                    if bb > 0.0 {
                        Some(p.stack.amount / bb)
                    } else {
                        None
                    }
                })
                .fold(f64::INFINITY, f64::min);

            if villain_eff_bb == f64::INFINITY {
                continue;
            }

            let effective_bb = hero_stack_bb.min(villain_eff_bb);

            let tid = match &hand.game_type {
                GameType::Tournament { tournament_id, .. } => *tournament_id,
                _ => 0,
            };

            entries.push((
                pot_bb,
                serde_json::json!({
                    "hand_id": hand.id,
                    "tournament_id": tid,
                    "timestamp": hand.timestamp,
                    "pot_bb": format!("{:.1}", pot_bb),
                    "hero_stack_bb": format!("{:.1}", hero_stack_bb),
                    "villain_eff_bb": format!("{:.1}", villain_eff_bb),
                    "effective_stack_bb": format!("{:.1}", effective_bb),
                }),
            ));
        }

        // Sort by pot size descending, limit 50
        entries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        entries.truncate(50);

        let results: Vec<serde_json::Value> = entries.into_iter().map(|(_, v)| v).collect();

        let response = serde_json::json!({
            "min_pot_bb": min_pot_bb,
            "results_count": results.len(),
            "hands": results,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
