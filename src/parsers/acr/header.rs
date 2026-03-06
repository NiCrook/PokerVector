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

pub(super) struct HeaderInfo {
    pub hand_id: u64,
    pub game_type: GameType,
    pub timestamp: String,
    pub variant: PokerVariant,
    pub betting_limit: BettingLimit,
    pub is_hi_lo: bool,
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

pub(super) fn parse_header(line: &str) -> ParseResult<HeaderInfo> {
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

pub(super) fn parse_table_line(line: &str) -> ParseResult<(String, u8, u8)> {
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

pub(super) fn parse_seat_line(line: &str, game_type: &GameType) -> Option<Player> {
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
