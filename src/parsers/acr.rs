use std::sync::OnceLock;
use regex::Regex;

use crate::parsers::*;
use crate::types::*;

fn re_header_tournament() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"^Game Hand #(\d+) - Tournament #(\d+) - (.+?) \((.+?)\) - Level (\d+) \(([0-9.]+)/([0-9.]+)\) - (.+) UTC$"
    ).unwrap())
}

fn re_header_cash() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"^Hand #(\d+) - (.+?) \((.+?)\) - \$([0-9.]+)/\$([0-9.]+)(?:, Ante \$([0-9.]+))? - (.+) UTC$"
    ).unwrap())
}

fn re_table_tournament() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"^Table '(.+?)' (\d+)-max Seat #(\d+) is the button$"
    ).unwrap())
}

fn re_table_cash() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"^(.+?) (\d+)-max Seat #(\d+) is the button$"
    ).unwrap())
}

fn re_table_stud() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.+?) (\d+)-max$").unwrap())
}

fn re_seat() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(
        r"^Seat (\d+): (.+?) \((\$?[0-9.]+)\)(?:\s+is sitting out)?$"
    ).unwrap())
}

fn re_summary_position() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(.+?) \((button|small blind|big blind)\)").unwrap())
}

fn re_uncalled_bet() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^Uncalled bet \((\$?[0-9.]+)\) returned to (.+)$").unwrap())
}

fn re_bracket_cards() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[([^\]]+)\]").unwrap())
}

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

struct HeaderInfo {
    hand_id: u64,
    game_type: GameType,
    timestamp: String,
    variant: PokerVariant,
    betting_limit: BettingLimit,
    is_hi_lo: bool,
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

fn parse_variant_limit(game_str: &str, limit_str: &str) -> (PokerVariant, BettingLimit, bool) {
    let (variant, is_hi_lo) = match game_str {
        "Holdem" => (PokerVariant::Holdem, false),
        "Omaha H/L" => (PokerVariant::Omaha, true),
        "Omaha" => (PokerVariant::Omaha, false),
        "5Card Omaha" => (PokerVariant::FiveCardOmaha, false),
        "5Card Omaha H/L" => (PokerVariant::FiveCardOmaha, true),
        "7Stud H/L" => (PokerVariant::SevenCardStud, true),
        "7Stud" => (PokerVariant::SevenCardStud, false),
        _ => (PokerVariant::Holdem, false),
    };
    let betting_limit = match limit_str {
        "No Limit" => BettingLimit::NoLimit,
        "Pot Limit" => BettingLimit::PotLimit,
        "Fixed Limit" => BettingLimit::FixedLimit,
        _ => BettingLimit::NoLimit,
    };
    (variant, betting_limit, is_hi_lo)
}

fn parse_header(line: &str) -> ParseResult<HeaderInfo> {
    // Tournament: "Game Hand #ID - Tournament #TID - GAME (LIMIT) - Level L (SB/BB) - TIMESTAMP UTC"
    if line.starts_with("Game Hand #") {
        if let Some(caps) = re_header_tournament().captures(line) {
            let hand_id: u64 = caps[1].parse().map_err(|_| ParseError::Header(line.into()))?;
            let tournament_id: u64 = caps[2].parse().map_err(|_| ParseError::Header(line.into()))?;
            let (variant, betting_limit, is_hi_lo) = parse_variant_limit(&caps[3], &caps[4]);
            let level: u32 = caps[5].parse().map_err(|_| ParseError::Header(line.into()))?;
            let sb: f64 = caps[6].parse().map_err(|_| ParseError::Header(line.into()))?;
            let bb: f64 = caps[7].parse().map_err(|_| ParseError::Header(line.into()))?;
            let timestamp = caps[8].to_string();

            return Ok(HeaderInfo {
                hand_id,
                game_type: GameType::Tournament {
                    tournament_id,
                    level,
                    small_blind: Money { amount: sb, currency: Currency::Chips },
                    big_blind: Money { amount: bb, currency: Currency::Chips },
                    ante: None,
                },
                timestamp,
                variant,
                betting_limit,
                is_hi_lo,
            });
        }
        return Err(ParseError::Header(line.into()));
    }

    // Cash: "Hand #ID - GAME (LIMIT) - $SB/$BB[, Ante $ANTE] - TIMESTAMP UTC"
    if line.starts_with("Hand #") {
        if let Some(caps) = re_header_cash().captures(line) {
            let hand_id: u64 = caps[1].parse().map_err(|_| ParseError::Header(line.into()))?;
            let (variant, betting_limit, is_hi_lo) = parse_variant_limit(&caps[2], &caps[3]);
            let sb: f64 = caps[4].parse().map_err(|_| ParseError::Header(line.into()))?;
            let bb: f64 = caps[5].parse().map_err(|_| ParseError::Header(line.into()))?;
            let ante = caps.get(6).map(|m| {
                let a: f64 = m.as_str().parse().unwrap_or(0.0);
                Money { amount: a, currency: Currency::USD }
            });
            let timestamp = caps[7].to_string();

            return Ok(HeaderInfo {
                hand_id,
                game_type: GameType::Cash {
                    small_blind: Money { amount: sb, currency: Currency::USD },
                    big_blind: Money { amount: bb, currency: Currency::USD },
                    ante,
                },
                timestamp,
                variant,
                betting_limit,
                is_hi_lo,
            });
        }
        return Err(ParseError::Header(line.into()));
    }

    Err(ParseError::Header(line.into()))
}

fn parse_table_line(line: &str) -> ParseResult<(String, u8, u8)> {
    // Tournament: "Table 'N' M-max Seat #B is the button"
    if let Some(caps) = re_table_tournament().captures(line) {
        let table_name = caps[1].to_string();
        let table_size: u8 = caps[2].parse().map_err(|_| ParseError::Table(line.into()))?;
        let button_seat: u8 = caps[3].parse().map_err(|_| ParseError::Table(line.into()))?;
        return Ok((table_name, table_size, button_seat));
    }

    // Cash: "TableName M-max Seat #B is the button"
    if let Some(caps) = re_table_cash().captures(line) {
        let table_name = caps[1].to_string();
        let table_size: u8 = caps[2].parse().map_err(|_| ParseError::Table(line.into()))?;
        let button_seat: u8 = caps[3].parse().map_err(|_| ParseError::Table(line.into()))?;
        return Ok((table_name, table_size, button_seat));
    }

    // Stud: "TableName M-max" (no button)
    if let Some(caps) = re_table_stud().captures(line) {
        let table_name = caps[1].to_string();
        let table_size: u8 = caps[2].parse().map_err(|_| ParseError::Table(line.into()))?;
        return Ok((table_name, table_size, 0));
    }

    Err(ParseError::Table(line.into()))
}

fn parse_seat_line(line: &str, game_type: &GameType) -> Option<Player> {
    // "Seat N: NAME will be allowed to play after the button"
    if line.contains("will be allowed to play after the button") {
        return None;
    }

    // "Seat N: NAME (STACK)[ is sitting out]"
    if let Some(caps) = re_seat().captures(line) {
        let seat: u8 = caps[1].parse().ok()?;
        let name = caps[2].to_string();
        let stack_str = &caps[3];
        let sitting_out = line.ends_with("is sitting out");

        let stack = if stack_str.starts_with('$') {
            Money {
                amount: stack_str[1..].parse().ok()?,
                currency: Currency::USD,
            }
        } else {
            let currency = match game_type {
                GameType::Cash { .. } => Currency::USD,
                GameType::Tournament { .. } => Currency::Chips,
            };
            Money {
                amount: stack_str.parse().ok()?,
                currency,
            }
        };

        return Some(Player {
            seat,
            name,
            stack,
            position: None,
            is_hero: false,
            is_sitting_out: sitting_out,
        });
    }

    None
}

fn extract_bracket_cards(line: &str) -> Option<&str> {
    let start = line.find('[')?;
    let end = line.find(']')?;
    Some(&line[start..=end])
}

fn parse_pot_rake_line(
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

fn parse_total_pot_line(
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

fn parse_summary_seat_line(
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

fn parse_uncalled_bet(
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

fn parse_money_or_chips(s: &str, default_currency: Currency) -> Money {
    if let Ok(m) = parse_money(s) {
        m
    } else {
        Money {
            amount: s.parse().unwrap_or(0.0),
            currency: default_currency,
        }
    }
}

fn parse_stud_dealt_line(
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

fn try_parse_action(
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

fn determine_hero_result(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cash_header() {
        let line = "Hand #2651598865 - Holdem (No Limit) - $0.01/$0.02 - 2026/01/22 11:09:21 UTC";
        let h = parse_header(line).unwrap();
        assert_eq!(h.hand_id, 2651598865);
        assert_eq!(h.timestamp, "2026/01/22 11:09:21");
        assert_eq!(h.variant, PokerVariant::Holdem);
        assert_eq!(h.betting_limit, BettingLimit::NoLimit);
        assert!(!h.is_hi_lo);
        match h.game_type {
            GameType::Cash { small_blind, big_blind, ante } => {
                assert_eq!(small_blind.amount, 0.01);
                assert_eq!(big_blind.amount, 0.02);
                assert!(ante.is_none());
            }
            _ => panic!("expected cash game"),
        }
    }

    #[test]
    fn test_parse_cash_header_with_ante() {
        let line = "Hand #2662089748 - Holdem (No Limit) - $0.01/$0.02, Ante $0.01 - 2026/02/03 03:56:30 UTC";
        let h = parse_header(line).unwrap();
        assert_eq!(h.hand_id, 2662089748);
        match h.game_type {
            GameType::Cash { ante, .. } => {
                assert!(ante.is_some());
                assert_eq!(ante.unwrap().amount, 0.01);
            }
            _ => panic!("expected cash game"),
        }
    }

    #[test]
    fn test_parse_tournament_header() {
        let line = "Game Hand #2653060401 - Tournament #34375286 - Holdem (No Limit) - Level 17 (2500.00/5000.00) - 2026/01/24 03:15:18 UTC";
        let h = parse_header(line).unwrap();
        assert_eq!(h.hand_id, 2653060401);
        match h.game_type {
            GameType::Tournament { tournament_id, level, small_blind, big_blind, .. } => {
                assert_eq!(tournament_id, 34375286);
                assert_eq!(level, 17);
                assert_eq!(small_blind.amount, 2500.0);
                assert_eq!(big_blind.amount, 5000.0);
            }
            _ => panic!("expected tournament"),
        }
    }

    #[test]
    fn test_parse_omaha_hl_header() {
        let line = "Hand #2672065483 - Omaha H/L (Fixed Limit) - $0.02/$0.04 - 2026/02/16 03:36:06 UTC";
        let h = parse_header(line).unwrap();
        assert_eq!(h.variant, PokerVariant::Omaha);
        assert_eq!(h.betting_limit, BettingLimit::FixedLimit);
        assert!(h.is_hi_lo);
    }

    #[test]
    fn test_parse_5card_omaha_header() {
        let line = "Hand #2672065251 - 5Card Omaha (Pot Limit) - $0.01/$0.02 - 2026/02/16 03:35:48 UTC";
        let h = parse_header(line).unwrap();
        assert_eq!(h.variant, PokerVariant::FiveCardOmaha);
        assert_eq!(h.betting_limit, BettingLimit::PotLimit);
        assert!(!h.is_hi_lo);
    }

    #[test]
    fn test_parse_7stud_hl_header() {
        let line = "Hand #2672065780 - 7Stud H/L (Fixed Limit) - $0.04/$0.08, Ante $0.01 - 2026/02/16 03:36:34 UTC";
        let h = parse_header(line).unwrap();
        assert_eq!(h.variant, PokerVariant::SevenCardStud);
        assert_eq!(h.betting_limit, BettingLimit::FixedLimit);
        assert!(h.is_hi_lo);
        match h.game_type {
            GameType::Cash { ante, .. } => {
                assert!(ante.is_some());
                assert_eq!(ante.unwrap().amount, 0.01);
            }
            _ => panic!("expected cash game"),
        }
    }

    #[test]
    fn test_parse_table_cash() {
        let line = "McCook 9-max Seat #9 is the button";
        let (name, size, btn) = parse_table_line(line).unwrap();
        assert_eq!(name, "McCook");
        assert_eq!(size, 9);
        assert_eq!(btn, 9);
    }

    #[test]
    fn test_parse_table_cash_dot() {
        let line = "St. Petersburg 6-max Seat #1 is the button";
        let (name, size, btn) = parse_table_line(line).unwrap();
        assert_eq!(name, "St. Petersburg");
        assert_eq!(size, 6);
        assert_eq!(btn, 1);
    }

    #[test]
    fn test_parse_table_tournament() {
        let line = "Table '36' 8-max Seat #6 is the button";
        let (name, size, btn) = parse_table_line(line).unwrap();
        assert_eq!(name, "36");
        assert_eq!(size, 8);
        assert_eq!(btn, 6);
    }

    #[test]
    fn test_parse_table_stud() {
        let line = "Kappa 8-max";
        let (name, size, btn) = parse_table_line(line).unwrap();
        assert_eq!(name, "Kappa");
        assert_eq!(size, 8);
        assert_eq!(btn, 0);
    }

    #[test]
    fn test_detect() {
        assert!(AcrParser::detect("Hand #123 - Holdem"));
        assert!(AcrParser::detect("Game Hand #123 - Tournament"));
        assert!(!AcrParser::detect("PokerStars Hand #123"));
    }
}
