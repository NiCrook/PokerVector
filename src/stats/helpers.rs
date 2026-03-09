use crate::types::*;

pub(crate) fn big_blind_size(hand: &Hand) -> f64 {
    match &hand.game_type {
        GameType::Cash { big_blind, .. } => big_blind.amount,
        GameType::Tournament { big_blind, .. } => big_blind.amount,
    }
}

/// Check if the player saw a given street (had actions on it or a later street).
pub(crate) fn hero_saw_street(hand: &Hand, hero: &str, street: Street) -> bool {
    let dominated_streets = match street {
        Street::Flop => &[Street::Flop, Street::Turn, Street::River, Street::Showdown][..],
        Street::Turn => &[Street::Turn, Street::River, Street::Showdown][..],
        Street::River => &[Street::River, Street::Showdown][..],
        _ => return true,
    };

    // If there are actions on this street or later, hero saw it unless they folded before
    let hand_reached_street = hand
        .actions
        .iter()
        .any(|a| dominated_streets.contains(&a.street));
    if !hand_reached_street {
        return false;
    }

    // Hero must not have folded before this street
    !hand.actions.iter().any(|a| {
        a.player == hero
            && matches!(a.action_type, ActionType::Fold)
            && street_order(a.street) < street_order(street)
    })
}

pub(crate) fn hero_folded_before_showdown(hand: &Hand, hero: &str) -> bool {
    hand.actions
        .iter()
        .any(|a| a.player == hero && matches!(a.action_type, ActionType::Fold))
}

pub(crate) fn street_order(street: Street) -> u8 {
    match street {
        Street::Preflop => 0,
        Street::Flop => 1,
        Street::Turn => 2,
        Street::River => 3,
        Street::ThirdStreet => 0,
        Street::FourthStreet => 1,
        Street::FifthStreet => 2,
        Street::SixthStreet => 3,
        Street::SeventhStreet => 4,
        Street::Showdown => 5,
    }
}

/// Total amount hero invested in a hand (blinds + calls + bets + raises).
pub(crate) fn hero_invested(hand: &Hand, hero: &str) -> f64 {
    let mut total = 0.0;
    for action in &hand.actions {
        if action.player != hero {
            continue;
        }
        match &action.action_type {
            ActionType::PostSmallBlind { amount, .. }
            | ActionType::PostBigBlind { amount, .. }
            | ActionType::PostAnte { amount }
            | ActionType::PostBlind { amount }
            | ActionType::BringsIn { amount } => total += amount.amount,
            ActionType::Call { amount, .. } => total += amount.amount,
            ActionType::Bet { amount, .. } => total += amount.amount,
            ActionType::Raise { to, .. } => total += to.amount,
            ActionType::UncalledBet { amount } => total -= amount.amount,
            _ => {}
        }
    }
    total
}

/// Total amount hero collected from pots.
pub(crate) fn hero_collected(hand: &Hand, hero: &str) -> f64 {
    hand.result
        .winners
        .iter()
        .filter(|w| w.player == hero)
        .map(|w| w.amount.amount)
        .sum()
}

/// Find the last player to raise preflop (the preflop aggressor).
pub(crate) fn find_preflop_aggressor(hand: &Hand) -> Option<String> {
    let mut last_raiser: Option<String> = None;
    for action in &hand.actions {
        if action.street != Street::Preflop {
            continue;
        }
        if matches!(
            action.action_type,
            ActionType::Raise { .. } | ActionType::Bet { .. }
        ) {
            last_raiser = Some(action.player.clone());
        }
    }
    last_raiser
}

/// Position ordering for IP/OOP determination. Higher = more in position.
/// BTN > CO > HJ > LJ > MP2 > MP1 > UTG > SB > BB
pub(crate) fn position_order_pos(pos: Position) -> u8 {
    match pos {
        Position::BB => 0,
        Position::SB => 1,
        Position::UTG => 2,
        Position::MP1 => 3,
        Position::MP2 => 4,
        Position::LJ => 5,
        Position::HJ => 6,
        Position::CO => 7,
        Position::BTN => 8,
    }
}

/// Returns true if player_a is in position relative to player_b (acts later postflop).
pub(crate) fn is_player_ip(hand: &Hand, player_a: &str, player_b: &str) -> bool {
    let pos_a = hand
        .players
        .iter()
        .find(|p| p.name == player_a)
        .and_then(|p| p.position);
    let pos_b = hand
        .players
        .iter()
        .find(|p| p.name == player_b)
        .and_then(|p| p.position);
    match (pos_a, pos_b) {
        (Some(a), Some(b)) => position_order_pos(a) > position_order_pos(b),
        _ => false,
    }
}

/// Estimate the pot size at the start of a given street by summing all bets/calls/blinds before it.
pub(crate) fn estimate_pot_at_street(hand: &Hand, street: Street) -> f64 {
    let target_order = street_order(street);
    let mut pot = 0.0;
    for action in &hand.actions {
        if street_order(action.street) >= target_order {
            break;
        }
        match &action.action_type {
            ActionType::PostSmallBlind { amount, .. }
            | ActionType::PostBigBlind { amount, .. }
            | ActionType::PostAnte { amount }
            | ActionType::PostBlind { amount }
            | ActionType::Call { amount, .. }
            | ActionType::Bet { amount, .. } => pot += amount.amount,
            ActionType::Raise { to, .. } => pot += to.amount,
            ActionType::UncalledBet { amount } => pot -= amount.amount,
            _ => {}
        }
    }
    // Also add actions on the current street up to any bet/raise we're measuring
    // Actually, we want pot at the START of the street, so stop before it
    pot
}
