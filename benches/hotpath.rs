use std::time::Instant;
use pokervector::parsers;
use pokervector::action_encoder;
use pokervector::summarizer;
use pokervector::embedder::Embedder;
use pokervector::stats;
use pokervector::sessions;
use pokervector::types::Hand;

const HERO: &str = "TestHero";
const ITERS: u32 = 10;
const EMBED_ITERS: u32 = 3; // fewer iters for slow ONNX inference

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

    // 9. Embedding (ONNX inference) — the import bottleneck
    let summaries: Vec<String> = hands.iter().map(|h| summarizer::summarize(h)).collect();
    let action_encodings: Vec<String> = hands.iter()
        .map(|h| action_encoder::encode_action_sequence(h, HERO))
        .collect();

    println!();
    println!("--- Embedding (batch_size=32) ---");

    let mut embedder = Embedder::new().expect("Failed to load embedding model");

    let summary_refs: Vec<&str> = summaries.iter().map(|s| s.as_str()).collect();
    let action_refs: Vec<&str> = action_encodings.iter().map(|s| s.as_str()).collect();

    // Single batch of 32 summaries (to get per-batch cost)
    let sample_batch: Vec<&str> = summary_refs.iter().take(32).copied().collect();
    time("embed_batch (32 summaries)", EMBED_ITERS, || {
        std::hint::black_box(embedder.embed_batch(&sample_batch).unwrap());
    });

    let sample_actions: Vec<&str> = action_refs.iter().take(32).copied().collect();
    time("embed_batch (32 actions)", EMBED_ITERS, || {
        std::hint::black_box(embedder.embed_batch(&sample_actions).unwrap());
    });

    // Full embed pipeline: all hands in batches of 32 (unsorted — original order)
    time("embed summaries (unsorted)", EMBED_ITERS, || {
        for chunk in summary_refs.chunks(32) {
            std::hint::black_box(embedder.embed_batch(chunk).unwrap());
        }
    });

    // Sorted by length — reduces padding waste
    let mut sorted_summaries: Vec<&str> = summary_refs.clone();
    sorted_summaries.sort_by_key(|s| s.len());

    time("embed summaries (sorted)", EMBED_ITERS, || {
        for chunk in sorted_summaries.chunks(32) {
            std::hint::black_box(embedder.embed_batch(chunk).unwrap());
        }
    });

    time("embed ALL actions", EMBED_ITERS, || {
        for chunk in action_refs.chunks(32) {
            std::hint::black_box(embedder.embed_batch(chunk).unwrap());
        }
    });

    // Total import pipeline (no DB): parse + summarize + encode + embed (sorted)
    println!();
    println!("--- Full Import Pipeline (no DB) ---");
    time("pipeline (sorted)", EMBED_ITERS, || {
        let hs = parse_all(&content);
        let mut work: Vec<(String, String)> = hs.iter()
            .map(|h| (summarizer::summarize(h), action_encoder::encode_action_sequence(h, HERO)))
            .collect();
        work.sort_by_key(|(s, _)| s.len());
        for chunk in work.chunks(32) {
            let sum_refs: Vec<&str> = chunk.iter().map(|(s, _)| s.as_str()).collect();
            let act_refs: Vec<&str> = chunk.iter().map(|(_, a)| a.as_str()).collect();
            std::hint::black_box(embedder.embed_batch(&sum_refs).unwrap());
            std::hint::black_box(embedder.embed_batch(&act_refs).unwrap());
        }
    });
}
