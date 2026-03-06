use rmcp::model::*;

use crate::stats;
use crate::types::GameType;

use super::helpers::mcp_error;
use super::PokerVectorMcp;

const HERO_STATS_URI: &str = "pokervector://hero-stats";
const DATABASE_INFO_URI: &str = "pokervector://database-info";

impl PokerVectorMcp {
    pub(crate) fn list_resource_entries(&self) -> Vec<Resource> {
        vec![
            RawResource {
                uri: HERO_STATS_URI.to_string(),
                name: "Hero Stats".to_string(),
                title: Some("Hero Aggregate Statistics".to_string()),
                description: Some(format!(
                    "Current aggregate statistics for {} (VPIP, PFR, 3-bet%, c-bet, winrate, etc.)",
                    self.hero
                )),
                mime_type: Some("application/json".to_string()),
                size: None,
                icons: None,
                meta: None,
            }
            .no_annotation(),
            RawResource {
                uri: DATABASE_INFO_URI.to_string(),
                name: "Database Info".to_string(),
                title: Some("Database Overview".to_string()),
                description: Some(
                    "Hand count, date range, stakes, variants, and storage info".to_string(),
                ),
                mime_type: Some("application/json".to_string()),
                size: None,
                icons: None,
                meta: None,
            }
            .no_annotation(),
        ]
    }

    pub(crate) async fn read_resource_by_uri(
        &self,
        uri: &str,
    ) -> Result<ReadResourceResult, ErrorData> {
        match uri {
            HERO_STATS_URI => self.read_hero_stats().await,
            DATABASE_INFO_URI => self.read_database_info().await,
            _ => Err(ErrorData::invalid_params(
                format!("Unknown resource URI: {}", uri),
                None,
            )),
        }
    }

    async fn read_hero_stats(&self) -> Result<ReadResourceResult, ErrorData> {
        let hands = self
            .store
            .scroll_hands(None)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let player_stats = stats::calculate_stats(&hands, &self.hero);
        let json = serde_json::to_string_pretty(&player_stats)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::TextResourceContents {
                uri: HERO_STATS_URI.to_string(),
                mime_type: Some("application/json".to_string()),
                text: json,
                meta: None,
            }],
        })
    }

    async fn read_database_info(&self) -> Result<ReadResourceResult, ErrorData> {
        let hands = self
            .store
            .scroll_hands(None)
            .await
            .map_err(|e| mcp_error(&format!("Failed to scroll hands: {}", e)))?;

        let total = hands.len();

        // Date range
        let (earliest, latest) = if hands.is_empty() {
            (None, None)
        } else {
            let mut timestamps: Vec<&str> = hands.iter().map(|h| h.timestamp.as_str()).collect();
            timestamps.sort();
            (
                timestamps.first().map(|s| s.to_string()),
                timestamps.last().map(|s| s.to_string()),
            )
        };

        // Breakdowns
        let mut stakes_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut variant_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        let mut game_type_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for hand in &hands {
            *stakes_counts
                .entry(format!("{}", hand.game_type))
                .or_default() += 1;
            *variant_counts
                .entry(format!("{:?}", hand.variant))
                .or_default() += 1;
            let gt = match hand.game_type {
                GameType::Cash { .. } => "cash",
                GameType::Tournament { .. } => "tournament",
            };
            *game_type_counts.entry(gt.to_string()).or_default() += 1;
        }

        let villains = stats::list_villains(&hands, &self.hero, 1);

        let info = serde_json::json!({
            "hero": self.hero,
            "total_hands": total,
            "unique_opponents": villains.len(),
            "date_range": {
                "earliest": earliest,
                "latest": latest,
            },
            "stakes": stakes_counts,
            "variants": variant_counts,
            "game_types": game_type_counts,
        });

        let json = serde_json::to_string_pretty(&info)
            .map_err(|e| mcp_error(&format!("Serialization failed: {}", e)))?;

        Ok(ReadResourceResult {
            contents: vec![ResourceContents::TextResourceContents {
                uri: DATABASE_INFO_URI.to_string(),
                mime_type: Some("application/json".to_string()),
                text: json,
                meta: None,
            }],
        })
    }
}
