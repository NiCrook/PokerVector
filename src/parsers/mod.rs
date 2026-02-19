pub mod acr;

use crate::types::*;
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum ParseError {
    #[error("Failed to parse header: {0}")]
    Header(String),
    #[error("Failed to parse table line: {0}")]
    Table(String),
    #[error("Failed to parse seat: {0}")]
    Seat(String),
    #[error("Failed to parse action: {0}")]
    Action(String),
    #[error("Failed to parse card: {0}")]
    Card(String),
    #[error("Failed to parse money: {0}")]
    Money(String),
    #[error("Unknown site format")]
    UnknownSite,
    #[error("Incomplete hand: {0}")]
    Incomplete(String),
}

pub type ParseResult<T> = Result<T, ParseError>;

pub trait SiteParser {
    fn parse_file(&self, content: &str, hero: &str) -> Vec<ParseResult<Hand>>;
    fn detect(content: &str) -> bool;
}

/// Auto-detect site and parse file
pub fn parse_auto(content: &str, hero: &str) -> Vec<ParseResult<Hand>> {
    if acr::AcrParser::detect(content) {
        let parser = acr::AcrParser;
        parser.parse_file(content, hero)
    } else {
        vec![Err(ParseError::UnknownSite)]
    }
}

/// Parse a card string like "Ah" -> Card
pub fn parse_card(s: &str) -> ParseResult<Card> {
    let s = s.trim();
    if s == "-" {
        // Partial card showing (e.g., "[- Jc]") — skip
        return Err(ParseError::Card("partial card '-'".into()));
    }
    let mut chars = s.chars();
    let rank_ch = chars.next().ok_or_else(|| ParseError::Card(s.into()))?;
    let suit_ch = chars.next().ok_or_else(|| ParseError::Card(s.into()))?;
    let rank = Rank::from_char(rank_ch).ok_or_else(|| ParseError::Card(s.into()))?;
    let suit = Suit::from_char(suit_ch).ok_or_else(|| ParseError::Card(s.into()))?;
    Ok(Card { rank, suit })
}

/// Parse cards from bracket notation like "[Ah Kd]"
pub fn parse_cards(s: &str) -> Vec<Card> {
    let s = s.trim().trim_start_matches('[').trim_end_matches(']');
    s.split_whitespace()
        .filter_map(|c| parse_card(c).ok())
        .collect()
}

/// Parse a money string: "$0.05" -> USD, "5000.00" -> Chips
pub fn parse_money(s: &str) -> ParseResult<Money> {
    let s = s.trim();
    if s.starts_with('$') {
        let amount: f64 = s[1..]
            .parse()
            .map_err(|_| ParseError::Money(s.into()))?;
        Ok(Money {
            amount,
            currency: Currency::USD,
        })
    } else {
        let amount: f64 = s.parse().map_err(|_| ParseError::Money(s.into()))?;
        Ok(Money {
            amount,
            currency: Currency::Chips,
        })
    }
}

/// Split a file into individual hand texts, separated by blank lines
pub fn split_hands(content: &str) -> Vec<&str> {
    // Split on double newlines (handles both \n\n and \r\n\r\n)
    let mut hands = Vec::new();
    // Normalize: find sections separated by one or more blank lines
    let mut remaining = content;
    while !remaining.is_empty() {
        // Skip leading whitespace/newlines
        let trimmed = remaining.trim_start_matches(|c: char| c == '\n' || c == '\r' || c == ' ');
        if trimmed.is_empty() {
            break;
        }
        remaining = trimmed;

        // Find the next blank line (a line that is empty or only whitespace)
        // A blank line is \n\n or \n\r\n or \r\n\r\n
        let end = find_blank_line_boundary(remaining);
        let hand_text = remaining[..end].trim_end();
        if !hand_text.is_empty() {
            hands.push(hand_text);
        }
        remaining = &remaining[end..];
    }

    hands
}

fn find_blank_line_boundary(s: &str) -> usize {
    let bytes = s.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    while i < len {
        if bytes[i] == b'\n' {
            // Check if next line is empty
            let mut j = i + 1;
            // Skip optional \r
            if j < len && bytes[j] == b'\r' {
                j += 1;
            }
            if j >= len {
                return len; // end of string
            }
            if bytes[j] == b'\n' {
                return i + 1; // boundary after the \n
            }
        } else if bytes[i] == b'\r' && i + 1 < len && bytes[i + 1] == b'\n' {
            // \r\n — check if next line is blank
            let j = i + 2;
            if j >= len {
                return len;
            }
            // Next line starts at j. If it's \r\n or \n, it's blank.
            if bytes[j] == b'\n' || (bytes[j] == b'\r' && j + 1 < len && bytes[j + 1] == b'\n') {
                return i + 2; // boundary after the \r\n
            }
        }
        i += 1;
    }
    len
}

/// Calculate position based on seat number relative to button
pub fn calculate_position(
    seat: u8,
    button_seat: u8,
    active_seats: &[u8],
) -> Option<Position> {
    let n = active_seats.len();
    if n == 0 {
        return None;
    }

    // Find button index in active seats
    let btn_idx = active_seats.iter().position(|&s| s == button_seat)?;

    // Find this seat's index
    let seat_idx = active_seats.iter().position(|&s| s == seat)?;

    // Calculate clockwise distance from button
    let distance = if seat_idx >= btn_idx {
        seat_idx - btn_idx
    } else {
        n - btn_idx + seat_idx
    };

    // Position assignments based on distance from button and table size
    // distance 0 = BTN, 1 = SB, 2 = BB, then from BB going clockwise: UTG, MP1, MP2, LJ, HJ, CO
    match n {
        2 => match distance {
            0 => Some(Position::BTN), // BTN/SB in heads-up
            1 => Some(Position::BB),
            _ => None,
        },
        3 => match distance {
            0 => Some(Position::BTN),
            1 => Some(Position::SB),
            2 => Some(Position::BB),
            _ => None,
        },
        4 => match distance {
            0 => Some(Position::BTN),
            1 => Some(Position::SB),
            2 => Some(Position::BB),
            3 => Some(Position::CO),
            _ => None,
        },
        5 => match distance {
            0 => Some(Position::BTN),
            1 => Some(Position::SB),
            2 => Some(Position::BB),
            3 => Some(Position::UTG),
            4 => Some(Position::CO),
            _ => None,
        },
        6 => match distance {
            0 => Some(Position::BTN),
            1 => Some(Position::SB),
            2 => Some(Position::BB),
            3 => Some(Position::UTG),
            4 => Some(Position::HJ),
            5 => Some(Position::CO),
            _ => None,
        },
        7 => match distance {
            0 => Some(Position::BTN),
            1 => Some(Position::SB),
            2 => Some(Position::BB),
            3 => Some(Position::UTG),
            4 => Some(Position::MP1),
            5 => Some(Position::HJ),
            6 => Some(Position::CO),
            _ => None,
        },
        8 => match distance {
            0 => Some(Position::BTN),
            1 => Some(Position::SB),
            2 => Some(Position::BB),
            3 => Some(Position::UTG),
            4 => Some(Position::MP1),
            5 => Some(Position::MP2),
            6 => Some(Position::HJ),
            7 => Some(Position::CO),
            _ => None,
        },
        9 => match distance {
            0 => Some(Position::BTN),
            1 => Some(Position::SB),
            2 => Some(Position::BB),
            3 => Some(Position::UTG),
            4 => Some(Position::MP1),
            5 => Some(Position::MP2),
            6 => Some(Position::LJ),
            7 => Some(Position::HJ),
            8 => Some(Position::CO),
            _ => None,
        },
        _ => None,
    }
}
