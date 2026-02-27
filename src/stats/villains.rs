use std::collections::HashMap;

use crate::types::*;
use super::calculate::calculate_stats;
use super::types::VillainSummary;
use super::helpers::{hero_invested, hero_collected, big_blind_size};

/// List villain stats for all opponents with at least `min_hands` shared hands.
pub fn list_villains(hands: &[Hand], hero: &str, min_hands: u64) -> Vec<VillainSummary> {
    // Group hands by opponent
    let mut opponent_hands: HashMap<String, Vec<&Hand>> = HashMap::new();

    for hand in hands {
        let hero_in_hand = hand.players.iter().any(|p| p.name == hero && !p.is_sitting_out);
        if !hero_in_hand {
            continue;
        }
        for player in &hand.players {
            if player.name != hero && !player.is_sitting_out {
                opponent_hands.entry(player.name.clone()).or_default().push(hand);
            }
        }
    }

    let mut villains: Vec<VillainSummary> = opponent_hands
        .into_iter()
        .filter(|(_, h)| h.len() as u64 >= min_hands)
        .map(|(name, player_hands)| {
            // Collect owned hands for calculate_stats (needs &[Hand])
            let owned_hands: Vec<Hand> = player_hands.iter().map(|h| (*h).clone()).collect();
            let stats = calculate_stats(&owned_hands, &name);

            // Hero profit tracking (hero's perspective against this villain)
            let mut net_profit = 0.0f64;
            let mut net_profit_bb = 0.0f64;
            let mut hands_won = 0u64;
            let mut hands_lost = 0u64;

            for hand in &player_hands {
                let invested = hero_invested(hand, hero);
                let collected = hero_collected(hand, hero);
                let hand_profit = collected - invested;
                net_profit += hand_profit;
                let bb = big_blind_size(hand);
                if bb > 0.0 {
                    net_profit_bb += hand_profit / bb;
                }
                if hand_profit > 0.0 {
                    hands_won += 1;
                } else if hand_profit < 0.0 {
                    hands_lost += 1;
                }
            }

            VillainSummary {
                name,
                hands: stats.hands_played,
                net_profit,
                net_profit_bb,
                hands_won,
                hands_lost,
                vpip: stats.vpip,
                pfr: stats.pfr,
                aggression_factor: stats.aggression_factor,
                three_bet_pct: stats.three_bet_pct,
                fold_to_three_bet: stats.fold_to_three_bet,
                cbet_flop: stats.cbet_flop,
                fold_to_cbet_flop: stats.fold_to_cbet_flop,
                steal_pct: stats.steal_pct,
                wwsf: stats.wwsf,
            }
        })
        .collect();

    villains.sort_by(|a, b| b.net_profit.partial_cmp(&a.net_profit).unwrap_or(std::cmp::Ordering::Equal));
    villains
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_helpers::*;

    #[test]
    fn test_list_villains() {
        let mut h1 = base_hand();
        h1.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(0.05), all_in: false }, street: Street::Preflop },
        ];

        let mut h2 = base_hand();
        h2.id = 2;
        h2.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: make_money(0.06), all_in: false }, street: Street::Preflop },
        ];

        let villains = list_villains(&[h1, h2], "Hero", 1);
        assert!(villains.iter().any(|v| v.name == "Villain" && v.hands == 2));
        let v = villains.iter().find(|v| v.name == "Villain").unwrap();
        assert!((v.vpip - 100.0).abs() < 0.01); // Villain acted voluntarily in both hands
    }
}
