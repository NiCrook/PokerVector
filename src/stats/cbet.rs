use crate::types::*;
use super::helpers::{find_preflop_aggressor, hero_saw_street};

pub(crate) struct CbetResult {
    /// (opportunity, did_cbet) for flop
    pub flop: (bool, bool),
    /// (opportunity, did_cbet) for turn
    pub turn: (bool, bool),
    /// (opportunity, did_fold) for facing flop cbet
    pub fold_to_flop: (bool, bool),
    /// (opportunity, did_fold) for facing turn cbet
    pub fold_to_turn: (bool, bool),
}

pub(crate) fn cbet_analysis(hand: &Hand, player: &str) -> CbetResult {
    let pfa = find_preflop_aggressor(hand);
    let is_pfa = pfa.as_deref() == Some(player);

    let mut result = CbetResult {
        flop: (false, false),
        turn: (false, false),
        fold_to_flop: (false, false),
        fold_to_turn: (false, false),
    };

    // C-bet: player is PFA, first bet on the street
    if is_pfa {
        // Flop c-bet
        if hero_saw_street(hand, player, Street::Flop) {
            result.flop.0 = true; // opportunity
            let first_aggressor_on_flop = hand.actions.iter()
                .find(|a| a.street == Street::Flop && matches!(a.action_type, ActionType::Bet { .. }));
            if let Some(a) = first_aggressor_on_flop {
                if a.player == player {
                    result.flop.1 = true; // did c-bet flop
                    // Turn c-bet: must have c-bet flop and see turn
                    if hero_saw_street(hand, player, Street::Turn) {
                        result.turn.0 = true;
                        let first_aggressor_on_turn = hand.actions.iter()
                            .find(|a| a.street == Street::Turn && matches!(a.action_type, ActionType::Bet { .. }));
                        if let Some(a) = first_aggressor_on_turn {
                            if a.player == player {
                                result.turn.1 = true;
                            }
                        }
                    }
                }
            }
        }
    }

    // Fold to c-bet: player is NOT PFA, PFA bets, player faces it
    if !is_pfa {
        if let Some(ref pfa_name) = pfa {
            // Flop: did PFA c-bet the flop?
            if hero_saw_street(hand, player, Street::Flop) {
                let first_bet_flop = hand.actions.iter()
                    .find(|a| a.street == Street::Flop && matches!(a.action_type, ActionType::Bet { .. }));
                if let Some(bet_action) = first_bet_flop {
                    if bet_action.player == *pfa_name {
                        // PFA c-bet flop. Did player face it? (player must have action after this bet)
                        let player_response = hand.actions.iter()
                            .skip_while(|a| !(a.street == Street::Flop && a.player == *pfa_name && matches!(a.action_type, ActionType::Bet { .. })))
                            .skip(1) // skip the bet itself
                            .find(|a| a.street == Street::Flop && a.player == player);
                        if let Some(resp) = player_response {
                            result.fold_to_flop.0 = true;
                            if matches!(resp.action_type, ActionType::Fold) {
                                result.fold_to_flop.1 = true;
                            }
                        }
                    }
                }
            }

            // Turn: did PFA c-bet turn (after c-betting flop)?
            if hero_saw_street(hand, player, Street::Turn) {
                // Check PFA c-bet flop first
                let pfa_cbet_flop = hand.actions.iter()
                    .find(|a| a.street == Street::Flop && matches!(a.action_type, ActionType::Bet { .. }) && a.player == *pfa_name)
                    .is_some();
                if pfa_cbet_flop {
                    let first_bet_turn = hand.actions.iter()
                        .find(|a| a.street == Street::Turn && matches!(a.action_type, ActionType::Bet { .. }));
                    if let Some(bet_action) = first_bet_turn {
                        if bet_action.player == *pfa_name {
                            let player_response = hand.actions.iter()
                                .skip_while(|a| !(a.street == Street::Turn && a.player == *pfa_name && matches!(a.action_type, ActionType::Bet { .. })))
                                .skip(1)
                                .find(|a| a.street == Street::Turn && a.player == player);
                            if let Some(resp) = player_response {
                                result.fold_to_turn.0 = true;
                                if matches!(resp.action_type, ActionType::Fold) {
                                    result.fold_to_turn.1 = true;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::super::calculate::calculate_stats;
    use super::super::test_helpers::*;
    use crate::types::*;

    #[test]
    fn test_cbet_flop() {
        // Hero raises preflop (PFA), villain calls. Flop: Hero bets (c-bet).
        let mut hand = sixmax_hand();
        hand.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::PostSmallBlind { amount: make_money(0.50), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::PostBigBlind { amount: make_money(1.00), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(1.00), to: make_money(3.00), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(2.50), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            // Flop
            Action { player: "Villain".to_string(), action_type: ActionType::Check, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::Bet { amount: make_money(4.00), all_in: false }, street: Street::Flop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Flop },
        ];
        hand.board = vec![make_card('T', 'h'), make_card('5', 'd'), make_card('2', 'c')];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.cbet_flop - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_cbet_turn() {
        // Hero raises preflop, cbets flop, then cbets turn.
        let mut hand = sixmax_hand();
        hand.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::PostSmallBlind { amount: make_money(0.50), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::PostBigBlind { amount: make_money(1.00), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(1.00), to: make_money(3.00), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(2.50), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            // Flop: c-bet
            Action { player: "Villain".to_string(), action_type: ActionType::Check, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::Bet { amount: make_money(4.00), all_in: false }, street: Street::Flop },
            Action { player: "Villain".to_string(), action_type: ActionType::Call { amount: make_money(4.00), all_in: false }, street: Street::Flop },
            // Turn: c-bet
            Action { player: "Villain".to_string(), action_type: ActionType::Check, street: Street::Turn },
            Action { player: "Hero".to_string(), action_type: ActionType::Bet { amount: make_money(8.00), all_in: false }, street: Street::Turn },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Turn },
        ];
        hand.board = vec![make_card('T', 'h'), make_card('5', 'd'), make_card('2', 'c'), make_card('7', 's')];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.cbet_flop - 100.0).abs() < 0.01);
        assert!((stats.cbet_turn - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_fold_to_cbet() {
        // Villain raises (PFA), Hero calls. Flop: Villain bets (c-bet), Hero folds.
        let mut hand = sixmax_hand();
        hand.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::PostSmallBlind { amount: make_money(0.50), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::PostBigBlind { amount: make_money(1.00), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: make_money(1.00), to: make_money(3.00), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: make_money(3.00), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            // Flop: PFA c-bets, hero folds
            Action { player: "Villain".to_string(), action_type: ActionType::Bet { amount: make_money(4.00), all_in: false }, street: Street::Flop },
            Action { player: "Hero".to_string(), action_type: ActionType::Fold, street: Street::Flop },
        ];
        hand.board = vec![make_card('T', 'h'), make_card('5', 'd'), make_card('2', 'c')];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.fold_to_cbet_flop - 100.0).abs() < 0.01);
    }
}
