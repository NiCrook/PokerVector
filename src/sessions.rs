use crate::stats::{self, PlayerStats};
use crate::types::*;
use chrono::NaiveDateTime;

const TABLE_SESSION_GAP_MINUTES: i64 = 5;
const MULTI_TABLE_SESSION_GAP_MINUTES: i64 = 30;
const TIMESTAMP_FORMAT: &str = "%Y/%m/%d %H:%M:%S";

#[derive(Debug, Clone, serde::Serialize)]
pub struct TableSession {
    pub table_name: String,
    pub stakes: String,
    pub hands: Vec<Hand>,
    pub start_time: String,
    pub end_time: String,
    pub hand_count: usize,
    pub net_profit: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Session {
    pub session_id: u32,
    pub start_time: String,
    pub end_time: String,
    pub duration_minutes: u64,
    pub tables: Vec<TableSession>,
    pub total_hands: usize,
    pub net_profit: f64,
    pub net_profit_bb: f64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SessionReview {
    pub session: Session,
    pub stats: PlayerStats,
    pub notable_hands: Vec<NotableHand>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct NotableHand {
    pub hand_id: u64,
    pub reason: String,
    pub profit: f64,
    pub summary: String,
}

fn parse_timestamp(ts: &str) -> Option<NaiveDateTime> {
    NaiveDateTime::parse_from_str(ts, TIMESTAMP_FORMAT).ok()
}

fn stakes_string(hand: &Hand) -> String {
    match &hand.game_type {
        GameType::Cash {
            small_blind,
            big_blind,
            ..
        } => {
            format!("{}/{}", small_blind, big_blind)
        }
        GameType::Tournament {
            level,
            small_blind,
            big_blind,
            ..
        } => {
            format!("L{} {}/{}", level, small_blind, big_blind)
        }
    }
}

fn big_blind_amount(hand: &Hand) -> f64 {
    match &hand.game_type {
        GameType::Cash { big_blind, .. } => big_blind.amount,
        GameType::Tournament { big_blind, .. } => big_blind.amount,
    }
}

/// Calculate hero's net profit for a single hand.
/// Profit = amount collected - amount invested (blinds, antes, calls, bets, raises).
fn hero_profit(hand: &Hand) -> f64 {
    let hero = match &hand.hero {
        Some(h) => h,
        None => return 0.0,
    };

    let mut invested = 0.0f64;
    let mut collected = 0.0f64;

    for action in &hand.actions {
        if action.player != *hero {
            continue;
        }
        match &action.action_type {
            ActionType::PostSmallBlind { amount, .. }
            | ActionType::PostBigBlind { amount, .. }
            | ActionType::PostAnte { amount }
            | ActionType::PostBlind { amount }
            | ActionType::BringsIn { amount }
            | ActionType::Call { amount, .. }
            | ActionType::Bet { amount, .. } => {
                invested += amount.amount;
            }
            ActionType::Raise { to, .. } => {
                invested += to.amount;
            }
            ActionType::UncalledBet { amount } => {
                invested -= amount.amount;
            }
            ActionType::Collected { amount, .. } => {
                collected += amount.amount;
            }
            _ => {}
        }
    }

    // Also check winners in result (some wins only appear in summary)
    for winner in &hand.result.winners {
        if winner.player == *hero {
            // Only add if not already counted via Collected actions
            let already_collected = hand.actions.iter().any(|a| {
                a.player == *hero && matches!(&a.action_type, ActionType::Collected { .. })
            });
            if !already_collected {
                collected += winner.amount.amount;
            }
        }
    }

    collected - invested
}

/// Filter to only cash hands.
fn cash_hands(hands: Vec<Hand>) -> Vec<Hand> {
    hands
        .into_iter()
        .filter(|h| matches!(h.game_type, GameType::Cash { .. }))
        .collect()
}

/// Detect table sessions from a list of cash hands.
/// Groups by table_name, sorts by timestamp, splits on gaps > 5 minutes.
pub fn detect_table_sessions(hands: &[Hand], hero: &str) -> Vec<TableSession> {
    use std::collections::HashMap;

    // Group hands by table name
    let mut by_table: HashMap<&str, Vec<&Hand>> = HashMap::new();
    for hand in hands {
        if hand.hero.as_deref() != Some(hero) {
            continue;
        }
        by_table.entry(&hand.table_name).or_default().push(hand);
    }

    let mut sessions = Vec::new();

    for (table_name, mut table_hands) in by_table {
        // Sort by timestamp
        table_hands.sort_by(|a, b| {
            let ta = parse_timestamp(&a.timestamp);
            let tb = parse_timestamp(&b.timestamp);
            ta.cmp(&tb)
        });

        // Split into sessions on gaps > TABLE_SESSION_GAP_MINUTES
        let mut current: Vec<&Hand> = Vec::new();

        for hand in table_hands {
            if let Some(last) = current.last() {
                if let (Some(prev_ts), Some(cur_ts)) = (
                    parse_timestamp(&last.timestamp),
                    parse_timestamp(&hand.timestamp),
                ) {
                    let gap = (cur_ts - prev_ts).num_minutes();
                    if gap > TABLE_SESSION_GAP_MINUTES {
                        // Flush current session
                        sessions.push(build_table_session(table_name, &current));
                        current.clear();
                    }
                }
            }
            current.push(hand);
        }

        if !current.is_empty() {
            sessions.push(build_table_session(table_name, &current));
        }
    }

    sessions
}

fn build_table_session(table_name: &str, hands: &[&Hand]) -> TableSession {
    let stakes = hands.first().map(|h| stakes_string(h)).unwrap_or_default();
    let start_time = hands
        .first()
        .map(|h| h.timestamp.clone())
        .unwrap_or_default();
    let end_time = hands
        .last()
        .map(|h| h.timestamp.clone())
        .unwrap_or_default();
    let net_profit: f64 = hands.iter().map(|h| hero_profit(h)).sum();
    let owned_hands: Vec<Hand> = hands.iter().map(|h| (*h).clone()).collect();

    TableSession {
        table_name: table_name.to_string(),
        stakes,
        hands: owned_hands,
        start_time,
        end_time,
        hand_count: hands.len(),
        net_profit,
    }
}

/// Detect multi-table sessions by merging table sessions with < 30 min gap.
pub fn detect_sessions(hands: Vec<Hand>, hero: &str) -> Vec<Session> {
    let cash = cash_hands(hands);
    if cash.is_empty() {
        return Vec::new();
    }

    let mut table_sessions = detect_table_sessions(&cash, hero);
    if table_sessions.is_empty() {
        return Vec::new();
    }

    // Sort all table sessions by start time
    table_sessions.sort_by(|a, b| {
        let ta = parse_timestamp(&a.start_time);
        let tb = parse_timestamp(&b.start_time);
        ta.cmp(&tb)
    });

    // Merge into multi-table sessions using 30-min inactivity timeout
    let mut sessions: Vec<Session> = Vec::new();
    let mut current_tables: Vec<TableSession> = vec![table_sessions.remove(0)];

    for ts in table_sessions {
        // Find the latest end_time across all tables in current session
        let latest_end = current_tables
            .iter()
            .filter_map(|t| parse_timestamp(&t.end_time))
            .max();

        let this_start = parse_timestamp(&ts.start_time);

        let should_merge = match (latest_end, this_start) {
            (Some(end), Some(start)) => {
                (start - end).num_minutes() <= MULTI_TABLE_SESSION_GAP_MINUTES
            }
            _ => true, // Can't parse, merge by default
        };

        if should_merge {
            current_tables.push(ts);
        } else {
            // Flush current session
            let id = sessions.len() as u32 + 1;
            sessions.push(build_session(id, current_tables));
            current_tables = vec![ts];
        }
    }

    // Flush final session
    if !current_tables.is_empty() {
        let id = sessions.len() as u32 + 1;
        sessions.push(build_session(id, current_tables));
    }

    // Sort most recent first
    sessions.reverse();
    sessions
}

fn build_session(session_id: u32, tables: Vec<TableSession>) -> Session {
    let start_time = tables
        .iter()
        .filter_map(|t| parse_timestamp(&t.start_time).map(|ts| (ts, t.start_time.clone())))
        .min_by_key(|(ts, _)| *ts)
        .map(|(_, s)| s)
        .unwrap_or_default();

    let end_time = tables
        .iter()
        .filter_map(|t| parse_timestamp(&t.end_time).map(|ts| (ts, t.end_time.clone())))
        .max_by_key(|(ts, _)| *ts)
        .map(|(_, s)| s)
        .unwrap_or_default();

    let duration_minutes = match (parse_timestamp(&start_time), parse_timestamp(&end_time)) {
        (Some(s), Some(e)) => (e - s).num_minutes().max(0) as u64,
        _ => 0,
    };

    let total_hands: usize = tables.iter().map(|t| t.hand_count).sum();
    let net_profit: f64 = tables.iter().map(|t| t.net_profit).sum();

    // Calculate net_profit_bb using weighted average big blind
    let net_profit_bb = if total_hands > 0 {
        let total_bb_amount: f64 = tables
            .iter()
            .flat_map(|t| t.hands.iter())
            .map(|h| big_blind_amount(h))
            .sum::<f64>();
        let avg_bb = total_bb_amount / total_hands as f64;
        if avg_bb > 0.0 {
            net_profit / avg_bb
        } else {
            0.0
        }
    } else {
        0.0
    };

    Session {
        session_id,
        start_time,
        end_time,
        duration_minutes,
        tables,
        total_hands,
        net_profit,
        net_profit_bb,
    }
}

/// Build a session review with stats and notable hands.
pub fn review_session(session: &Session, hero: &str, summaries: &[(u64, String)]) -> SessionReview {
    // Collect all hands from all tables
    let all_hands: Vec<Hand> = session
        .tables
        .iter()
        .flat_map(|t| t.hands.iter().cloned())
        .collect();

    let player_stats = stats::calculate_stats(&all_hands, hero);

    // Find notable hands (top 3 wins, top 3 losses)
    let mut hand_profits: Vec<(u64, f64)> =
        all_hands.iter().map(|h| (h.id, hero_profit(h))).collect();

    // Sort by profit descending for wins
    hand_profits.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    let summary_map: std::collections::HashMap<u64, &str> =
        summaries.iter().map(|(id, s)| (*id, s.as_str())).collect();

    let mut notable = Vec::new();

    // Top 3 wins
    for &(hand_id, profit) in hand_profits.iter().take(3) {
        if profit > 0.0 {
            notable.push(NotableHand {
                hand_id,
                reason: if notable.is_empty() {
                    "Biggest win".to_string()
                } else {
                    format!("#{} biggest win", notable.len() + 1)
                },
                profit,
                summary: summary_map.get(&hand_id).unwrap_or(&"").to_string(),
            });
        }
    }

    // Top 3 losses (most negative)
    let loss_start = notable.len();
    for &(hand_id, profit) in hand_profits.iter().rev().take(3) {
        if profit < 0.0 {
            let loss_idx = notable.len() - loss_start;
            notable.push(NotableHand {
                hand_id,
                reason: if loss_idx == 0 {
                    "Biggest loss".to_string()
                } else {
                    format!("#{} biggest loss", loss_idx + 1)
                },
                profit,
                summary: summary_map.get(&hand_id).unwrap_or(&"").to_string(),
            });
        }
    }

    SessionReview {
        session: session.clone(),
        stats: player_stats,
        notable_hands: notable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cash_hand(id: u64, table: &str, timestamp: &str, hero: &str) -> Hand {
        Hand {
            id,
            site: Site::ACR,
            variant: PokerVariant::Holdem,
            betting_limit: BettingLimit::NoLimit,
            is_hi_lo: false,
            is_bomb_pot: false,
            game_type: GameType::Cash {
                small_blind: Money {
                    amount: 0.01,
                    currency: Currency::USD,
                },
                big_blind: Money {
                    amount: 0.02,
                    currency: Currency::USD,
                },
                ante: None,
            },
            timestamp: timestamp.to_string(),
            table_name: table.to_string(),
            table_size: 6,
            button_seat: 1,
            players: vec![
                Player {
                    seat: 1,
                    name: hero.to_string(),
                    stack: Money {
                        amount: 2.0,
                        currency: Currency::USD,
                    },
                    position: Some(Position::BTN),
                    is_hero: true,
                    is_sitting_out: false,
                },
                Player {
                    seat: 2,
                    name: "Villain".to_string(),
                    stack: Money {
                        amount: 2.0,
                        currency: Currency::USD,
                    },
                    position: Some(Position::BB),
                    is_hero: false,
                    is_sitting_out: false,
                },
            ],
            hero: Some(hero.to_string()),
            hero_position: Some(Position::BTN),
            hero_cards: vec![],
            actions: vec![],
            board: vec![],
            pot: Some(Money {
                amount: 0.04,
                currency: Currency::USD,
            }),
            rake: None,
            result: HandResult {
                winners: vec![],
                hero_result: HeroResult::Folded,
            },
            raw_text: String::new(),
            stud_cards: None,
        }
    }

    #[test]
    fn test_table_session_splitting() {
        let hands = vec![
            make_cash_hand(1, "Table1", "2026/02/15 10:00:00", "Hero"),
            make_cash_hand(2, "Table1", "2026/02/15 10:02:00", "Hero"),
            make_cash_hand(3, "Table1", "2026/02/15 10:04:00", "Hero"),
            // 10 min gap — should split
            make_cash_hand(4, "Table1", "2026/02/15 10:14:00", "Hero"),
            make_cash_hand(5, "Table1", "2026/02/15 10:16:00", "Hero"),
        ];

        let sessions = detect_table_sessions(&hands, "Hero");
        assert_eq!(sessions.len(), 2);

        let mut sessions_sorted: Vec<_> = sessions.iter().collect();
        sessions_sorted.sort_by_key(|s| s.start_time.clone());

        assert_eq!(sessions_sorted[0].hand_count, 3);
        assert_eq!(sessions_sorted[1].hand_count, 2);
    }

    #[test]
    fn test_table_session_no_split() {
        let hands = vec![
            make_cash_hand(1, "Table1", "2026/02/15 10:00:00", "Hero"),
            make_cash_hand(2, "Table1", "2026/02/15 10:03:00", "Hero"),
            make_cash_hand(3, "Table1", "2026/02/15 10:05:00", "Hero"),
        ];

        let sessions = detect_table_sessions(&hands, "Hero");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].hand_count, 3);
    }

    #[test]
    fn test_multi_table_session_merging() {
        let hands = vec![
            // Table 1: 10:00 - 10:02
            make_cash_hand(1, "Table1", "2026/02/15 10:00:00", "Hero"),
            make_cash_hand(2, "Table1", "2026/02/15 10:02:00", "Hero"),
            // Table 2: overlapping 10:01 - 10:03
            make_cash_hand(3, "Table2", "2026/02/15 10:01:00", "Hero"),
            make_cash_hand(4, "Table2", "2026/02/15 10:03:00", "Hero"),
        ];

        let sessions = detect_sessions(hands, "Hero");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].total_hands, 4);
        assert_eq!(sessions[0].tables.len(), 2);
    }

    #[test]
    fn test_multi_table_session_split_on_gap() {
        let hands = vec![
            // Session 1: 10:00 - 10:20
            make_cash_hand(1, "Table1", "2026/02/15 10:00:00", "Hero"),
            make_cash_hand(2, "Table1", "2026/02/15 10:20:00", "Hero"),
            // Session 2: 11:00 - 11:10 (40 min gap > 30 min threshold)
            make_cash_hand(3, "Table1", "2026/02/15 11:00:00", "Hero"),
            make_cash_hand(4, "Table1", "2026/02/15 11:10:00", "Hero"),
        ];

        let sessions = detect_sessions(hands, "Hero");
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_single_hand_session() {
        let hands = vec![make_cash_hand(1, "Table1", "2026/02/15 10:00:00", "Hero")];

        let sessions = detect_sessions(hands, "Hero");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].total_hands, 1);
        assert_eq!(sessions[0].duration_minutes, 0);
    }

    #[test]
    fn test_no_cash_hands() {
        let mut hand = make_cash_hand(1, "Table1", "2026/02/15 10:00:00", "Hero");
        hand.game_type = GameType::Tournament {
            tournament_id: 123,
            level: 1,
            small_blind: Money {
                amount: 10.0,
                currency: Currency::Chips,
            },
            big_blind: Money {
                amount: 20.0,
                currency: Currency::Chips,
            },
            ante: None,
        };

        let sessions = detect_sessions(vec![hand], "Hero");
        assert_eq!(sessions.len(), 0);
    }

    #[test]
    fn test_profit_calculation() {
        let mut hand = make_cash_hand(1, "Table1", "2026/02/15 10:00:00", "Hero");
        hand.actions = vec![
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::PostSmallBlind {
                    amount: Money {
                        amount: 0.01,
                        currency: Currency::USD,
                    },
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Raise {
                    amount: Money {
                        amount: 0.04,
                        currency: Currency::USD,
                    },
                    to: Money {
                        amount: 0.06,
                        currency: Currency::USD,
                    },
                    all_in: false,
                },
                street: Street::Preflop,
            },
            Action {
                player: "Hero".to_string(),
                action_type: ActionType::Collected {
                    amount: Money {
                        amount: 0.12,
                        currency: Currency::USD,
                    },
                    pot: "Main pot".to_string(),
                },
                street: Street::Preflop,
            },
        ];

        let profit = hero_profit(&hand);
        // invested: 0.01 (SB) + 0.06 (raise to) = 0.07, collected: 0.12
        assert!((profit - 0.05).abs() < 0.001);
    }

    #[test]
    fn test_sessions_ordered_most_recent_first() {
        let hands = vec![
            make_cash_hand(1, "Table1", "2026/02/15 08:00:00", "Hero"),
            make_cash_hand(2, "Table1", "2026/02/15 08:10:00", "Hero"),
            make_cash_hand(3, "Table1", "2026/02/15 12:00:00", "Hero"),
            make_cash_hand(4, "Table1", "2026/02/15 12:10:00", "Hero"),
        ];

        let sessions = detect_sessions(hands, "Hero");
        assert_eq!(sessions.len(), 2);
        // Most recent first
        assert!(sessions[0].start_time > sessions[1].start_time);
    }
}
