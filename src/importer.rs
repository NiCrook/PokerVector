use anyhow::Result;
use std::path::Path;

use crate::action_encoder;
use crate::embedder::Embedder;
use crate::parsers;
use crate::storage::{HandEmbeddings, VectorStore};
use crate::summarizer;
use crate::types;

pub struct ImportResult {
    pub imported: u64,
    pub skipped: u64,
    pub errors: u64,
}

/// Import hand histories from a directory. Returns structured result (no stdout output).
pub async fn import_directory(
    path: &Path,
    hero: &str,
    embedder: &mut Embedder,
    store: &VectorStore,
) -> Result<ImportResult> {
    // Phase 1: Parse all hands
    let pattern = path.join("*.txt");
    let pattern_str = pattern.to_string_lossy();

    let mut all_hands: Vec<types::Hand> = Vec::new();
    let mut total_errors = 0u64;

    for entry in glob::glob(&pattern_str)? {
        let file_path = entry?;
        let content = std::fs::read_to_string(&file_path)?;
        let results = parsers::parse_auto(&content, hero);

        for result in results {
            match result {
                Ok(hand) => all_hands.push(hand),
                Err(_) => total_errors += 1,
            }
        }
    }

    if all_hands.is_empty() {
        return Ok(ImportResult {
            imported: 0,
            skipped: 0,
            errors: total_errors,
        });
    }

    // Phase 2: Summarize, embed, and store in batches
    let batch_size = 32;
    let mut imported = 0u64;
    let mut skipped = 0u64;

    for chunk in all_hands.chunks(batch_size) {
        let mut to_process: Vec<&types::Hand> = Vec::new();
        for hand in chunk {
            if store.hand_exists(hand.id).await? {
                skipped += 1;
            } else {
                to_process.push(hand);
            }
        }

        if to_process.is_empty() {
            continue;
        }

        let summaries: Vec<String> = to_process
            .iter()
            .map(|h| summarizer::summarize(h))
            .collect();
        let action_encodings: Vec<String> = to_process
            .iter()
            .map(|h| action_encoder::encode_action_sequence(h, hero))
            .collect();

        let summary_refs: Vec<&str> = summaries.iter().map(|s| s.as_str()).collect();
        let action_refs: Vec<&str> = action_encodings.iter().map(|s| s.as_str()).collect();

        let summary_embeddings = embedder.embed_batch(&summary_refs)?;
        let action_embeddings = embedder.embed_batch(&action_refs)?;

        let batch: Vec<(&types::Hand, &str, &str, HandEmbeddings)> = to_process
            .into_iter()
            .zip(summaries.iter())
            .zip(action_encodings.iter())
            .zip(summary_embeddings.into_iter().zip(action_embeddings.into_iter()))
            .map(|(((hand, summary), action_enc), (sum_emb, act_emb))| {
                (hand, summary.as_str(), action_enc.as_str(), HandEmbeddings {
                    summary: sum_emb,
                    action: act_emb,
                })
            })
            .collect();

        let batch_count = batch.len() as u64;
        store.upsert_hands_batch(batch).await?;
        imported += batch_count;
    }

    Ok(ImportResult {
        imported,
        skipped,
        errors: total_errors,
    })
}
