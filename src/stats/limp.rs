use crate::types::*;

pub(crate) struct LimpResult {
    /// (opportunity to open, did_limp)
    pub limp: (bool, bool),
    /// (limped and faced raise, did_call)
    pub limp_call: (bool, bool),
    /// (limped and faced raise, did_fold)
    pub limp_fold: (bool, bool),
    /// (limped and faced raise, did_raise)
    pub limp_raise: (bool, bool),
}

pub(crate) fn limp_analysis(hand: &Hand, player: &str) -> LimpResult {
    let mut result = LimpResult {
        limp: (false, false),
        limp_call: (false, false),
        limp_fold: (false, false),
        limp_raise: (false, false),
    };

    // Check if folded to player preflop (no raises before them)
    let mut folded_to = true;
    let mut player_is_blind = false;

    // Check if player is SB or BB (blinds can't "open limp" in the traditional sense, but SB completing counts)
    let player_pos = hand
        .players
        .iter()
        .find(|p| p.name == player)
        .and_then(|p| p.position);
    if let Some(pos) = player_pos {
        if pos == Position::BB {
            return result; // BB can't open limp
        }
        if pos == Position::SB {
            player_is_blind = true;
        }
    }

    for action in &hand.actions {
        if action.street != Street::Preflop {
            continue;
        }
        if action.player == player {
            if matches!(
                action.action_type,
                ActionType::PostSmallBlind { .. }
                    | ActionType::PostBigBlind { .. }
                    | ActionType::PostAnte { .. }
                    | ActionType::PostBlind { .. }
            ) {
                continue;
            }
            break;
        }
        match &action.action_type {
            ActionType::Call { .. } | ActionType::Raise { .. } | ActionType::Bet { .. } => {
                folded_to = false;
                break;
            }
            _ => {}
        }
    }

    if !folded_to {
        return result;
    }

    // Player had the opportunity to open
    result.limp.0 = true;

    // Find player's first voluntary action
    let first_action = hand.actions.iter().find(|a| {
        a.street == Street::Preflop
            && a.player == player
            && !matches!(
                a.action_type,
                ActionType::PostSmallBlind { .. }
                    | ActionType::PostBigBlind { .. }
                    | ActionType::PostAnte { .. }
                    | ActionType::PostBlind { .. }
            )
    });

    if let Some(action) = first_action {
        let did_limp = if player_is_blind {
            // SB completing = calling without raising
            matches!(action.action_type, ActionType::Call { .. })
        } else {
            matches!(action.action_type, ActionType::Call { .. })
        };

        if did_limp {
            result.limp.1 = true;

            // Did someone raise after the limp?
            let raise_after_limp = hand
                .actions
                .iter()
                .skip_while(|a| {
                    !(a.street == Street::Preflop
                        && a.player == player
                        && matches!(a.action_type, ActionType::Call { .. }))
                })
                .skip(1)
                .any(|a| {
                    a.street == Street::Preflop
                        && a.player != player
                        && matches!(
                            a.action_type,
                            ActionType::Raise { .. } | ActionType::Bet { .. }
                        )
                });

            if raise_after_limp {
                // Find player's response to the raise
                let response = hand
                    .actions
                    .iter()
                    .skip_while(|a| {
                        !(a.street == Street::Preflop
                            && a.player == player
                            && matches!(a.action_type, ActionType::Call { .. }))
                    })
                    .skip(1)
                    .find(|a| a.street == Street::Preflop && a.player == player);

                result.limp_call.0 = true;
                result.limp_fold.0 = true;
                result.limp_raise.0 = true;

                if let Some(resp) = response {
                    match &resp.action_type {
                        ActionType::Call { .. } => result.limp_call.1 = true,
                        ActionType::Fold => result.limp_fold.1 = true,
                        ActionType::Raise { .. } | ActionType::Bet { .. } => {
                            result.limp_raise.1 = true
                        }
                        _ => {}
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
    fn test_limp_and_limp_call() {
        // Hero on BTN, folded to, limps. Villain raises, Hero calls.
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
                player: "LJ_Player".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            Action {
                player: "HJ_Player".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            Action {
                player: "CO_Player".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(4.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::Fold,
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
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.limp_pct - 100.0).abs() < 0.01);
        assert!((stats.limp_call - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_limp_fold() {
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
                player: "LJ_Player".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            Action {
                player: "HJ_Player".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            Action {
                player: "CO_Player".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Call {
                    amount: make_money(1.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Villain".to_string(),
                action_type: ActionType::Raise {
                    amount: make_money(1.00),
                    to: make_money(4.00),
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Fish".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Fold,
                street: Street::Preflop,
            },
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.limp_pct - 100.0).abs() < 0.01);
        assert!((stats.limp_fold - 100.0).abs() < 0.01);
    }
}
