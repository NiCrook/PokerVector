mod header;
mod actions;
mod summary;

#[cfg(test)]
mod tests;

use crate::parsers::*;
use crate::types::*;

use header::{parse_header, parse_table_line, parse_seat_line};
use actions::{try_parse_action, parse_uncalled_bet, parse_stud_dealt_line};
use summary::{parse_pot_rake_line, parse_total_pot_line, parse_summary_seat_line, determine_hero_result};

pub struct AcrParser;

impl SiteParser for AcrParser {
    fn parse_file(&self, content: &str, hero: &str) -> Vec<ParseResult<Hand>> {
        let hand_texts = split_hands(content);
        hand_texts
            .into_iter()
            .map(|text| parse_hand(text, hero))
            .collect()
    }

    fn detect(content: &str) -> bool {
        let first_line = content.lines().next().unwrap_or("");
        first_line.starts_with("Hand #") || first_line.starts_with("Game Hand #")
    }
}

impl AcrParser {
    pub fn detect(content: &str) -> bool {
        <Self as SiteParser>::detect(content)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ParseState {
    Preflop,
    Flop,
    Turn,
    River,
    ThirdStreet,
    FourthStreet,
    FifthStreet,
    SixthStreet,
    SeventhStreet,
    Showdown,
    Summary,
}

fn parse_hand(text: &str, hero: &str) -> ParseResult<Hand> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.is_empty() {
        return Err(ParseError::Incomplete("empty hand text".into()));
    }

    // --- Pass 1: Parse header, table, seats to collect known player names ---
    let header = parse_header(lines[0])?;
    let is_stud = header.variant == PokerVariant::SevenCardStud;

    if lines.len() < 2 {
        return Err(ParseError::Incomplete("no table line".into()));
    }
    let (table_name, table_size, button_seat) = parse_table_line(lines[1])?;

    // Collect seat info
    let mut players: Vec<Player> = Vec::new();
    let mut line_idx = 2;

    while line_idx < lines.len() {
        let line = lines[line_idx].trim();
        if line.starts_with("Seat ") && line.contains(": ") {
            if let Some(p) = parse_seat_line(line, &header.game_type) {
                players.push(p);
            }
        } else {
            break;
        }
        line_idx += 1;
    }

    // Build sorted list of known names (longest first for prefix matching)
    let mut known_names: Vec<String> = players.iter().map(|p| p.name.clone()).collect();
    known_names.sort_by(|a, b| b.len().cmp(&a.len()));

    // Calculate positions (skip for stud — no button)
    if !is_stud {
        let active_seats: Vec<u8> = players
            .iter()
            .filter(|p| !p.is_sitting_out)
            .map(|p| p.seat)
            .collect();

        for player in &mut players {
            if !player.is_sitting_out {
                player.position = calculate_position(player.seat, button_seat, &active_seats);
            }
        }
    }

    for player in &mut players {
        if player.name == hero {
            player.is_hero = true;
        }
    }

    let hero_position = players
        .iter()
        .find(|p| p.is_hero)
        .and_then(|p| p.position);

    // --- Pass 2: Parse actions ---
    let mut actions: Vec<Action> = Vec::new();
    let mut hero_cards: Vec<Card> = Vec::new();
    let mut board: Vec<Card> = Vec::new();
    let mut pot: Option<Money> = None;
    let mut rake: Option<Money> = None;
    let mut winners: Vec<Winner> = Vec::new();
    let mut is_bomb_pot = false;
    let mut stud_cards: Vec<StudPlayerCards> = Vec::new();
    let mut state = if is_stud {
        ParseState::ThirdStreet
    } else {
        ParseState::Preflop
    };

    let currency = match &header.game_type {
        GameType::Cash { .. } => Currency::USD,
        GameType::Tournament { .. } => Currency::Chips,
    };

    while line_idx < lines.len() {
        let line = lines[line_idx].trim();
        line_idx += 1;

        if line.is_empty() {
            continue;
        }

        // Street markers
        if line == "*** HOLE CARDS ***" {
            state = ParseState::Preflop;
            continue;
        }
        if line.starts_with("*** FLOP *** [") {
            state = ParseState::Flop;
            if let Some(cards_str) = extract_bracket_cards(line) {
                board = parse_cards(cards_str);
            }
            continue;
        }
        if line.starts_with("*** TURN *** ") {
            state = ParseState::Turn;
            if let Some(pos) = line.rfind('[') {
                let new_card_str = &line[pos..];
                let new_cards = parse_cards(new_card_str);
                board.extend(new_cards);
            }
            continue;
        }
        if line.starts_with("*** RIVER *** ") {
            state = ParseState::River;
            if let Some(pos) = line.rfind('[') {
                let new_card_str = &line[pos..];
                let new_cards = parse_cards(new_card_str);
                board.extend(new_cards);
            }
            continue;
        }
        // Stud street markers
        if line == "*** 3rd STREET ***" {
            state = ParseState::ThirdStreet;
            continue;
        }
        if line == "*** 4th STREET ***" {
            state = ParseState::FourthStreet;
            continue;
        }
        if line == "*** 5th STREET ***" {
            state = ParseState::FifthStreet;
            continue;
        }
        if line == "*** 6th STREET ***" {
            state = ParseState::SixthStreet;
            continue;
        }
        if line == "*** 7th STREET ***" {
            state = ParseState::SeventhStreet;
            continue;
        }
        if line == "*** SHOW DOWN ***" {
            state = ParseState::Showdown;
            continue;
        }
        if line == "*** SUMMARY ***" {
            state = ParseState::Summary;
            continue;
        }

        // Pot/rake lines (appear between streets and in showdown)
        if line.starts_with("Main pot ") || line.starts_with("Side pot(") {
            if state == ParseState::Summary {
                // In summary we parse Total pot instead
            } else {
                if line.starts_with("Main pot ") {
                    parse_pot_rake_line(line, currency, &mut pot, &mut rake);
                }
            }
            continue;
        }

        // Summary section
        if state == ParseState::Summary {
            if line == "BombPot" {
                is_bomb_pot = true;
                continue;
            }
            if line.starts_with("Total pot ") {
                parse_total_pot_line(line, currency, &mut pot, &mut rake);
            } else if line.starts_with("Board ") {
                // We already have board from street markers
            } else if line.starts_with("Seat ") {
                parse_summary_seat_line(line, currency, &mut winners);
            }
            continue;
        }

        // Dealt to lines
        if line.starts_with("Dealt to ") {
            if is_stud {
                // Stud: parse dealt cards for all players
                parse_stud_dealt_line(line, hero, &known_names, &mut hero_cards, &mut stud_cards);
            } else {
                if let Some(bracket_pos) = line.find('[') {
                    let name_part = &line[9..bracket_pos].trim_end();
                    if *name_part == hero {
                        hero_cards = parse_cards(&line[bracket_pos..]);
                    }
                }
            }
            continue;
        }

        // Uncalled bet
        if line.starts_with("Uncalled bet (") {
            if let Some(action) = parse_uncalled_bet(line, &known_names, current_street(state), currency) {
                actions.push(action);
            }
            continue;
        }

        // Try matching player action lines
        if let Some(action) = try_parse_action(line, &known_names, current_street(state), currency) {
            // Track collected amounts for winners
            if let ActionType::Collected { ref amount, ref pot } = action.action_type {
                winners.push(Winner {
                    player: action.player.clone(),
                    amount: *amount,
                    pot: pot.clone(),
                });
            }
            actions.push(action);
        }
    }

    // Determine hero result
    let hero_result = determine_hero_result(&actions, hero, &winners);

    Ok(Hand {
        id: header.hand_id,
        site: Site::ACR,
        variant: header.variant,
        betting_limit: header.betting_limit,
        is_hi_lo: header.is_hi_lo,
        is_bomb_pot,
        game_type: header.game_type,
        timestamp: header.timestamp,
        table_name,
        table_size,
        button_seat,
        players,
        hero: Some(hero.to_string()),
        hero_position,
        hero_cards,
        actions,
        board,
        pot,
        rake,
        result: HandResult {
            winners,
            hero_result,
        },
        raw_text: text.to_string(),
        stud_cards: if is_stud { Some(stud_cards) } else { None },
    })
}

fn current_street(state: ParseState) -> Street {
    match state {
        ParseState::Preflop => Street::Preflop,
        ParseState::Flop => Street::Flop,
        ParseState::Turn => Street::Turn,
        ParseState::River => Street::River,
        ParseState::ThirdStreet => Street::ThirdStreet,
        ParseState::FourthStreet => Street::FourthStreet,
        ParseState::FifthStreet => Street::FifthStreet,
        ParseState::SixthStreet => Street::SixthStreet,
        ParseState::SeventhStreet => Street::SeventhStreet,
        ParseState::Showdown | ParseState::Summary => Street::Showdown,
    }
}

fn extract_bracket_cards(line: &str) -> Option<&str> {
    let start = line.find('[')?;
    let end = line.find(']')?;
    Some(&line[start..=end])
}
