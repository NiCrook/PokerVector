use crate::types::*;

/// Check if a player voluntarily put money in preflop (VPIP) and/or raised preflop (PFR).
pub(crate) fn preflop_vpip_pfr(hand: &Hand, player: &str) -> (bool, bool) {
    let mut is_vpip = false;
    let mut is_pfr = false;

    for action in &hand.actions {
        if action.street != Street::Preflop || action.player != player {
            continue;
        }
        match &action.action_type {
            ActionType::Call { .. } => is_vpip = true,
            ActionType::Bet { .. } | ActionType::Raise { .. } => {
                is_vpip = true;
                is_pfr = true;
            }
            _ => {}
        }
    }

    (is_vpip, is_pfr)
}

/// Determine if hero had a 3-bet opportunity and whether they took it.
/// A 3-bet opportunity exists when another player raises preflop and hero hasn't yet acted aggressively.
pub(crate) fn three_bet_analysis(hand: &Hand, hero: &str) -> (bool, bool) {
    let mut raise_count = 0u32;
    let mut hero_had_opportunity = false;
    let mut hero_three_bet = false;

    for action in &hand.actions {
        if action.street != Street::Preflop {
            continue;
        }
        match &action.action_type {
            ActionType::Raise { .. } | ActionType::Bet { .. } => {
                if action.player == hero {
                    if raise_count == 1 {
                        // Hero is making the second raise = 3-bet
                        hero_three_bet = true;
                    }
                    break; // Hero acted aggressively, no more opportunity
                }
                raise_count += 1;
            }
            ActionType::Fold | ActionType::Call { .. } if action.player == hero => {
                if raise_count == 1 {
                    // Hero faced a raise and chose not to 3-bet
                    hero_had_opportunity = true;
                }
                break;
            }
            _ => {}
        }
    }

    if hero_three_bet {
        (true, true)
    } else {
        (hero_had_opportunity, false)
    }
}

/// Determine if hero raised, faced a 3-bet, and then folded.
pub(crate) fn fold_to_three_bet_analysis(hand: &Hand, hero: &str) -> (bool, bool) {
    let mut raise_count = 0u32;
    let mut hero_raised = false;
    let mut hero_faced_3bet = false;

    for action in &hand.actions {
        if action.street != Street::Preflop {
            continue;
        }
        match &action.action_type {
            ActionType::Raise { .. } | ActionType::Bet { .. } => {
                if action.player == hero {
                    if !hero_raised {
                        hero_raised = true;
                        raise_count += 1;
                    } else {
                        // Hero re-raised (4-bet), didn't fold
                        return (true, false);
                    }
                } else {
                    raise_count += 1;
                    if hero_raised && raise_count >= 2 {
                        hero_faced_3bet = true;
                    }
                }
            }
            ActionType::Fold if action.player == hero && hero_faced_3bet => {
                return (true, true);
            }
            ActionType::Call { .. } if action.player == hero && hero_faced_3bet => {
                return (true, false);
            }
            _ => {}
        }
    }

    (hero_faced_3bet, false)
}

/// Squeeze: 3-bet preflop when there's been a raise AND one or more callers.
/// Returns (had_opportunity, did_squeeze).
pub(crate) fn squeeze_analysis(hand: &Hand, player: &str) -> (bool, bool) {
    let mut raise_count = 0u32;
    let mut callers_after_raise = 0u32;

    for action in &hand.actions {
        if action.street != Street::Preflop {
            continue;
        }
        match &action.action_type {
            ActionType::Raise { .. } | ActionType::Bet { .. } => {
                if action.player == player {
                    // Player raises — is this a squeeze?
                    if raise_count == 1 && callers_after_raise >= 1 {
                        return (true, true);
                    }
                    return (false, false); // Open raise or other scenario
                }
                raise_count += 1;
                callers_after_raise = 0; // Reset callers for new raise level
            }
            ActionType::Call { .. } => {
                if action.player == player {
                    // Player called instead of squeezing
                    if raise_count == 1 && callers_after_raise >= 1 {
                        return (true, false);
                    }
                    return (false, false);
                }
                if raise_count >= 1 {
                    callers_after_raise += 1;
                }
            }
            ActionType::Fold if action.player == player => {
                if raise_count == 1 && callers_after_raise >= 1 {
                    return (true, false);
                }
                return (false, false);
            }
            _ => {}
        }
    }

    (false, false)
}

/// Cold call: call a raise preflop without having put money in (excluding blinds/antes).
/// Returns (had_opportunity, did_cold_call).
pub(crate) fn cold_call_analysis(hand: &Hand, player: &str) -> (bool, bool) {
    let mut raise_seen = false;

    for action in &hand.actions {
        if action.street != Street::Preflop {
            continue;
        }
        if matches!(action.action_type, ActionType::PostSmallBlind { .. } | ActionType::PostBigBlind { .. } | ActionType::PostAnte { .. } | ActionType::PostBlind { .. }) {
            continue;
        }

        if matches!(action.action_type, ActionType::Raise { .. } | ActionType::Bet { .. }) {
            if action.player == player {
                return (false, false); // Player raised first — not a cold call scenario
            }
            raise_seen = true;
        }

        if action.player == player && raise_seen {
            match &action.action_type {
                ActionType::Call { .. } => return (true, true),
                ActionType::Fold => return (true, false),
                ActionType::Raise { .. } => return (true, false), // 3-bet, not cold call
                _ => {}
            }
        }
    }

    (false, false)
}

#[cfg(test)]
mod tests {
    use super::super::calculate::calculate_stats;
    use super::super::test_helpers::*;
    use crate::types::*;

    #[test]
    fn test_three_bet_percentage() {
        // Hand 1: Villain raises, hero 3-bets
        let mut h1 = base_hand();
        h1.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.06), to: make_money(0.18), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];
        h1.result = HandResult { winners: vec![Winner { player: "Hero".to_string(), amount: make_money(0.09), pot: "Main pot".to_string() }], hero_result: HeroResult::Won };

        // Hand 2: Villain raises, hero calls (had opportunity but didn't 3-bet)
        let mut h2 = base_hand();
        h2.id = 2;
        h2.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: make_money(0.06), all_in: false }, street: Street::Preflop },
        ];

        let stats = calculate_stats(&[h1, h2], "Hero");
        // 1 out of 2 opportunities = 50%
        assert!((stats.three_bet_pct - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_fold_to_three_bet() {
        // Hero raises, villain 3-bets, hero folds
        let mut h1 = base_hand();
        h1.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(0.02), to: make_money(0.06), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Raise { amount: make_money(0.06), to: make_money(0.18), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];

        let stats = calculate_stats(&[h1], "Hero");
        assert!((stats.fold_to_three_bet - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_squeeze() {
        // CO opens, BTN calls, Hero (SB) 3-bets = squeeze.
        let mut hand = sixmax_hand();
        hand.players[0].position = Some(Position::SB);
        hand.hero_position = Some(Position::SB);
        hand.players[1].position = Some(Position::BB);
        hand.players[2].position = Some(Position::BTN);
        hand.actions = vec![
            Action { player: "Hero".to_string(), action_type: ActionType::PostSmallBlind { amount: make_money(0.50), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::PostBigBlind { amount: make_money(1.00), all_in: false }, street: Street::Preflop },
            Action { player: "CO_Player".to_string(), action_type: ActionType::Raise { amount: make_money(1.00), to: make_money(2.50), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Call { amount: make_money(2.50), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Raise { amount: make_money(2.50), to: make_money(10.00), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "CO_Player".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.squeeze_pct - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_cold_call() {
        // Villain raises, Hero calls (cold call — no prior action from Hero).
        let mut hand = sixmax_hand();
        hand.actions = vec![
            Action { player: "Villain".to_string(), action_type: ActionType::PostSmallBlind { amount: make_money(0.50), all_in: false }, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::PostBigBlind { amount: make_money(1.00), all_in: false }, street: Street::Preflop },
            Action { player: "LJ_Player".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "HJ_Player".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "CO_Player".to_string(), action_type: ActionType::Raise { amount: make_money(1.00), to: make_money(2.50), all_in: false }, street: Street::Preflop },
            Action { player: "Hero".to_string(), action_type: ActionType::Call { amount: make_money(2.50), all_in: false }, street: Street::Preflop },
            Action { player: "Villain".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
            Action { player: "Fish".to_string(), action_type: ActionType::Fold, street: Street::Preflop },
        ];

        let stats = calculate_stats(&[hand], "Hero");
        assert!((stats.cold_call_pct - 100.0).abs() < 0.01);
    }
}
