use std::time::Instant;
use pokervector::parsers;
use pokervector::action_encoder;
use pokervector::summarizer;
use pokervector::stats;
use pokervector::sessions;
use pokervector::types::Hand;

const HERO: &str = "TestHero";
const ITERS: u32 = 10;

fn load_all() -> String {
    let dir = std::env::var("POKERVECTOR_HH_DIR")
        .unwrap_or_else(|_| "tests/fixtures".to_string());
    let pattern = format!("{}/*.txt", dir);
    let mut all = String::new();
    for path in glob::glob(&pattern)
        .unwrap()
        .flatten()
    {
        all.push_str(&std::fs::read_to_string(&path).unwrap());
        all.push_str("\n\n");
    }
    all
}

fn parse_all(content: &str) -> Vec<Hand> {
    parsers::parse_auto(content, HERO)
        .into_iter()
        .filter_map(|r| r.ok())
        .collect()
}

fn time<F: FnMut()>(label: &str, iters: u32, mut f: F) {
    // warmup
    f();
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / iters;
    println!("{label:>30}: {per_iter:>10.2?}  ({iters} iters, total {elapsed:.2?})");
}

fn main() {
    let content = load_all();
    let raw_hands = parsers::split_hands(&content);
    let hands = parse_all(&content);

    // Serialize hands to JSON (simulates scroll_hands DB read)
    let jsons: Vec<String> = hands.iter().map(|h| serde_json::to_string(h).unwrap()).collect();
    let json_kb: f64 = jsons.iter().map(|j| j.len()).sum::<usize>() as f64 / 1024.0;

    println!("=== PokerVector Hot Path Profile ===");
    println!("  {} raw hand texts, {} parsed hands", raw_hands.len(), hands.len());
    println!("  {json_kb:.1} KB total JSON");
    println!();

    // 1. split_hands
    time("split_hands", ITERS, || {
        std::hint::black_box(parsers::split_hands(&content));
    });

    // 2. parse_auto (split + parse) — audit item 1 (regex recompilation)
    time("parse_auto (split+parse)", ITERS, || {
        std::hint::black_box(parsers::parse_auto(&content, HERO));
    });

    // 3. summarize (all hands)
    time("summarize (all)", ITERS, || {
        for h in &hands {
            std::hint::black_box(summarizer::summarize(h));
        }
    });

    // 4. action_encode (all hands) — audit items 5, 9, 10
    time("action_encode (all)", ITERS, || {
        for h in &hands {
            std::hint::black_box(action_encoder::encode_action_sequence(h, HERO));
        }
    });

    // 5. calculate_stats — audit item 10 (multi-pass)
    time("calculate_stats", ITERS, || {
        std::hint::black_box(stats::calculate_stats(&hands, HERO));
    });

    // 6. detect_sessions — audit item 8 (timestamp parse in sort)
    time("detect_sessions", ITERS, || {
        std::hint::black_box(sessions::detect_sessions(hands.clone(), HERO));
    });

    // 7. JSON deserialize (all hands) — audit items 4, 7
    time("json_deser (all)", ITERS, || {
        for j in &jsons {
            std::hint::black_box(serde_json::from_str::<Hand>(j).unwrap());
        }
    });

    // 8. JSON deser + calculate_stats (simulates full MCP get_stats path)
    time("json_deser + stats", ITERS, || {
        let deserialized: Vec<Hand> = jsons.iter()
            .map(|j| serde_json::from_str(j).unwrap())
            .collect();
        std::hint::black_box(stats::calculate_stats(&deserialized, HERO));
    });
}
