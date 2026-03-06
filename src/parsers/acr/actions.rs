use std::sync::OnceLock;
use regex::Regex;

use crate::parsers::*;
use crate::types::*;

fn re_uncalled_bet() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^Uncalled bet \((\$?[0-9.]+)\) returned to (.+)$").unwrap())
}

fn re_bracket_cards() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]+)\]").unwrap())
}

pub(super) fn parse_money_or_chips(s: &str, default_currency: Currency) -> Money {
    if let Ok(m) = parse_money(s) {
        m
    } else {
        Money {
            amount: s.parse().unwrap_or(0.0),
            currency: default_currency,
        }
    }
}

pub(super) fn parse_uncalled_bet(
    line: &str,
    _known_names: &[String],
    street: Street,
    currency: Currency,
) -> Option<Action> {
    // "Uncalled bet ($0.18) returned to NAME" or "Uncalled bet (29700.00) returned to NAME"
    if let Some(caps) = re_uncalled_bet().captures(line) {
        let amount = parse_money_or_chips(&caps[1], currency);
        let player = caps[2].trim().to_string();
        return Some(Action {
            player,
            action_type: ActionType::UncalledBet { amount },
            street,
        });
    }
    None
}

pub(super) fn parse_stud_dealt_line(
    line: &str,
    hero: &str,
    _known_names: &[String],
    hero_cards: &mut Vec<Card>,
    stud_cards: &mut Vec<StudPlayerCards>,
) {
    // Stud dealt lines:
    // Hero 3rd: "Dealt to TestHero [5s 3s 9d]" (2 hidden + 1 up)
    // Others 3rd: "Dealt to PLAYER [7h]" (1 visible card)
    // 4th+: "Dealt to TestHero [5s 3s 9d] [Jd]" (cumulative + new)
    // Others 4th+: "Dealt to PLAYER [7h] [5c]" (cumulative visible + new)
    if let Some(bracket_pos) = line.find('[') {
        let name_part = line[9..bracket_pos].trim_end();

        // Find if there's a second bracket set (4th street+)
        let cards_section = &line[bracket_pos..];

        if name_part == hero {
            // For hero, take ALL cards from all bracket groups
            let mut all_cards = Vec::new();
            for cap in re_bracket_cards().find_iter(cards_section) {
                all_cards.extend(parse_cards(cap.as_str()));
            }
            // On each street, hero gets cumulative cards. We want the latest full set.
            *hero_cards = all_cards;
        }

        // For stud_cards, collect visible cards per player
        // Find/create entry for this player
        let entry = stud_cards.iter_mut().find(|sc| sc.player == name_part);
        if name_part == hero {
            // Hero's visible cards: all of them
            if let Some(entry) = entry {
                entry.cards = hero_cards.clone();
            } else {
                stud_cards.push(StudPlayerCards {
                    player: name_part.to_string(),
                    cards: hero_cards.clone(),
                });
            }
        } else {
            // For opponents, collect all visible cards across streets
            let mut visible_cards = Vec::new();
            for cap in re_bracket_cards().find_iter(cards_section) {
                visible_cards.extend(parse_cards(cap.as_str()));
            }
            if let Some(entry) = entry {
                // Replace with latest cumulative set
                entry.cards = visible_cards;
            } else {
                stud_cards.push(StudPlayerCards {
                    player: name_part.to_string(),
                    cards: visible_cards,
                });
            }
        }
    }
}

pub(super) fn try_parse_action(
    line: &str,
    known_names: &[String],
    street: Street,
    currency: Currency,
) -> Option<Action> {
    // Try each known name (longest first) as prefix
    for name in known_names {
        if !line.starts_with(name.as_str()) {
            continue;
        }

        let remainder = &line[name.len()..];

        // Remainder must start with a space or be exactly the rest
        if !remainder.starts_with(' ') {
            continue;
        }
        let remainder = remainder.trim_start();

        // Posts
        if let Some(r) = remainder.strip_prefix("posts the small blind ") {
            let all_in = r.ends_with(" and is all-in");
            let amt_str = if all_in {
                r.trim_end_matches(" and is all-in")
            } else {
                r
            };
            let amount = parse_money_or_chips(amt_str, currency);
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::PostSmallBlind { amount, all_in },
                street,
            });
        }
        if let Some(r) = remainder.strip_prefix("posts the big blind ") {
            let all_in = r.ends_with(" and is all-in");
            let amt_str = if all_in {
                r.trim_end_matches(" and is all-in")
            } else {
                r
            };
            let amount = parse_money_or_chips(amt_str, currency);
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::PostBigBlind { amount, all_in },
                street,
            });
        }
        if let Some(r) = remainder.strip_prefix("posts ante ") {
            let amount = parse_money_or_chips(r, currency);
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::PostAnte { amount },
                street,
            });
        }
        // "posts $0.02" (new player blind post)
        if let Some(r) = remainder.strip_prefix("posts ") {
            if !r.starts_with("the ") && !r.starts_with("ante ") {
                let amount = parse_money_or_chips(r, currency);
                return Some(Action {
                    player: name.clone(),
                    action_type: ActionType::PostBlind { amount },
                    street,
                });
            }
        }

        // Brings in (stud)
        if let Some(r) = remainder.strip_prefix("brings in ") {
            let amount = parse_money_or_chips(r, currency);
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::BringsIn { amount },
                street,
            });
        }

        // Basic actions
        if remainder == "folds" {
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::Fold,
                street,
            });
        }
        if remainder == "checks" {
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::Check,
                street,
            });
        }

        // Calls
        if let Some(r) = remainder.strip_prefix("calls ") {
            let all_in = r.ends_with(" and is all-in");
            let amt_str = if all_in {
                r.trim_end_matches(" and is all-in")
            } else {
                r
            };
            let amount = parse_money_or_chips(amt_str, currency);
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::Call { amount, all_in },
                street,
            });
        }

        // Bets
        if let Some(r) = remainder.strip_prefix("bets ") {
            let all_in = r.ends_with(" and is all-in");
            let amt_str = if all_in {
                r.trim_end_matches(" and is all-in")
            } else {
                r
            };
            let amount = parse_money_or_chips(amt_str, currency);
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::Bet { amount, all_in },
                street,
            });
        }

        // Raises: "raises $X to $Y[ and is all-in]"
        if let Some(r) = remainder.strip_prefix("raises ") {
            let all_in = r.ends_with(" and is all-in");
            let r = if all_in {
                r.trim_end_matches(" and is all-in")
            } else {
                r
            };
            if let Some(to_pos) = r.find(" to ") {
                let raise_str = &r[..to_pos];
                let to_str = &r[to_pos + 4..];
                let amount = parse_money_or_chips(raise_str, currency);
                let to = parse_money_or_chips(to_str, currency);
                return Some(Action {
                    player: name.clone(),
                    action_type: ActionType::Raise { amount, to, all_in },
                    street,
                });
            }
        }

        // Shows: "shows [XX XX][ (description)]"
        if let Some(r) = remainder.strip_prefix("shows [") {
            let bracket_end = r.find(']').unwrap_or(r.len());
            let cards_str = &r[..bracket_end];
            let cards: Vec<Option<Card>> = cards_str
                .split_whitespace()
                .map(|c| parse_card(c).ok())
                .collect();
            let description = if bracket_end + 1 < r.len() {
                let rest = r[bracket_end + 1..].trim();
                if rest.starts_with('(') && rest.ends_with(')') {
                    Some(rest[1..rest.len() - 1].to_string())
                } else if !rest.is_empty() {
                    Some(rest.to_string())
                } else {
                    None
                }
            } else {
                None
            };
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::Shows { cards, description },
                street,
            });
        }

        if remainder == "does not show" {
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::DoesNotShow,
                street,
            });
        }

        if remainder == "mucks" {
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::Mucks,
                street,
            });
        }

        if remainder == "sits out" {
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::SitsOut,
                street,
            });
        }

        if remainder == "waits for big blind" {
            return Some(Action {
                player: name.clone(),
                action_type: ActionType::WaitsForBigBlind,
                street,
            });
        }

        // Collected: "collected $X from main pot" / "from side pot-N"
        if let Some(r) = remainder.strip_prefix("collected ") {
            if let Some(from_pos) = r.find(" from ") {
                let amt_str = &r[..from_pos];
                let pot_name = r[from_pos + 6..].to_string();
                let amount = parse_money_or_chips(amt_str, currency);
                return Some(Action {
                    player: name.clone(),
                    action_type: ActionType::Collected {
                        amount,
                        pot: pot_name,
                    },
                    street,
                });
            }
        }

        // If we matched the name but couldn't parse the action, skip
        break;
    }

    None
}
