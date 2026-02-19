use crate::types::*;

pub(crate) struct StealResult {
    /// (opportunity, did_steal) for player attempting a steal
    pub steal: (bool, bool),
    /// (opportunity, did_fold) for player in BB facing a steal
    pub fold_bb: (bool, bool),
    /// (opportunity, did_fold) for player in SB facing a steal
    pub fold_sb: (bool, bool),
}

pub(crate) fn steal_analysis(hand: &Hand, player: &str) -> StealResult {
    let mut result = StealResult {
        steal: (false, false),
        fold_bb: (false, false),
        fold_sb: (false, false),
    };

    let player_pos = hand.players.iter().find(|p| p.name == player).and_then(|p| p.position);

    // Steal: open raise from CO/BTN/SB when folded to
    let steal_positions = [Position::CO, Position::BTN, Position::SB];
    if let Some(pos) = player_pos {
        if steal_positions.contains(&pos) {
            // Check if folded to player preflop
            let mut folded_to_player = true;
            for action in &hand.actions {
                if action.street != Street::Preflop {
                    continue;
                }
                if action.player == player {
                    break;
                }
                match &action.action_type {
                    ActionType::Call { .. } | ActionType::Raise { .. } | ActionType::Bet { .. } => {
                        folded_to_player = false;
                        break;
                    }
                    _ => {}
                }
            }
            if folded_to_player {
                result.steal.0 = true;
                // Did player raise?
                let player_action = hand.actions.iter()
                    .find(|a| a.street == Street::Preflop && a.player == player
                        && !matches!(a.action_type, ActionType::PostSmallBlind { .. } | ActionType::PostBigBlind { .. } | ActionType::PostAnte { .. } | ActionType::PostBlind { .. }));
                if let Some(a) = player_action {
                    if matches!(a.action_type, ActionType::Raise { .. } | ActionType::Bet { .. }) {
                        result.steal.1 = true;
                    }
                }
            }
        }
    }

    // Fold to steal (BB/SB perspective): someone in steal position open-raises, player is in BB or SB
    if let Some(pos) = player_pos {
        if pos == Position::BB || pos == Position::SB {
            // Find the first voluntary preflop action
            let first_raise = hand.actions.iter().find(|a| {
                a.street == Street::Preflop
                    && matches!(a.action_type, ActionType::Raise { .. } | ActionType::Bet { .. })
            });
            if let Some(raise_action) = first_raise {
                // Is this an open-raise from a steal position?
                let raiser_pos = hand.players.iter()
                    .find(|p| p.name == raise_action.player)
                    .and_then(|p| p.position);
                if let Some(rp) = raiser_pos {
                    if steal_positions.contains(&rp) {
                        // Check it was folded to the raiser
                        let mut folded_to_raiser = true;
                        for action in &hand.actions {
                            if action.street != Street::Preflop {
                                continue;
                            }
                            if action.player == raise_action.player {
                                break;
                            }
                            match &action.action_type {
                                ActionType::Call { .. } | ActionType::Raise { .. } | ActionType::Bet { .. } => {
                                    folded_to_raiser = false;
                                    break;
                                }
                                _ => {}
                            }
                        }
                        if folded_to_raiser {
                            // Player faces a steal
                            let player_response = hand.actions.iter()
                                .filter(|a| a.street == Street::Preflop && a.player == player)
                                .find(|a| !matches!(a.action_type,
                                    ActionType::PostSmallBlind { .. } | ActionType::PostBigBlind { .. }
                                    | ActionType::PostAnte { .. } | ActionType::PostBlind { .. }));
                            if let Some(resp) = player_response {
                                if pos == Position::BB {
                                    result.fold_bb.0 = true;
                                    if matches!(resp.action_type, ActionType::Fold) {
                                        result.fold_bb.1 = true;
                                    }
                                } else {
                                    result.fold_sb.0 = true;
                                    if matches!(resp.action_type, ActionType::Fold) {
                                        result.fold_sb.1 = true;
                                    }
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
    fn test_steal_from_btn() {
        // Hero on BTN, folded to Hero, Hero raises = steal
        let mut hand = sixmax_hand();
        hand.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::PostSmallBlind { amount: make_money(0.50), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::PostBigBlind { amount: make_money(1.00), all_in: false }, street: Street::Preflop },
            Action { player: "LJ_Player".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "HJ_Player".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "CO_Player".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(1.00), to: make_money(2.50), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.steal_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_fold_to_steal_bb() {
        // Hero is BB. CO opens (steal), SB folds, Hero folds.
        let mut hand = sixmax_hand();
        hand.players[0].position = Some(Position::BB);
        hand.players[0].seat = 3;
        hand.players[2].position = Some(Position::BTN);
        hand.players[2].seat = 1;
        hand.hero_position = Some(Position::BB);
        hand.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::PostSmallBlind { amount: make_money(0.50), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::PostBigBlind { amount: make_money(1.00), all_in: false }, street: Street::Preflop },
            Action { player: "LJ_Player".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "HJ_Player".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "CO_Player".to_string(), action_type: ActionType::Raise { amount: make_money(1.00), to: make_money(2.50), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.fold_to_steal_bb - 100.0).abs() < 0.01);
    }
}
