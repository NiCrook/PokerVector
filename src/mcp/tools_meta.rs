use std::collections::{HashMap, HashSet};

use rmcp::model::*;

use crate::action_encoder;
use crate::config;
use crate::importer;
use crate::parsers;
use crate::storage::HandEmbeddings;
use crate::summarizer;
use crate::types::GameType;

use super::helpers::{dir_size, mcp_error};
use super::params::{
    GetDatabaseHealthParams, GetLastImportParams, ReimportHandParams, WatchDirectoryParams,
};
use super::PokerVectorMcp;

impl PokerVectorMcp {
    pub(crate) async fn tool_watch_directory(
        &self,
        params: WatchDirectoryParams,
    ) -> Result<CallToolResult, ErrorData> {
        let mut embedder = self.embedder.lock().await;

        let mut total_imported = 0u64;
        let mut total_skipped = 0u64;
        let mut total_errors = 0u64;
        let mut accounts_checked = 0u64;

        if let Some(path_str) = params.path {
            let path = std::path::PathBuf::from(&path_str);
            let result =
                importer::import_directory(&path, &self.hero, &mut *embedder, &self.store)
                    .await
                    .map_err(|e| mcp_error(&format!("Import failed: {}", e)))?;
            total_imported += result.imported;
            total_skipped += result.skipped;
            total_errors += result.errors;
            accounts_checked = 1;
        } else {
            if self.accounts.is_empty() {
                return Ok(CallToolResult::success(vec![Content::text(
                    serde_json::json!({
                        "error": "No accounts configured. Run `pokervector scan` or `pokervector add-account` first."
                    })
                    .to_string(),
                )]));
            }
            for account in &self.accounts {
                let result = importer::import_directory(
                    &account.path,
                    &account.hero,
                    &mut *embedder,
                    &self.store,
                )
                .await
                .map_err(|e| mcp_error(&format!("Import failed for {}: {}", account.hero, e)))?;
                total_imported += result.imported;
                total_skipped += result.skipped;
                total_errors += result.errors;
                accounts_checked += 1;
            }
        }

        // Update import log in config
        let timestamp = chrono::Utc::now().to_rfc3339();
        if let Ok(mut cfg) = config::load_config() {
            cfg.last_import = Some(config::ImportLog {
                timestamp: timestamp.clone(),
                hands_imported: total_imported,
                hands_skipped: total_skipped,
                errors: total_errors,
            });
            let _ = config::save_config(&cfg);
        }

        let response = serde_json::json!({
            "imported": total_imported,
            "skipped": total_skipped,
            "errors": total_errors,
            "accounts_checked": accounts_checked,
            "timestamp": timestamp,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_last_import(
        &self,
        _params: GetLastImportParams,
    ) -> Result<CallToolResult, ErrorData> {
        let cfg = config::load_config()
            .map_err(|e| mcp_error(&format!("Failed to load config: {}", e)))?;

        let total_hands = self
            .store
            .count()
            .await
            .map_err(|e| mcp_error(&format!("Failed to count hands: {}", e)))?;

        let response = serde_json::json!({
            "last_import": cfg.last_import,
            "total_hands_in_db": total_hands,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_reimport_hand(
        &self,
        params: ReimportHandParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hand = self
            .store
            .get_hand(params.hand_id)
            .await
            .map_err(|e| mcp_error(&format!("Failed to retrieve hand: {}", e)))?;

        let hand = match hand {
            Some(h) => h,
            None => {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "Hand {} not found",
                    params.hand_id
                ))]))
            }
        };

        let hero_name = hand.hero.as_deref().unwrap_or(&self.hero);
        let raw_text = &hand.raw_text;

        // Re-parse
        let results = parsers::parse_auto(raw_text, hero_name);
        let new_hand = results
            .into_iter()
            .find_map(|r| r.ok())
            .ok_or_else(|| mcp_error("Failed to re-parse hand from raw text"))?;

        // Re-summarize and re-encode
        let summary = summarizer::summarize(&new_hand);
        let action_enc = action_encoder::encode_action_sequence(&new_hand, hero_name);

        // Re-embed
        let mut embedder = self.embedder.lock().await;
        let vectors = embedder
            .embed_batch(&[&summary, &action_enc])
            .map_err(|e| mcp_error(&format!("Embedding failed: {}", e)))?;

        let embeddings = HandEmbeddings {
            summary: vectors[0].clone(),
            action: vectors[1].clone(),
        };

        // Upsert
        self.store
            .upsert_hand(&new_hand, &summary, &action_enc, embeddings)
            .await
            .map_err(|e| mcp_error(&format!("Upsert failed: {}", e)))?;

        let response = serde_json::json!({
            "hand_id": new_hand.id,
            "status": "reimported",
            "summary": summary,
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }

    pub(crate) async fn tool_get_database_health(
        &self,
        _params: GetDatabaseHealthParams,
    ) -> Result<CallToolResult, ErrorData> {
        let hands = self
            .store
            .scroll_hands(None)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let total = hands.len();
        let mut cash_count = 0u64;
        let mut tournament_count = 0u64;
        let mut variant_counts: HashMap<String, u64> = HashMap::new();
        let mut stakes_counts: HashMap<String, u64> = HashMap::new();
        let mut heroes: HashSet<String> = HashSet::new();
        let mut min_ts: Option<&str> = None;
        let mut max_ts: Option<&str> = None;
        let mut missing_hero = 0u64;
        let mut missing_cards = 0u64;

        for hand in &hands {
            match &hand.game_type {
                GameType::Cash { .. } => cash_count += 1,
                GameType::Tournament { .. } => tournament_count += 1,
            }
            *variant_counts
                .entry(format!("{}", hand.variant))
                .or_default() += 1;
            let stakes = match &hand.game_type {
                GameType::Cash {
                    small_blind,
                    big_blind,
                    ..
                } => format!("{}/{}", small_blind, big_blind),
                GameType::Tournament { .. } => "tournament".to_string(),
            };
            *stakes_counts.entry(stakes).or_default() += 1;

            if let Some(ref h) = hand.hero {
                heroes.insert(h.clone());
            } else {
                missing_hero += 1;
            }
            if hand.hero_cards.is_empty() {
                missing_cards += 1;
            }

            let ts = hand.timestamp.as_str();
            min_ts = Some(match min_ts {
                Some(m) if m < ts => m,
                _ => ts,
            });
            max_ts = Some(match max_ts {
                Some(m) if m > ts => m,
                _ => ts,
            });
        }

        // Calculate storage size
        let data_dir = config::data_dir();
        let storage_bytes = dir_size(&data_dir);
        let storage_mb = storage_bytes as f64 / (1024.0 * 1024.0);

        let response = serde_json::json!({
            "total_hands": total,
            "cash_hands": cash_count,
            "tournament_hands": tournament_count,
            "variants": variant_counts,
            "stakes": stakes_counts,
            "date_range": {
                "earliest": min_ts.unwrap_or("N/A"),
                "latest": max_ts.unwrap_or("N/A"),
            },
            "heroes": heroes.into_iter().collect::<Vec<_>>(),
            "data_quality": {
                "hands_missing_hero": missing_hero,
                "hands_missing_cards": missing_cards,
            },
            "storage_mb": format!("{:.1}", storage_mb),
        });

        let json = serde_json::to_string_pretty(&response)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;
        Ok(CallToolResult::success(vec![Content::text(json)]))
    }
}
