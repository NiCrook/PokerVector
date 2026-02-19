use std::collections::HashMap;

use crate::types::*;
use super::types::VillainSummary;
use super::preflop::*;
use super::cbet::cbet_analysis;
use super::steal::steal_analysis;
use super::postflop::wwsf_analysis;

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
            let total = player_hands.len() as u64;
            let mut vpip_count = 0u64;
            let mut pfr_count = 0u64;
            let mut postflop_bets_raises = 0u64;
            let mut postflop_calls = 0u64;
            let mut three_bet_opp = 0u64;
            let mut three_bet_cnt = 0u64;
            let mut ft3b_opp = 0u64;
            let mut ft3b_cnt = 0u64;
            let mut cbet_flop_opp = 0u64;
            let mut cbet_flop_cnt = 0u64;
            let mut ftcb_flop_opp = 0u64;
            let mut ftcb_flop_cnt = 0u64;
            let mut steal_opp = 0u64;
            let mut steal_cnt = 0u64;
            let mut wwsf_opp = 0u64;
            let mut wwsf_cnt = 0u64;

            for hand in &player_hands {
                let (is_vpip, is_pfr) = preflop_vpip_pfr(hand, &name);
                if is_vpip { vpip_count += 1; }
                if is_pfr { pfr_count += 1; }

                let (tb_opp, tb_did) = three_bet_analysis(hand, &name);
                if tb_opp { three_bet_opp += 1; if tb_did { three_bet_cnt += 1; } }

                let (f3_opp, f3_did) = fold_to_three_bet_analysis(hand, &name);
                if f3_opp { ft3b_opp += 1; if f3_did { ft3b_cnt += 1; } }

                let cbet = cbet_analysis(hand, &name);
                if cbet.flop.0 { cbet_flop_opp += 1; if cbet.flop.1 { cbet_flop_cnt += 1; } }
                if cbet.fold_to_flop.0 { ftcb_flop_opp += 1; if cbet.fold_to_flop.1 { ftcb_flop_cnt += 1; } }

                let steal = steal_analysis(hand, &name);
                if steal.steal.0 { steal_opp += 1; if steal.steal.1 { steal_cnt += 1; } }

                let (ww_opp, ww_did) = wwsf_analysis(hand, &name);
                if ww_opp { wwsf_opp += 1; if ww_did { wwsf_cnt += 1; } }

                for action in &hand.actions {
                    if action.player != name { continue; }
                    if action.street == Street::Preflop || action.street == Street::Showdown { continue; }
                    match &action.action_type {
                        ActionType::Bet { .. } | ActionType::Raise { .. } => postflop_bets_raises += 1,
                        ActionType::Call { .. } => postflop_calls += 1,
                        _ => {}
                    }
                }
            }

            let pct = |num: u64, den: u64| -> f64 {
                if den > 0 { num as f64 / den as f64 * 100.0 } else { 0.0 }
            };

            VillainSummary {
                name,
                hands: total,
                vpip: pct(vpip_count, total),
                pfr: pct(pfr_count, total),
                aggression_factor: if postflop_calls > 0 { postflop_bets_raises as f64 / postflop_calls as f64 } else if postflop_bets_raises > 0 { f64::INFINITY } else { 0.0 },
                three_bet_pct: pct(three_bet_cnt, three_bet_opp),
                fold_to_three_bet: pct(ft3b_cnt, ft3b_opp),
                cbet_flop: pct(cbet_flop_cnt, cbet_flop_opp),
                fold_to_cbet_flop: pct(ftcb_flop_cnt, ftcb_flop_opp),
                steal_pct: pct(steal_cnt, steal_opp),
                wwsf: pct(wwsf_cnt, wwsf_opp),
            }
        })
        .collect();

    villains.sort_by(|a, b| b.hands.cmp(&a.hands));
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
