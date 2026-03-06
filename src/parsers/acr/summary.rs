use std::sync::OnceLock;
use regex::Regex;

use crate::parsers::*;
use crate::types::*;

use super::actions::parse_money_or_chips;

fn re_summary_position() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.+?) \((button|small blind|big blind)\)").unwrap())
}

pub(super) fn parse_pot_rake_line(
    line: &str,
    currency: Currency,
    pot: &mut Option<Money>,
    rake: &mut Option<Money>,
) {
    // "Main pot $0.54 | Rake $0.02" or "Main pot 194350.00"
    let parts: Vec<&str> = line.split('|').collect();
    if let Some(main_part) = parts.first() {
        let main_part = main_part.trim();
        if main_part.starts_with("Main pot ") {
            let val_str = main_part.trim_start_matches("Main pot ").trim();
            if let Ok(m) = parse_money(val_str) {
                *pot = Some(m);
            } else if let Ok(amount) = val_str.parse::<f64>() {
                *pot = Some(Money { amount, currency });
            }
        }
    }
    if let Some(rake_part) = parts.get(1) {
        let rake_part = rake_part.trim();
        if rake_part.starts_with("Rake ") {
            let val_str = rake_part.trim_start_matches("Rake ").trim();
            if let Ok(m) = parse_money(val_str) {
                *rake = Some(m);
            } else if let Ok(amount) = val_str.parse::<f64>() {
                *rake = Some(Money { amount, currency });
            }
        }
    }
}

pub(super) fn parse_total_pot_line(
    line: &str,
    currency: Currency,
    pot: &mut Option<Money>,
    rake: &mut Option<Money>,
) {
    // "Total pot $1.14 | Rake $0.04 | JP Fee $0.02" or "Total pot $0.06"
    let parts: Vec<&str> = line.split('|').collect();
    if let Some(main_part) = parts.first() {
        let main_part = main_part.trim();
        if main_part.starts_with("Total pot ") {
            let val_str = main_part.trim_start_matches("Total pot ").trim();
            if let Ok(m) = parse_money(val_str) {
                *pot = Some(m);
            } else if let Ok(amount) = val_str.parse::<f64>() {
                *pot = Some(Money { amount, currency });
            }
        }
    }
    if let Some(rake_part) = parts.get(1) {
        let rake_part = rake_part.trim();
        if rake_part.starts_with("Rake ") {
            let val_str = rake_part.trim_start_matches("Rake ").trim();
            if let Ok(m) = parse_money(val_str) {
                *rake = Some(m);
            } else if let Ok(amount) = val_str.parse::<f64>() {
                *rake = Some(Money { amount, currency });
            }
        }
    }
}

pub(super) fn parse_summary_seat_line(
    line: &str,
    currency: Currency,
    winners: &mut Vec<Winner>,
) {
    // Patterns:
    // "Seat N: NAME showed [XX XX] and won $X with ..."
    // "Seat N: NAME did not show and won $X"
    // "Seat N: NAME (position) showed [XX XX] and won $X with ..."
    // "Seat N: NAME (position) did not show and won $X"
    // Also tournament: "and won 12345.00"

    // Find "and won " in the line
    if let Some(won_pos) = line.find(" and won ") {
        let after_won = &line[won_pos + 9..]; // after " and won "
        // Extract the amount — it's the next token, possibly followed by " with ..."
        let amt_str = after_won
            .split_whitespace()
            .next()
            .unwrap_or("");
        let amount = parse_money_or_chips(amt_str, currency);

        // Extract player name from "Seat N: NAME ..." or "Seat N: NAME (pos) ..."
        // Find the colon after seat number
        if let Some(colon_pos) = line.find(": ") {
            let after_colon = &line[colon_pos + 2..];
            // Player name ends at " showed", " did not show", " (", or " folded"
            let name = extract_player_name_from_summary(after_colon);

            // Check if this player is already in the winners list (from "collected" lines)
            if !winners.iter().any(|w| w.player == name) {
                winners.push(Winner {
                    player: name,
                    amount,
                    pot: "main pot".to_string(),
                });
            }
        }
    }
}

fn extract_player_name_from_summary(s: &str) -> String {
    // Name ends before one of these patterns:
    // " showed [", " did not show", " (button)", " (small blind)", " (big blind)", " folded"
    let terminators = [
        " showed [",
        " did not show",
        " folded",
        " (button)",
        " (small blind)",
        " (big blind)",
    ];

    let mut end = s.len();
    for t in &terminators {
        if let Some(pos) = s.find(t) {
            if pos < end {
                end = pos;
            }
        }
    }

    // Also check for position in parens: " (something) "
    // Pattern: name might be followed by " (" for position
    if let Some(caps) = re_summary_position().captures(s) {
        return caps[1].to_string();
    }

    s[..end].to_string()
}

pub(super) fn determine_hero_result(
    actions: &[Action],
    hero: &str,
    winners: &[Winner],
) -> HeroResult {
    // Check if hero is among winners
    if winners.iter().any(|w| w.player == hero) {
        return HeroResult::Won;
    }

    // Check if hero folded
    if actions.iter().any(|a| a.player == hero && a.action_type == ActionType::Fold) {
        return HeroResult::Folded;
    }

    // Check if hero sat out
    let hero_acted = actions.iter().any(|a| {
        a.player == hero
            && !matches!(
                a.action_type,
                ActionType::SitsOut | ActionType::WaitsForBigBlind
            )
    });
    if !hero_acted {
        return HeroResult::SatOut;
    }

    HeroResult::Lost
}
