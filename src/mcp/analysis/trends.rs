use crate::stats;
use crate::types::Hand;

use crate::mcp::helpers::{days_from_ymd, ymd_from_days};

pub fn get_trends_analysis(mut hands: Vec<Hand>, hero: &str, period: &str) -> serde_json::Value {
    if hands.is_empty() {
        return serde_json::json!({
            "periods": [],
            "total_hands": 0,
        });
    }

    hands.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let bucket_key = |ts: &str| -> String {
        let date_part = ts.split(' ').next().unwrap_or(ts);
        match period {
            "day" => date_part.to_string(),
            "month" => {
                let parts: Vec<&str> = date_part.split('/').collect();
                if parts.len() >= 2 {
                    format!("{}/{}", parts[0], parts[1])
                } else {
                    date_part.to_string()
                }
            }
            "week" | _ => {
                let parts: Vec<&str> = date_part.split('/').collect();
                if parts.len() == 3 {
                    let year: i32 = parts[0].parse().unwrap_or(0);
                    let month: u32 = parts[1].parse().unwrap_or(1);
                    let day: u32 = parts[2].parse().unwrap_or(1);
                    let days = days_from_ymd(year, month, day);
                    let dow = ((days % 7) + 7) % 7;
                    let monday = days - dow;
                    let (my, mm, md) = ymd_from_days(monday);
                    format!("{:04}/{:02}/{:02}", my, mm, md)
                } else {
                    date_part.to_string()
                }
            }
        }
    };

    let mut buckets: Vec<(String, Vec<&Hand>)> = Vec::new();
    for hand in &hands {
        let key = bucket_key(&hand.timestamp);
        if let Some(last) = buckets.last_mut() {
            if last.0 == key {
                last.1.push(hand);
                continue;
            }
        }
        buckets.push((key, vec![hand]));
    }

    let mut cumulative_profit = 0.0f64;
    let mut periods = Vec::new();
    for (key, bucket_hands) in &buckets {
        let owned: Vec<Hand> = bucket_hands.iter().map(|h| (*h).clone()).collect();
        let s = stats::calculate_stats(&owned, hero);

        let mut period_profit = 0.0f64;
        for hand in bucket_hands {
            let invested = stats::hero_invested(hand, hero);
            let collected = stats::hero_collected(hand, hero);
            period_profit += collected - invested;
        }
        cumulative_profit += period_profit;

        let label = match period {
            "week" => format!("week of {}", key),
            _ => key.clone(),
        };

        periods.push(serde_json::json!({
            "period": label,
            "hands": s.hands_played,
            "vpip": format!("{:.1}", s.vpip),
            "pfr": format!("{:.1}", s.pfr),
            "three_bet_pct": format!("{:.1}", s.three_bet_pct),
            "aggression_factor": format!("{:.2}", s.aggression_factor),
            "winrate_bb100": format!("{:.1}", s.winrate_bb100),
            "profit": format!("{:.2}", period_profit),
            "cumulative_profit": format!("{:.2}", cumulative_profit),
            "went_to_showdown_pct": format!("{:.1}", s.went_to_showdown_pct),
            "won_at_showdown_pct": format!("{:.1}", s.won_at_showdown_pct),
            "cbet_flop": format!("{:.1}", s.cbet_flop),
            "wwsf": format!("{:.1}", s.wwsf),
        }));
    }

    serde_json::json!({
        "period_type": period,
        "total_hands": hands.len(),
        "total_periods": periods.len(),
        "periods": periods,
    })
}
