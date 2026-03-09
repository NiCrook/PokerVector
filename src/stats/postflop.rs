use super::helpers::{find_preflop_aggressor, hero_saw_street, is_player_ip};
use crate::types::*;

/// Donk bet: bet into the PFA from OOP on any postflop street.
/// Returns (opportunities, donks) aggregated across all postflop streets.
pub(crate) fn donk_bet_analysis(hand: &Hand, player: &str) -> (u64, u64) {
    let pfa = match find_preflop_aggressor(hand) {
        Some(p) => p,
        None => return (0, 0),
    };
    if player == pfa {
        return (0, 0); // PFA can't donk into themselves
    }
    if is_player_ip(hand, player, &pfa) {
        return (0, 0); // Must be OOP to donk
    }

    let mut opportunities = 0u64;
    let mut donks = 0u64;

    for &street in &[Street::Flop, Street::Turn, Street::River] {
        if !hero_saw_street(hand, player, street) || !hero_saw_street(hand, &pfa, street) {
            continue;
        }
        // Check if player is first to act (or checked to) on this street
        let first_action = hand
            .actions
            .iter()
            .find(|a| a.street == street && (a.player == player || a.player == pfa));
        if let Some(a) = first_action {
            if a.player == player {
                // Player acts first on this street — opportunity to donk
                opportunities += 1;
                if matches!(a.action_type, ActionType::Bet { .. }) {
                    donks += 1;
                }
            }
        }
    }

    (opportunities, donks)
}

/// Float: call a bet IP on one street, then bet when checked to on the next street.
/// Returns (opportunities, floats).
pub(crate) fn float_analysis(hand: &Hand, player: &str) -> (u64, u64) {
    let mut opportunities = 0u64;
    let mut floats = 0u64;

    let street_pairs = [(Street::Flop, Street::Turn), (Street::Turn, Street::River)];

    for &(street1, street2) in &street_pairs {
        if !hero_saw_street(hand, player, street1) || !hero_saw_street(hand, player, street2) {
            continue;
        }

        // Did player call a bet IP on street1?
        let mut someone_bet_street1 = false;
        let mut player_called_ip = false;
        let mut bettor: Option<String> = None;

        for action in hand.actions.iter().filter(|a| a.street == street1) {
            if matches!(action.action_type, ActionType::Bet { .. }) && action.player != player {
                someone_bet_street1 = true;
                bettor = Some(action.player.clone());
            }
            if someone_bet_street1
                && action.player == player
                && matches!(action.action_type, ActionType::Call { .. })
            {
                if let Some(ref b) = bettor {
                    if is_player_ip(hand, player, b) {
                        player_called_ip = true;
                    }
                }
            }
        }

        if !player_called_ip {
            continue;
        }

        // Did opponent check on street2 and player bet?
        if let Some(ref b) = bettor {
            let opponent_checked = hand
                .actions
                .iter()
                .find(|a| a.street == street2 && a.player == *b)
                .map(|a| matches!(a.action_type, ActionType::Check))
                .unwrap_or(false);

            if opponent_checked {
                opportunities += 1;
                let player_bet = hand
                    .actions
                    .iter()
                    .find(|a| a.street == street2 && a.player == player)
                    .map(|a| matches!(a.action_type, ActionType::Bet { .. }))
                    .unwrap_or(false);
                if player_bet {
                    floats += 1;
                }
            }
        }
    }

    (opportunities, floats)
}

/// Check-raise per street: check then raise on the same postflop street.
/// Returns (flop_opp, flop_cr, turn_opp, turn_cr, river_opp, river_cr).
pub(crate) fn check_raise_by_street_analysis(
    hand: &Hand,
    player: &str,
) -> (u64, u64, u64, u64, u64, u64) {
    let mut results = [(0u64, 0u64); 3]; // flop, turn, river

    for (i, &street) in [Street::Flop, Street::Turn, Street::River]
        .iter()
        .enumerate()
    {
        let street_actions: Vec<&Action> =
            hand.actions.iter().filter(|a| a.street == street).collect();

        let player_checked = street_actions
            .iter()
            .any(|a| a.player == player && matches!(a.action_type, ActionType::Check));

        if !player_checked {
            continue;
        }

        let mut past_check = false;
        let mut faced_bet = false;
        for action in &street_actions {
            if action.player == player && matches!(action.action_type, ActionType::Check) {
                past_check = true;
                continue;
            }
            if past_check
                && action.player != player
                && matches!(action.action_type, ActionType::Bet { .. })
            {
                faced_bet = true;
                continue;
            }
            if faced_bet && action.player == player {
                results[i].0 += 1;
                if matches!(action.action_type, ActionType::Raise { .. }) {
                    results[i].1 += 1;
                }
                break;
            }
        }
    }

    (
        results[0].0,
        results[0].1,
        results[1].0,
        results[1].1,
        results[2].0,
        results[2].1,
    )
}

/// Check-raise: check then raise on the same postflop street (aggregate).
/// Returns (opportunities: times checked and faced a bet, did_check_raise).
pub(crate) fn check_raise_analysis(hand: &Hand, player: &str) -> (u64, u64) {
    let (fo, fc, to, tc, ro, rc) = check_raise_by_street_analysis(hand, player);
    (fo + to + ro, fc + tc + rc)
}

/// Probe bet: PFA checked behind on a street, player bets on the next street.
/// Returns (opportunities, probes).
pub(crate) fn probe_analysis(hand: &Hand, player: &str) -> (u64, u64) {
    let pfa = match find_preflop_aggressor(hand) {
        Some(p) => p,
        None => return (0, 0),
    };
    if player == pfa {
        return (0, 0); // PFA can't probe against themselves
    }

    let mut opportunities = 0u64;
    let mut probes = 0u64;

    let street_pairs = [(Street::Flop, Street::Turn), (Street::Turn, Street::River)];

    for &(street1, street2) in &street_pairs {
        if !hero_saw_street(hand, player, street1) || !hero_saw_street(hand, player, street2) {
            continue;
        }

        // Did PFA check on street1 (not bet)?
        let pfa_action_street1 = hand
            .actions
            .iter()
            .find(|a| a.street == street1 && a.player == pfa);
        let pfa_checked = pfa_action_street1
            .map(|a| matches!(a.action_type, ActionType::Check))
            .unwrap_or(false);

        // Also: no bet/raise on street1 from PFA
        let pfa_bet_street1 = hand.actions.iter().any(|a| {
            a.street == street1
                && a.player == pfa
                && matches!(
                    a.action_type,
                    ActionType::Bet { .. } | ActionType::Raise { .. }
                )
        });

        if !pfa_checked || pfa_bet_street1 {
            continue;
        }

        // Player has opportunity to probe on street2
        let player_action_street2 = hand.actions.iter().find(|a| {
            a.street == street2
                && a.player == player
                && !matches!(
                    a.action_type,
                    ActionType::PostSmallBlind { .. } | ActionType::PostBigBlind { .. }
                )
        });

        if let Some(action) = player_action_street2 {
            opportunities += 1;
            if matches!(action.action_type, ActionType::Bet { .. }) {
                probes += 1;
            }
        }
    }

    (opportunities, probes)
}

/// Overbet: postflop bet/raise larger than the pot.
/// Returns (total_postflop_bets_raises, overbets).
pub(crate) fn overbet_analysis(hand: &Hand, player: &str) -> (u64, u64) {
    let mut total = 0u64;
    let mut overbets = 0u64;

    // Track running pot for overbet detection
    let mut running_pot = 0.0f64;
    let mut current_street = Street::Preflop;

    for action in &hand.actions {
        if action.street != current_street {
            current_street = action.street;
        }

        // Update running pot
        match &action.action_type {
            ActionType::PostSmallBlind { amount, .. }
            | ActionType::PostBigBlind { amount, .. }
            | ActionType::PostAnte { amount }
            | ActionType::PostBlind { amount }
            | ActionType::Call { amount, .. }
            | ActionType::Bet { amount, .. } => running_pot += amount.amount,
            ActionType::Raise { to, .. } => running_pot += to.amount,
            ActionType::UncalledBet { amount } => running_pot -= amount.amount,
            _ => {}
        }

        // Check if this is a postflop bet/raise by player
        if action.player == player
            && action.street != Street::Preflop
            && action.street != Street::Showdown
        {
            match &action.action_type {
                ActionType::Bet { amount, .. } => {
                    total += 1;
                    // Pot before this bet
                    let pot_before = running_pot - amount.amount;
                    if pot_before > 0.0 && amount.amount > pot_before {
                        overbets += 1;
                    }
                }
                ActionType::Raise {
                    amount: raise_by,
                    to,
                    ..
                } => {
                    total += 1;
                    let pot_before = running_pot - to.amount;
                    if pot_before > 0.0 && raise_by.amount > pot_before {
                        overbets += 1;
                    }
                }
                _ => {}
            }
        }
    }

    (total, overbets)
}

/// WWSF: did player win money in a hand where they saw the flop?
/// Returns (saw_flop, won_money).
pub(crate) fn wwsf_analysis(hand: &Hand, player: &str) -> (bool, bool) {
    let saw_flop = hero_saw_street(hand, player, Street::Flop);
    if !saw_flop {
        return (false, false);
    }
    let won = hand.result.winners.iter().any(|w| w.player == player);
    (true, won)
}

#[cfg(test)]
mod tests {
    use super::super::calculate::calculate_stats;
    use super::super::test_helpers::*;
    use crate::types::*;

    #[test]
    fn test_donk_bet_flop() {
        // Villain raises (PFA), Hero calls from BB (OOP). Flop: Hero bets (donk).
        let mut hand = sixmax_hand();
        hand.players[0].position = Some(Position::BB);
        hand.hero_position = Some(Position::BB);
        hand.players[2].position = Some(Position::BTN);
        hand.actions = vec![
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::PostSmallBlind {
                    amount: make_money(0.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::PostBigBlind {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(2.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            // Flop: Hero donks
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(4.00),
                    all_in: false,
                },
                street: Street::Flop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Fold,
                street: Street::Flop,
            },
        ];
        hand.board = vec![
            make_card('T', 'h'),
            make_card('5', 'd'),
            make_card('2', 'c'),
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.donk_bet_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_donk_bet_turn() {
        // Villain raises (PFA), Hero calls from BB (OOP). Flop: both check. Turn: Hero bets (donk).
        let mut hand = sixmax_hand();
        hand.players[0].position = Some(Position::BB);
        hand.hero_position = Some(Position::BB);
        hand.players[2].position = Some(Position::BTN);
        hand.actions = vec![
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::PostSmallBlind {
                    amount: make_money(0.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::PostBigBlind {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(2.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            // Flop: both check
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Check,
                street: Street::Flop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Check,
                street: Street::Flop,
            },
            // Turn: Hero donks
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(4.00),
                    all_in: false,
                },
                street: Street::Turn,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Fold,
                street: Street::Turn,
            },
        ];
        hand.board = vec![
            make_card('T', 'h'),
            make_card('5', 'd'),
            make_card('2', 'c'),
            make_card('7', 's'),
        ];

        let stats = calculate_stats(&[hand], "Hero");
        // Flop: hero checked (not a donk), Turn: hero bet (donk) → 1 donk out of 2 opp
        assert!(stats.donk_bet_pct > 0.0);
    }

    #[test]
    fn test_float_bet() {
        // Villain bets flop, Hero calls IP. Turn: Villain checks, Hero bets (float).
        let mut hand = sixmax_hand();
        hand.actions = vec![
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::PostSmallBlind {
                    amount: make_money(0.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::PostBigBlind {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            // Flop: Villain bets, Hero calls (IP since BTN > SB)
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(4.00),
                    all_in: false,
                },
                street: Street::Flop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(4.00),
                    all_in: false,
                },
                street: Street::Flop,
            },
            // Turn: Villain checks, Hero bets (float)
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Check,
                street: Street::Turn,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(8.00),
                    all_in: false,
                },
                street: Street::Turn,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Fold,
                street: Street::Turn,
            },
        ];
        hand.board = vec![
            make_card('T', 'h'),
            make_card('5', 'd'),
            make_card('2', 'c'),
            make_card('7', 's'),
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.float_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_float_bet_river() {
        // Villain bets turn, Hero calls IP. River: Villain checks, Hero bets (float).
        let mut hand = sixmax_hand();
        hand.actions = vec![
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::PostSmallBlind {
                    amount: make_money(0.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::PostBigBlind {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            // Flop: both check
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Check,
                street: Street::Flop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Check,
                street: Street::Flop,
            },
            // Turn: Villain bets, Hero calls
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(4.00),
                    all_in: false,
                },
                street: Street::Turn,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(4.00),
                    all_in: false,
                },
                street: Street::Turn,
            },
            // River: Villain checks, Hero bets (float)
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Check,
                street: Street::River,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(8.00),
                    all_in: false,
                },
                street: Street::River,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Fold,
                street: Street::River,
            },
        ];
        hand.board = vec![
            make_card('T', 'h'),
            make_card('5', 'd'),
            make_card('2', 'c'),
            make_card('7', 's'),
            make_card('3', 'h'),
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.float_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_check_raise() {
        // Hero checks flop, Villain bets, Hero raises.
        let mut hand = sixmax_hand();
        hand.players[0].position = Some(Position::BB);
        hand.hero_position = Some(Position::BB);
        hand.players[2].position = Some(Position::BTN);
        hand.actions = vec![
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::PostSmallBlind {
                    amount: make_money(0.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::PostBigBlind {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(2.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            // Flop: Hero checks, Villain bets, Hero raises (check-raise)
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Check,
                street: Street::Flop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(4.00),
                    all_in: false,
                },
                street: Street::Flop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(4.00),
                    to: make_money(12.00),
                    all_in: false,
                },
                street: Street::Flop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Fold,
                street: Street::Flop,
            },
        ];
        hand.board = vec![
            make_card('T', 'h'),
            make_card('5', 'd'),
            make_card('2', 'c'),
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.check_raise_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_probe_bet() {
        // Villain is PFA, checks flop. Turn: Hero bets (probe).
        let mut hand = sixmax_hand();
        hand.players[0].position = Some(Position::BB);
        hand.hero_position = Some(Position::BB);
        hand.players[2].position = Some(Position::BTN);
        hand.actions = vec![
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::PostSmallBlind {
                    amount: make_money(0.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::PostBigBlind {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(2.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            // Flop: both check (PFA checks behind)
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Check,
                street: Street::Flop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Check,
                street: Street::Flop,
            },
            // Turn: Hero bets (probe since PFA checked behind)
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(4.00),
                    all_in: false,
                },
                street: Street::Turn,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Fold,
                street: Street::Turn,
            },
        ];
        hand.board = vec![
            make_card('T', 'h'),
            make_card('5', 'd'),
            make_card('2', 'c'),
            make_card('7', 's'),
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.probe_bet_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_wwsf() {
        // Hero sees the flop and wins.
        let mut hand = sixmax_hand();
        hand.actions = vec![
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::PostSmallBlind {
                    amount: make_money(0.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::PostBigBlind {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(2.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            // Flop
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Check,
                street: Street::Flop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(4.00),
                    all_in: false,
                },
                street: Street::Flop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Fold,
                street: Street::Flop,
            },
        ];
        hand.board = vec![
            make_card('T', 'h'),
            make_card('5', 'd'),
            make_card('2', 'c'),
        ];
        hand.result = HandResult {
            winners: vec![Winner {
                player: "Hero".to_string(),
                amount: make_money(6.00),
                pot: "Main pot".to_string(),
            }],
            hero_result: HeroResult::Won,
        };

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.wwsf - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_overbet() {
        // Hero bets more than the pot on the flop.
        let mut hand = sixmax_hand();
        hand.actions = vec![
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::PostSmallBlind {
                    amount: make_money(0.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::PostBigBlind {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(3.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(2.50),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            // Pot going into flop = 3.00 + 3.00 = 6.00 (approx, SB+BB+raises)
            // Actually: SB 0.50 + BB 1.00 + Hero raise to 3.00 + Villain call 2.50 = 7.00
            // Hero bets 10.00 (overbet: 10 > 7)
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Check,
                street: Street::Flop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Bet {
                    amount: make_money(10.00),
                    all_in: false,
                },
                street: Street::Flop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Fold,
                street: Street::Flop,
            },
        ];
        hand.board = vec![
            make_card('T', 'h'),
            make_card('5', 'd'),
            make_card('2', 'c'),
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.overbet_pct - 100.0).abs() < 0.01);
    }
}
