use super::*;
use super::header::{parse_header, parse_table_line};

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
