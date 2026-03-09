use rmcp::model::{ErrorCode, ErrorData};

use crate::types::{Card, Rank};

pub fn mcp_error(msg: &str) -> ErrorData {
    ErrorData {
        code: ErrorCode::INTERNAL_ERROR,
        message: msg.to_string().into(),
        data: None,
    }
}

/// Days since an arbitrary epoch (2000-01-03, a Monday) for week alignment.
pub fn days_from_ymd(y: i32, m: u32, d: u32) -> i64 {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let y = if m <= 2 { y as i64 - 1 } else { y as i64 };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400) as u64;
    let m = m as u64;
    let d = d as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days_abs = era * 146097 + doe as i64 - 719468; // days since 1970-01-01
    days_abs - 10957 // offset to 2000-01-03 (Monday)
}

pub fn ymd_from_days(days: i64) -> (i32, u32, u32) {
    let days_abs = days + 10957; // back to 1970-01-01 epoch
    let z = days_abs + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m as u32, d as u32)
}

pub fn rank_order(rank: Rank) -> u8 {
    match rank {
        Rank::Two => 2,
        Rank::Three => 3,
        Rank::Four => 4,
        Rank::Five => 5,
        Rank::Six => 6,
        Rank::Seven => 7,
        Rank::Eight => 8,
        Rank::Nine => 9,
        Rank::Ten => 10,
        Rank::Jack => 11,
        Rank::Queen => 12,
        Rank::King => 13,
        Rank::Ace => 14,
    }
}

pub fn combo_label(cards: &[Card]) -> Option<String> {
    if cards.len() != 2 {
        return None;
    }
    let (c1, c2) = (&cards[0], &cards[1]);
    let r1 = rank_order(c1.rank);
    let r2 = rank_order(c2.rank);
    let (high, low) = if r1 >= r2 { (c1, c2) } else { (c2, c1) };
    if high.rank == low.rank {
        Some(format!("{}{}", high.rank, low.rank))
    } else if high.suit == low.suit {
        Some(format!("{}{}s", high.rank, low.rank))
    } else {
        Some(format!("{}{}o", high.rank, low.rank))
    }
}

pub fn dir_size(path: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let ft = entry.file_type();
            if let Ok(ft) = ft {
                if ft.is_file() {
                    total += entry.metadata().map(|m| m.len()).unwrap_or(0);
                } else if ft.is_dir() {
                    total += dir_size(&entry.path());
                }
            }
        }
    }
    total
}
