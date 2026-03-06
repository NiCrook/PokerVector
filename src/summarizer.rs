use crate::types::*;

/// Convert a Hand into a natural language summary optimized for semantic search.
/// Deterministic — no LLM, just template-based generation from struct fields.
pub fn summarize(hand: &Hand) -> String {
    let mut parts = Vec::new();

    // Line 1: Game info + hero info
    parts.push(game_info_line(hand));

    // Lines 2+: Per-street action summaries
    let streets = summarize_streets(hand);
    if !streets.is_empty() {
        parts.push(streets);
    }

    // Final line: Result
    parts.push(result_line(hand));

    parts.join("\n")
}

fn game_info_line(hand: &Hand) -> String {
    // Build variant descriptor
    let mut variant_parts = Vec::new();
    if hand.is_bomb_pot {
        variant_parts.push("Bomb pot".to_string());
    }
    if hand.variant != PokerVariant::Holdem {
        let mut v = hand.variant.to_string();
        if hand.is_hi_lo {
            v.push_str(" H/L");
        }
        variant_parts.push(v);
    } else if hand.is_hi_lo {
        variant_parts.push("Hold'em H/L".to_string());
    }
    if hand.betting_limit != BettingLimit::NoLimit {
        variant_parts.push(hand.betting_limit.to_string());
    }

    let variant_prefix = if variant_parts.is_empty() {
        String::new()
    } else {
        format!("{} ", variant_parts.join(" "))
    };

    let game_desc = match &hand.game_type {
        GameType::Cash {
            small_blind,
            big_blind,
            ..
        } => {
            if variant_prefix.is_empty() {
                format!("Cash game {}/{}", small_blind, big_blind)
            } else {
                format!("{}cash game {}/{}", variant_prefix, small_blind, big_blind)
            }
        }
        GameType::Tournament {
            tournament_id,
            level,
            ..
        } => {
            if variant_prefix.is_empty() {
                format!("Tournament #{} Level {}", tournament_id, level)
            } else {
                format!("{}Tournament #{} Level {}", variant_prefix, tournament_id, level)
            }
        }
    };

    let table_size = format!("{}-max", hand.table_size);

    let hero_name = hand.hero.as_deref().unwrap_or("Unknown");
    let hero_pos = hand
        .hero_position
        .map(|p| format!(" in {}", p))
        .unwrap_or_default();
    let hero_cards = if hand.hero_cards.is_empty() {
        String::new()
    } else {
        let cards: Vec<String> = hand.hero_cards.iter().map(|c| c.to_string()).collect();
        format!(" with {}", cards.join(" "))
    };

    format!(
        "{}, {}. Hero {} {}{}.",
        game_desc, table_size, hero_name, hero_pos, hero_cards
    )
}

fn summarize_streets(hand: &Hand) -> String {
    let hero_name = hand.hero.as_deref().unwrap_or("");
    let mut street_summaries = Vec::new();

    let streets: &[Street] = if hand.variant == PokerVariant::SevenCardStud {
        &[Street::ThirdStreet, Street::FourthStreet, Street::FifthStreet, Street::SixthStreet, Street::SeventhStreet]
    } else {
        &[Street::Preflop, Street::Flop, Street::Turn, Street::River]
    };

    for &street in streets {
        let street_actions: Vec<&Action> = hand
            .actions
            .iter()
            .filter(|a| a.street == street && is_notable_action(a, hero_name))
            .collect();

        if street_actions.is_empty() {
            continue;
        }

        let action_strs: Vec<String> = street_actions
            .iter()
            .map(|a| format_action(a, hero_name))
            .collect();

        let prefix = match street {
            Street::Preflop => "Preflop:".to_string(),
            Street::Flop => format!("Flop: {}.", board_for_street(hand, street)),
            Street::Turn => format!("Turn: {}.", board_for_street(hand, street)),
            Street::River => format!("River: {}.", board_for_street(hand, street)),
            _ => format!("{}:", street),
        };

        street_summaries.push(format!("{} {}", prefix, action_strs.join(". ") + "."));
    }

    // Showdown
    let showdown_actions: Vec<&Action> = hand
        .actions
        .iter()
        .filter(|a| a.street == Street::Showdown)
        .collect();
    if !showdown_actions.is_empty() {
        let show_strs: Vec<String> = showdown_actions
            .iter()
            .filter_map(|a| match &a.action_type {
                ActionType::Shows { description, .. } => {
                    let desc = description
                        .as_deref()
                        .map(|d| format!(" ({})", d))
                        .unwrap_or_default();
                    Some(format!("{} shows{}", display_name(&a.player, hero_name), desc))
                }
                _ => None,
            })
            .collect();
        if !show_strs.is_empty() {
            street_summaries.push(format!("Showdown: {}.", show_strs.join(". ")));
        }
    }

    street_summaries.join(" ")
}

fn board_for_street(hand: &Hand, street: Street) -> String {
    let count = match street {
        Street::Flop => 3.min(hand.board.len()),
        Street::Turn => {
            if hand.board.len() >= 4 {
                1 // just the turn card
            } else {
                0
            }
        }
        Street::River => {
            if hand.board.len() >= 5 {
                1 // just the river card
            } else {
                0
            }
        }
        _ => 0,
    };

    match street {
        Street::Flop => {
            let cards: Vec<String> = hand.board[..count].iter().map(|c| c.to_string()).collect();
            cards.join(" ")
        }
        Street::Turn if hand.board.len() >= 4 => hand.board[3].to_string(),
        Street::River if hand.board.len() >= 5 => hand.board[4].to_string(),
        _ => String::new(),
    }
}

fn is_notable_action(action: &Action, hero_name: &str) -> bool {
    let is_hero = action.player == hero_name;
    match &action.action_type {
        // Always include hero actions (except posting blinds)
        ActionType::Fold if is_hero => true,
        ActionType::Check if is_hero => true,
        ActionType::Call { .. } if is_hero => true,
        ActionType::Bet { .. } => true,         // all bets are notable
        ActionType::Raise { .. } => true,       // all raises are notable
        ActionType::Call { all_in: true, .. } => true, // all-in calls are notable
        ActionType::Fold if !is_hero => false,   // skip non-hero folds
        ActionType::Check if !is_hero => false,  // skip non-hero checks
        ActionType::Call { .. } if !is_hero => true, // non-hero calls are somewhat notable
        // Skip blind posts, sit-out, etc
        ActionType::PostSmallBlind { .. }
        | ActionType::PostBigBlind { .. }
        | ActionType::PostAnte { .. }
        | ActionType::PostBlind { .. }
        | ActionType::SitsOut
        | ActionType::WaitsForBigBlind
        | ActionType::UncalledBet { .. }
        | ActionType::Collected { .. }
        | ActionType::Shows { .. }
        | ActionType::DoesNotShow
        | ActionType::Mucks
        | ActionType::BringsIn { .. } => false,
        _ => false,
    }
}

fn display_name<'a>(player: &'a str, hero_name: &str) -> &'a str {
    if player == hero_name {
        "Hero"
    } else {
        player
    }
}

fn format_action(action: &Action, hero_name: &str) -> String {
    let name = display_name(&action.player, hero_name);
    match &action.action_type {
        ActionType::Fold => format!("{} folds", name),
        ActionType::Check => format!("{} checks", name),
        ActionType::Call { amount, all_in } => {
            if *all_in {
                format!("{} calls {} all-in", name, amount)
            } else {
                format!("{} calls {}", name, amount)
            }
        }
        ActionType::Bet { amount, all_in } => {
            if *all_in {
                format!("{} bets {} all-in", name, amount)
            } else {
                format!("{} bets {}", name, amount)
            }
        }
        ActionType::Raise { to, all_in, .. } => {
            if *all_in {
                format!("{} raises to {} all-in", name, to)
            } else {
                format!("{} raises to {}", name, to)
            }
        }
        _ => String::new(),
    }
}

fn result_line(hand: &Hand) -> String {
    let hero_name = hand.hero.as_deref().unwrap_or("Unknown");
    match &hand.result.hero_result {
        HeroResult::Won => {
            let total: f64 = hand
                .result
                .winners
                .iter()
                .filter(|w| hand.hero.as_ref().map(|h| w.player == *h).unwrap_or(false))
                .map(|w| w.amount.amount)
                .sum();
            let went_to_showdown = hand
                .actions
                .iter()
                .any(|a| a.street == Street::Showdown);
            if went_to_showdown {
                format!("Hero {} wins {} at showdown.", hero_name, format_money_from_hand(hand, total))
            } else {
                format!("Hero {} wins {} without showdown.", hero_name, format_money_from_hand(hand, total))
            }
        }
        HeroResult::Lost => {
            let went_to_showdown = hand
                .actions
                .iter()
                .any(|a| a.street == Street::Showdown);
            if went_to_showdown {
                format!("Hero {} loses at showdown.", hero_name)
            } else {
                format!("Hero {} loses.", hero_name)
            }
        }
        HeroResult::Folded => {
            // Find which street hero folded on
            let fold_street = hand
                .actions
                .iter()
                .find(|a| {
                    a.player == hero_name && matches!(a.action_type, ActionType::Fold)
                })
                .map(|a| a.street);
            match fold_street {
                Some(street) => format!("Hero {} folded on the {}.", hero_name, street),
                None => format!("Hero {} folded.", hero_name),
            }
        }
        HeroResult::SatOut => format!("Hero {} sat out.", hero_name),
    }
}

fn format_money_from_hand(hand: &Hand, amount: f64) -> String {
    let currency = match &hand.game_type {
        GameType::Cash { big_blind, .. } => big_blind.currency,
        GameType::Tournament { big_blind, .. } => big_blind.currency,
    };
    Money { amount, currency }.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_card(r: char, s: char) -> Card {
        Card {
            rank: Rank::from_char(r).unwrap(),
            suit: Suit::from_char(s).unwrap(),
        }
    }

    fn make_cash_hand() -> Hand {
        Hand {
            id: 12345,
            site: Site::ACR,
            variant: PokerVariant::Holdem,
            betting_limit: BettingLimit::NoLimit,
            is_hi_lo: false,
            is_bomb_pot: false,
            game_type: GameType::Cash {
                small_blind: Money { amount: 0.01, currency: Currency::USD },
                big_blind: Money { amount: 0.02, currency: Currency::USD },
                ante: None,
            },
            timestamp: "2024-01-01 12:00:00".to_string(),
            table_name: "Test".to_string(),
            table_size: 9,
            button_seat: 1,
            players: vec![],
            hero: Some("TestHero".to_string()),
            hero_position: Some(Position::BB),
            hero_cards: vec![make_card('Q', 'd'), make_card('J', 'c')],
            actions: vec![
                Action {
                    player: "TestHero".to_string(),
                    action_type: ActionType::Raise {
                        amount: Money { amount: 0.02, currency: Currency::USD },
                        to: Money { amount: 0.04, currency: Currency::USD },
                        all_in: false,
                    },
                    street: Street::Preflop,
                },
                Action {
                    player: "Freddeyz".to_string(),
                    action_type: ActionType::Call {
                        amount: Money { amount: 0.04, currency: Currency::USD },
                        all_in: false,
                    },
                    street: Street::Preflop,
                },
            ],
            board: vec![],
            pot: Some(Money { amount: 0.06, currency: Currency::USD }),
            rake: None,
            result: HandResult {
                winners: vec![Winner {
                    player: "TestHero".to_string(),
                    amount: Money { amount: 0.06, currency: Currency::USD },
                    pot: "Main pot".to_string(),
                }],
                hero_result: HeroResult::Won,
            },
            raw_text: String::new(),
            stud_cards: None,
        }
    }

    #[test]
    fn test_cash_hand_summary() {
        let hand = make_cash_hand();
        let summary = summarize(&hand);
        assert!(summary.contains("Cash game"));
        assert!(summary.contains("$0.01/$0.02"));
        assert!(summary.contains("9-max"));
        assert!(summary.contains("TestHero"));
        assert!(summary.contains("BB"));
        assert!(summary.contains("Qd Jc"));
        assert!(summary.contains("Hero raises to $0.04"));
        assert!(summary.contains("Freddeyz calls"));
        assert!(summary.contains("wins $0.06 without showdown"));
    }

    #[test]
    fn test_folded_hand_summary() {
        let mut hand = make_cash_hand();
        hand.actions = vec![Action {
            player: "TestHero".to_string(),
            action_type: ActionType::Fold,
            street: Street::Preflop,
        }];
        hand.result.hero_result = HeroResult::Folded;
        hand.result.winners = vec![];
        let summary = summarize(&hand);
        assert!(summary.contains("Hero folds"));
        assert!(summary.contains("folded on the Preflop"));
    }

    #[test]
    fn test_tournament_hand_summary() {
        let mut hand = make_cash_hand();
        hand.game_type = GameType::Tournament {
            tournament_id: 34375286,
            level: 17,
            small_blind: Money { amount: 400.0, currency: Currency::Chips },
            big_blind: Money { amount: 800.0, currency: Currency::Chips },
            ante: None,
        };
        let summary = summarize(&hand);
        assert!(summary.contains("Tournament #34375286"));
        assert!(summary.contains("Level 17"));
    }

    #[test]
    fn test_sat_out_summary() {
        let mut hand = make_cash_hand();
        hand.hero_cards = vec![];
        hand.actions = vec![];
        hand.result.hero_result = HeroResult::SatOut;
        hand.result.winners = vec![];
        let summary = summarize(&hand);
        assert!(summary.contains("sat out"));
    }
}
