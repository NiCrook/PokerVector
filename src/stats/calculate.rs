use std::collections::HashMap;

use crate::types::*;
use super::types::{PlayerStats, PositionStats};
use super::helpers::*;
use super::preflop::*;
use super::cbet::cbet_analysis;
use super::steal::steal_analysis;
use super::limp::limp_analysis;
use super::postflop::*;

/// Calculate comprehensive stats for a player across a set of hands.
pub fn calculate_stats(hands: &[Hand], hero: &str) -> PlayerStats {
    let mut hands_dealt = 0u64;
    let mut vpip_count = 0u64;
    let mut pfr_count = 0u64;
    let mut three_bet_opportunities = 0u64;
    let mut three_bet_count = 0u64;
    let mut fold_to_3bet_opportunities = 0u64;
    let mut fold_to_3bet_count = 0u64;
    let mut postflop_bets_raises = 0u64;
    let mut postflop_calls = 0u64;
    let mut net_won = 0.0f64;
    let mut total_bb = 0.0f64;
    let mut saw_flop_count = 0u64;
    let mut went_to_showdown_count = 0u64;
    let mut won_at_showdown_count = 0u64;

    // New stat counters
    let mut cbet_flop_opp = 0u64;
    let mut cbet_flop_count = 0u64;
    let mut cbet_turn_opp = 0u64;
    let mut cbet_turn_count = 0u64;
    let mut fold_to_cbet_flop_opp = 0u64;
    let mut fold_to_cbet_flop_count = 0u64;
    let mut fold_to_cbet_turn_opp = 0u64;
    let mut fold_to_cbet_turn_count = 0u64;
    let mut steal_opp = 0u64;
    let mut steal_count = 0u64;
    let mut fold_to_steal_bb_opp = 0u64;
    let mut fold_to_steal_bb_count = 0u64;
    let mut fold_to_steal_sb_opp = 0u64;
    let mut fold_to_steal_sb_count = 0u64;
    let mut limp_opp = 0u64;
    let mut limp_count = 0u64;
    let mut limp_call_opp = 0u64;
    let mut limp_call_count = 0u64;
    let mut limp_fold_opp = 0u64;
    let mut limp_fold_count = 0u64;
    let mut limp_raise_opp = 0u64;
    let mut limp_raise_count = 0u64;
    let mut donk_opp = 0u64;
    let mut donk_count = 0u64;
    let mut float_opp = 0u64;
    let mut float_count = 0u64;
    let mut cr_opp = 0u64;
    let mut cr_count = 0u64;
    let mut probe_opp = 0u64;
    let mut probe_count = 0u64;
    let mut squeeze_opp = 0u64;
    let mut squeeze_count = 0u64;
    let mut cold_call_opp = 0u64;
    let mut cold_call_count = 0u64;
    let mut wwsf_opp = 0u64;
    let mut wwsf_count = 0u64;
    let mut overbet_total = 0u64;
    let mut overbet_count = 0u64;

    let mut position_data: HashMap<String, (u64, u64, u64)> = HashMap::new(); // (hands, vpip, pfr)

    for hand in hands {
        let is_hero_in_hand = hand.players.iter().any(|p| p.name == hero && !p.is_sitting_out);
        if !is_hero_in_hand {
            continue;
        }

        hands_dealt += 1;

        let bb_size = big_blind_size(hand);
        let pos_key = hand.players.iter()
            .find(|p| p.name == hero)
            .and_then(|p| p.position)
            .map(|p| p.to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let entry = position_data.entry(pos_key).or_insert((0, 0, 0));
        entry.0 += 1;

        // Preflop analysis
        let (is_vpip, is_pfr) = preflop_vpip_pfr(hand, hero);
        if is_vpip {
            vpip_count += 1;
            entry.1 += 1;
        }
        if is_pfr {
            pfr_count += 1;
            entry.2 += 1;
        }

        // 3-bet analysis
        let (three_bet_opp, did_three_bet) = three_bet_analysis(hand, hero);
        if three_bet_opp {
            three_bet_opportunities += 1;
            if did_three_bet {
                three_bet_count += 1;
            }
        }

        // Fold-to-3bet analysis
        let (ft3b_opp, did_fold_to_3bet) = fold_to_three_bet_analysis(hand, hero);
        if ft3b_opp {
            fold_to_3bet_opportunities += 1;
            if did_fold_to_3bet {
                fold_to_3bet_count += 1;
            }
        }

        // C-bet analysis
        let cbet = cbet_analysis(hand, hero);
        if cbet.flop.0 { cbet_flop_opp += 1; if cbet.flop.1 { cbet_flop_count += 1; } }
        if cbet.turn.0 { cbet_turn_opp += 1; if cbet.turn.1 { cbet_turn_count += 1; } }
        if cbet.fold_to_flop.0 { fold_to_cbet_flop_opp += 1; if cbet.fold_to_flop.1 { fold_to_cbet_flop_count += 1; } }
        if cbet.fold_to_turn.0 { fold_to_cbet_turn_opp += 1; if cbet.fold_to_turn.1 { fold_to_cbet_turn_count += 1; } }

        // Steal analysis
        let steal = steal_analysis(hand, hero);
        if steal.steal.0 { steal_opp += 1; if steal.steal.1 { steal_count += 1; } }
        if steal.fold_bb.0 { fold_to_steal_bb_opp += 1; if steal.fold_bb.1 { fold_to_steal_bb_count += 1; } }
        if steal.fold_sb.0 { fold_to_steal_sb_opp += 1; if steal.fold_sb.1 { fold_to_steal_sb_count += 1; } }

        // Limp analysis
        let limp = limp_analysis(hand, hero);
        if limp.limp.0 { limp_opp += 1; if limp.limp.1 { limp_count += 1; } }
        if limp.limp_call.0 { limp_call_opp += 1; if limp.limp_call.1 { limp_call_count += 1; } }
        if limp.limp_fold.0 { limp_fold_opp += 1; if limp.limp_fold.1 { limp_fold_count += 1; } }
        if limp.limp_raise.0 { limp_raise_opp += 1; if limp.limp_raise.1 { limp_raise_count += 1; } }

        // Donk bet analysis
        let (d_opp, d_count) = donk_bet_analysis(hand, hero);
        donk_opp += d_opp;
        donk_count += d_count;

        // Float analysis
        let (f_opp, f_count) = float_analysis(hand, hero);
        float_opp += f_opp;
        float_count += f_count;

        // Check-raise analysis
        let (c_opp, c_count) = check_raise_analysis(hand, hero);
        cr_opp += c_opp;
        cr_count += c_count;

        // Probe analysis
        let (p_opp, p_count) = probe_analysis(hand, hero);
        probe_opp += p_opp;
        probe_count += p_count;

        // Squeeze analysis
        let (sq_opp, sq_did) = squeeze_analysis(hand, hero);
        if sq_opp { squeeze_opp += 1; if sq_did { squeeze_count += 1; } }

        // Cold call analysis
        let (cc_opp, cc_did) = cold_call_analysis(hand, hero);
        if cc_opp { cold_call_opp += 1; if cc_did { cold_call_count += 1; } }

        // WWSF analysis
        let (ww_opp, ww_did) = wwsf_analysis(hand, hero);
        if ww_opp { wwsf_opp += 1; if ww_did { wwsf_count += 1; } }

        // Overbet analysis
        let (ob_total, ob_count) = overbet_analysis(hand, hero);
        overbet_total += ob_total;
        overbet_count += ob_count;

        // Postflop aggression
        for action in &hand.actions {
            if action.player != hero {
                continue;
            }
            if action.street == Street::Preflop || action.street == Street::Showdown {
                continue;
            }
            match &action.action_type {
                ActionType::Bet { .. } | ActionType::Raise { .. } => postflop_bets_raises += 1,
                ActionType::Call { .. } => postflop_calls += 1,
                _ => {}
            }
        }

        // Did hero see the flop?
        let hero_saw_flop = hero_saw_street(hand, hero, Street::Flop);
        if hero_saw_flop {
            saw_flop_count += 1;
        }

        // Showdown tracking
        let hand_reached_showdown = hand.actions.iter().any(|a| a.street == Street::Showdown);
        let hero_at_showdown = hand_reached_showdown && hero_saw_flop && !hero_folded_before_showdown(hand, hero);
        if hero_at_showdown {
            went_to_showdown_count += 1;
            if hand.result.hero_result == HeroResult::Won {
                won_at_showdown_count += 1;
            }
        }

        // Win rate calculation
        let invested = hero_invested(hand, hero);
        let collected = hero_collected(hand, hero);
        net_won += collected - invested;
        total_bb += bb_size;
    }

    let positions = if position_data.is_empty() {
        None
    } else {
        Some(position_data.into_iter().map(|(pos, (h, v, p))| {
            (pos, PositionStats {
                hands: h,
                vpip: if h > 0 { v as f64 / h as f64 * 100.0 } else { 0.0 },
                pfr: if h > 0 { p as f64 / h as f64 * 100.0 } else { 0.0 },
            })
        }).collect())
    };

    let pct = |num: u64, den: u64| -> f64 {
        if den > 0 { num as f64 / den as f64 * 100.0 } else { 0.0 }
    };

    PlayerStats {
        hands_played: hands_dealt,
        vpip: pct(vpip_count, hands_dealt),
        pfr: pct(pfr_count, hands_dealt),
        three_bet_pct: pct(three_bet_count, three_bet_opportunities),
        fold_to_three_bet: pct(fold_to_3bet_count, fold_to_3bet_opportunities),
        aggression_factor: if postflop_calls > 0 { postflop_bets_raises as f64 / postflop_calls as f64 } else if postflop_bets_raises > 0 { f64::INFINITY } else { 0.0 },
        winrate_bb100: if hands_dealt > 0 && total_bb > 0.0 { (net_won / (total_bb / hands_dealt as f64)) / hands_dealt as f64 * 100.0 } else { 0.0 },
        went_to_showdown_pct: pct(went_to_showdown_count, saw_flop_count),
        won_at_showdown_pct: pct(won_at_showdown_count, went_to_showdown_count),
        cbet_flop: pct(cbet_flop_count, cbet_flop_opp),
        cbet_turn: pct(cbet_turn_count, cbet_turn_opp),
        fold_to_cbet_flop: pct(fold_to_cbet_flop_count, fold_to_cbet_flop_opp),
        fold_to_cbet_turn: pct(fold_to_cbet_turn_count, fold_to_cbet_turn_opp),
        steal_pct: pct(steal_count, steal_opp),
        fold_to_steal_bb: pct(fold_to_steal_bb_count, fold_to_steal_bb_opp),
        fold_to_steal_sb: pct(fold_to_steal_sb_count, fold_to_steal_sb_opp),
        limp_pct: pct(limp_count, limp_opp),
        limp_call: pct(limp_call_count, limp_call_opp),
        limp_fold: pct(limp_fold_count, limp_fold_opp),
        limp_raise: pct(limp_raise_count, limp_raise_opp),
        donk_bet_pct: pct(donk_count, donk_opp),
        float_pct: pct(float_count, float_opp),
        check_raise_pct: pct(cr_count, cr_opp),
        probe_bet_pct: pct(probe_count, probe_opp),
        squeeze_pct: pct(squeeze_count, squeeze_opp),
        cold_call_pct: pct(cold_call_count, cold_call_opp),
        wwsf: pct(wwsf_count, wwsf_opp),
        overbet_pct: pct(overbet_count, overbet_total),
        positions,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::test_helpers::*;

    #[test]
    fn test_vpip_pfr() {
        let mut h1 = base_hand();
        h1.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];
        h1.result = HandResult { winners: vec![Winner { player: "Hero".to_string(), amount: make_money(0.03), pot: "Main pot".to_string() }], hero_result: HeroResult::Won };

        let mut h2 = base_hand();
        h2.id = 2;
        h2.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];

        let stats = calculate_stats(&[h1, h2], "Hero");
        assert_eq!(stats.hands_played, 2);
        assert!((stats.vpip - 50.0).abs() < 0.01);
        assert!((stats.pfr - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_vpip_call_is_vpip_not_pfr() {
        let mut h1 = base_hand();
        h1.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: make_money(0.06), all_in: false }, street: Street::Preflop },
        ];

        let stats = calculate_stats(&[h1], "Hero");
        assert!((stats.vpip - 100.0).abs() < 0.01);
        assert!((stats.pfr - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_aggression_factor() {
        let mut hand = base_hand();
        hand.actions = vec![
            // Preflop (should not count)
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(0.05), all_in: false }, street: Street::Preflop },
            // Flop: hero bets, villain calls
            Action { player: "Hero".to_string(), action_type: ActionType::Bet { amount: make_money(0.08), all_in: false }, street: Street::Flop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(0.08), all_in: false }, street: Street::Flop },
            // Turn: hero checks, villain bets, hero calls
            Action { player: "Hero".to_string(), action_type: ActionType::Check, street: Street::Turn },
            Action { player: "Villain".to_string(), action_type: ActionType::Bet { amount: make_money(0.15), all_in: false }, street: Street::Turn },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: make_money(0.15), all_in: false }, street: Street::Turn },
        ];
        hand.board = vec![make_card('T', 'h'), make_card('5', 'd'), make_card('2', 'c'), make_card('7', 's')];
        hand.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: make_money(0.56), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };

        let stats = calculate_stats(&[hand], "Hero");
        // 1 bet postflop, 1 call postflop => AF = 1.0
        assert!((stats.aggression_factor - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_showdown_stats() {
        let mut hand = base_hand();
        hand.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(0.05), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Bet { amount: make_money(0.08), all_in: false }, street: Street::Flop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(0.08), all_in: false }, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::Shows { cards: vec![], description: None }, street: Street::Showdown },
        ];
        hand.board = vec![make_card('T', 'h'), make_card('5', 'd'), make_card('2', 'c')];
        hand.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: make_money(0.28), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.went_to_showdown_pct - 100.0).abs() < 0.01);
        assert!((stats.won_at_showdown_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_sat_out_excluded() {
        let mut hand = base_hand();
        hand.players[0].is_sitting_out = true; // Hero sitting out
        hand.actions = vec![];
        hand.result.hero_result = HeroResult::SatOut;

        let stats = calculate_stats(&[hand], "Hero");
        assert_eq!(stats.hands_played, 0);
    }

    #[test]
    fn test_winrate_bb100() {
        let mut h1 = base_hand();
        h1.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: make_money(0.02), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: make_money(0.04), all_in: false }, street: Street::Preflop },
        ];
        h1.result = HandResult {
            winners: vec![Winner { player: "Hero".to_string(), amount: make_money(0.12), pot: "Main pot".to_string() }],
            hero_result: HeroResult::Won,
        };
        // Hero invested: 0.02 (bb) + 0.04 (call) = 0.06, collected 0.12, net = +0.06
        // BB size = 0.02, so net in BB = +3.0
        // winrate = 3.0 / 1 * 100 = 300 bb/100

        let stats = calculate_stats(&[h1], "Hero");
        assert!((stats.winrate_bb100 - 300.0).abs() < 0.01);
    }
}
