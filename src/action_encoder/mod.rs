mod classify;
mod pot_tracker;

use std::collections::HashMap;

use crate::types::*;

use classify::{classify_action, format_classified_action, ActionLabel};
use pot_tracker::PotTracker;

/// Extract the big blind amount as f64 from a Hand.
fn big_blind_amount(hand: &Hand) -> f64 {
    match &hand.game_type {
        GameType::Cash { big_blind, .. } => big_blind.amount,
        GameType::Tournament { big_blind, .. } => big_blind.amount,
    }
}

/// Build a map from player name to anonymous alias (HERO, V1, V2, ...).
fn build_alias_map(hand: &Hand, hero: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    // Hero alias
    if let Some(ref h) = hand.hero {
        if h == hero {
            map.insert(h.clone(), "HERO".to_string());
        }
    }

    // Villains ordered by seat number
    let mut villains: Vec<&Player> = hand
        .players
        .iter()
        .filter(|p| !p.is_sitting_out && p.name != hero)
        .collect();
    villains.sort_by_key(|p| p.seat);

    for (i, v) in villains.iter().enumerate() {
        map.insert(v.name.clone(), format!("V{}", i + 1));
    }

    map
}

/// Format board cards visible at a given street.
fn format_board(board: &[Card], street: Street) -> String {
    match street {
        Street::Flop if board.len() >= 3 => {
            let cards: String = board[..3].iter().map(|c| c.to_string()).collect();
            format!("[{}]", cards)
        }
        Street::Turn if board.len() >= 4 => {
            format!("[{}]", board[3])
        }
        Street::River if board.len() >= 5 => {
            format!("[{}]", board[4])
        }
        _ => String::new(),
    }
}

/// Format a number with given decimal places, trimming trailing zeros and dot.
fn fmt_trimmed(val: f64, decimals: usize) -> String {
    let s = format!("{:.prec$}", val, prec = decimals);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.to_string()
}

/// Check if a street is a stud street.
fn is_stud_street(street: Street) -> bool {
    matches!(
        street,
        Street::ThirdStreet
            | Street::FourthStreet
            | Street::FifthStreet
            | Street::SixthStreet
            | Street::SeventhStreet
    )
}

/// Get the street label for encoding output.
fn street_label(street: Street) -> &'static str {
    match street {
        Street::Preflop => "PRE",
        Street::Flop => "FLOP",
        Street::Turn => "TURN",
        Street::River => "RIVER",
        Street::ThirdStreet => "3RD",
        Street::FourthStreet => "4TH",
        Street::FifthStreet => "5TH",
        Street::SixthStreet => "6TH",
        Street::SeventhStreet => "7TH",
        Street::Showdown => "SHOWDOWN",
    }
}

/// Encode a hand's action sequence into a structured text representation.
pub fn encode_action_sequence(hand: &Hand, hero: &str) -> String {
    // Sat out — single line
    if hand.result.hero_result == HeroResult::SatOut {
        return "SAT_OUT".to_string();
    }

    let bb = big_blind_amount(hand);
    let alias_map = build_alias_map(hand, hero);
    let is_stud = hand.variant == PokerVariant::SevenCardStud;

    let mut pot_tracker = PotTracker::new();
    let mut lines: Vec<String> = Vec::new();

    // Bomb pot prefix
    if hand.is_bomb_pot {
        lines.push("BOMB_POT".to_string());
    }

    // Determine street order
    let streets: &[Street] = if is_stud {
        &[
            Street::ThirdStreet,
            Street::FourthStreet,
            Street::FifthStreet,
            Street::SixthStreet,
            Street::SeventhStreet,
        ]
    } else {
        &[Street::Preflop, Street::Flop, Street::Turn, Street::River]
    };

    // Track preflop aggressor for c-bet detection
    let mut preflop_aggressor: Option<String> = None;

    for &street in streets {
        let street_actions: Vec<&Action> =
            hand.actions.iter().filter(|a| a.street == street).collect();

        if street_actions.is_empty() {
            continue;
        }

        // Signal new street to pot tracker (except first street)
        if street != streets[0] {
            pot_tracker.new_street();
        }

        let is_preflop = street == Street::Preflop;
        let mut raise_count: u32 = 0;
        let mut street_tokens: Vec<String> = Vec::new();
        let mut has_bet_on_street = false;

        for action in &street_actions {
            // Classify BEFORE updating pot tracker (so current_bet reflects pre-action state)
            let classified = classify_action(
                action,
                &alias_map,
                &pot_tracker,
                bb,
                is_preflop,
                is_stud_street(street),
                &mut raise_count,
                &mut has_bet_on_street,
                &preflop_aggressor,
            );

            if let Some(ca) = classified {
                // Track preflop aggressor
                if is_preflop {
                    match &ca.label {
                        ActionLabel::Open(_)
                        | ActionLabel::ThreeBet(_)
                        | ActionLabel::FourBet(_)
                        | ActionLabel::FiveBetPlus(_) => {
                            preflop_aggressor = Some(action.player.clone());
                        }
                        _ => {}
                    }
                }

                street_tokens.push(format_classified_action(&ca));
            }

            // Update pot tracker AFTER classification
            pot_tracker.process_action(action, hero);
        }

        if street_tokens.is_empty() {
            continue;
        }

        // Walk detection: hero is BB, won, and only folds happened preflop
        if is_preflop && is_walk(hand, hero, &street_actions) {
            let label = street_label(street);
            lines.push(format!("{}: WALK", label));
            continue;
        }

        // Build street line
        let label = street_label(street);
        let board_str = if !is_stud {
            format_board(&hand.board, street)
        } else {
            String::new()
        };

        if board_str.is_empty() {
            lines.push(format!("{}: {}", label, street_tokens.join(" ")));
        } else {
            lines.push(format!(
                "{}{}: {}",
                label,
                board_str,
                street_tokens.join(" ")
            ));
        }
    }

    // Result line
    let result_line = build_result_line(hand, hero, bb, &pot_tracker);
    if let Some(rl) = result_line {
        lines.push(rl);
    }

    lines.join("\n")
}

/// Detect walk: hero is BB, hero won, and no voluntary actions (call/bet/raise) preflop.
fn is_walk(hand: &Hand, _hero: &str, preflop_actions: &[&Action]) -> bool {
    if hand.result.hero_result != HeroResult::Won {
        return false;
    }

    // Check hero is BB
    let hero_is_bb = hand
        .hero_position
        .map(|p| p == Position::BB)
        .unwrap_or(false);
    if !hero_is_bb {
        return false;
    }

    // Check no voluntary actions exist (only blinds, antes, and folds)
    !preflop_actions.iter().any(|a| {
        matches!(
            a.action_type,
            ActionType::Call { .. } | ActionType::Bet { .. } | ActionType::Raise { .. }
        )
    })
}

/// Build the RESULT line for the encoding.
fn build_result_line(hand: &Hand, hero: &str, bb: f64, pot_tracker: &PotTracker) -> Option<String> {
    match hand.result.hero_result {
        HeroResult::SatOut => None,
        HeroResult::Won => {
            let collected: f64 = hand
                .result
                .winners
                .iter()
                .filter(|w| w.player == hero)
                .map(|w| w.amount.amount)
                .sum();
            let net = (collected - pot_tracker.hero_invested) / bb;
            Some(format!("RESULT: HERO({}bb)", fmt_result(net)))
        }
        HeroResult::Lost | HeroResult::Folded => {
            let net = -pot_tracker.hero_invested / bb;
            Some(format!("RESULT: HERO({}bb)", fmt_result(net)))
        }
    }
}

/// Format a net result in BB with sign prefix.
fn fmt_result(net: f64) -> String {
    let formatted = fmt_trimmed(net.abs(), 1);
    if net >= 0.0 {
        format!("+{}", formatted)
    } else {
        format!("-{}", formatted)
    }
}

#[cfg(test)]
mod tests;
