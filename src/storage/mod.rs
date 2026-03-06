use std::sync::Arc;

use anyhow::{Context, Result};
use arrow_array::types::Float32Type;
use arrow_array::{
    Array, BooleanArray, FixedSizeListArray, Float64Array, RecordBatch, RecordBatchIterator,
    StringArray, UInt64Array, UInt8Array,
};
use arrow_schema::{DataType, Field, Schema};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use lancedb::table::NewColumnTransform;
use lancedb::{Connection, Table as LanceTable};

use crate::stats::classify_pot_type;
use crate::types::*;

const EMBEDDING_DIM: i32 = 384;

pub struct VectorStore {
    db: Connection,
    table_name: String,
    table: LanceTable,
}

#[derive(serde::Serialize)]
pub struct SearchResult {
    pub hand_id: u64,
    pub score: f32,
    pub summary: String,
    pub hero_position: String,
    pub hero_cards: String,
    pub stakes: String,
    pub hero_result: String,
    pub pot_type: String,
}

pub struct HandEmbeddings {
    pub summary: Vec<f32>,
    pub action: Vec<f32>,
}

fn table_schema() -> Arc<Schema> {
    Arc::new(Schema::new(vec![
        Field::new("id", DataType::UInt64, false),
        Field::new("hand_json", DataType::Utf8, false),
        Field::new("summary_text", DataType::Utf8, false),
        Field::new("action_text", DataType::Utf8, false),
        Field::new("site", DataType::Utf8, false),
        Field::new("game_type", DataType::Utf8, false),
        Field::new("variant", DataType::Utf8, false),
        Field::new("betting_limit", DataType::Utf8, false),
        Field::new("is_hi_lo", DataType::Boolean, false),
        Field::new("is_bomb_pot", DataType::Boolean, false),
        Field::new("stakes", DataType::Utf8, false),
        Field::new("table_size", DataType::UInt8, false),
        Field::new("hero", DataType::Utf8, false),
        Field::new("hero_position", DataType::Utf8, false),
        Field::new("hero_cards", DataType::Utf8, false),
        Field::new("hero_result", DataType::Utf8, false),
        Field::new("board", DataType::Utf8, false),
        Field::new("num_players", DataType::UInt64, false),
        Field::new("went_to_showdown", DataType::Boolean, false),
        Field::new("timestamp", DataType::Utf8, false),
        Field::new("pot_type", DataType::Utf8, false),
        Field::new("opponent_names", DataType::Utf8, false),
        Field::new("tournament_id", DataType::UInt64, false),
        Field::new("pot_amount", DataType::Float64, false),
        Field::new("tags", DataType::Utf8, false),
        Field::new(
            "summary",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIM,
            ),
            true,
        ),
        Field::new(
            "action",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                EMBEDDING_DIM,
            ),
            true,
        ),
    ]))
}

fn hero_result_str(result: &HandResult) -> &'static str {
    match result.hero_result {
        HeroResult::Won => "won",
        HeroResult::Lost => "lost",
        HeroResult::Folded => "folded",
        HeroResult::SatOut => "sat_out",
    }
}

fn variant_str(v: &PokerVariant) -> &'static str {
    match v {
        PokerVariant::Holdem => "holdem",
        PokerVariant::Omaha => "omaha",
        PokerVariant::FiveCardOmaha => "five_card_omaha",
        PokerVariant::SevenCardStud => "seven_card_stud",
    }
}

fn betting_limit_str(bl: &BettingLimit) -> &'static str {
    match bl {
        BettingLimit::NoLimit => "no_limit",
        BettingLimit::PotLimit => "pot_limit",
        BettingLimit::FixedLimit => "fixed_limit",
    }
}

fn stakes_str(game_type: &GameType) -> String {
    match game_type {
        GameType::Cash {
            small_blind,
            big_blind,
            ..
        } => format!("{}/{}", small_blind, big_blind),
        GameType::Tournament {
            level,
            small_blind,
            big_blind,
            ..
        } => format!("L{} {}/{}", level, small_blind, big_blind),
    }
}

fn went_to_showdown(hand: &Hand) -> bool {
    hand.actions
        .iter()
        .any(|a| a.street == Street::Showdown)
}

fn opponent_names_str(hand: &Hand) -> String {
    let names: Vec<&str> = hand
        .players
        .iter()
        .filter(|p| !p.is_hero && !p.is_sitting_out)
        .map(|p| p.name.as_str())
        .collect();
    if names.is_empty() {
        String::new()
    } else {
        format!(",{},", names.join(","))
    }
}

fn build_record_batch(
    hands: &[&Hand],
    summaries: &[&str],
    action_encodings: &[&str],
    embeddings: Vec<HandEmbeddings>,
) -> Result<RecordBatch> {
    let schema = table_schema();

    let ids: Vec<u64> = hands.iter().map(|h| h.id).collect();
    let hand_jsons: Vec<String> = hands
        .iter()
        .map(|h| serde_json::to_string(h).unwrap())
        .collect();
    let summary_texts: Vec<&str> = summaries.to_vec();
    let action_texts: Vec<&str> = action_encodings.to_vec();
    let sites: Vec<&str> = hands.iter().map(|_| "ACR").collect();
    let game_types: Vec<&str> = hands
        .iter()
        .map(|h| match &h.game_type {
            GameType::Cash { .. } => "cash",
            GameType::Tournament { .. } => "tournament",
        })
        .collect();
    let variants: Vec<&str> = hands.iter().map(|h| variant_str(&h.variant)).collect();
    let betting_limits: Vec<&str> = hands
        .iter()
        .map(|h| betting_limit_str(&h.betting_limit))
        .collect();
    let is_hi_los: Vec<bool> = hands.iter().map(|h| h.is_hi_lo).collect();
    let is_bomb_pots: Vec<bool> = hands.iter().map(|h| h.is_bomb_pot).collect();
    let stakes_vec: Vec<String> = hands.iter().map(|h| stakes_str(&h.game_type)).collect();
    let table_sizes: Vec<u8> = hands.iter().map(|h| h.table_size).collect();
    let heroes: Vec<String> = hands
        .iter()
        .map(|h| h.hero.clone().unwrap_or_default())
        .collect();
    let hero_positions: Vec<String> = hands
        .iter()
        .map(|h| h.hero_position.map(|p| p.to_string()).unwrap_or_default())
        .collect();
    let hero_cards_vec: Vec<String> = hands
        .iter()
        .map(|h| {
            h.hero_cards
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let hero_results: Vec<&str> = hands
        .iter()
        .map(|h| hero_result_str(&h.result))
        .collect();
    let boards: Vec<String> = hands
        .iter()
        .map(|h| {
            h.board
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect();
    let num_players: Vec<u64> = hands.iter().map(|h| h.players.len() as u64).collect();
    let showdowns: Vec<bool> = hands.iter().map(|h| went_to_showdown(h)).collect();
    let timestamps: Vec<&str> = hands.iter().map(|h| h.timestamp.as_str()).collect();
    let pot_types: Vec<&str> = hands.iter().map(|h| classify_pot_type(h)).collect();
    let opponent_names: Vec<String> = hands.iter().map(|h| opponent_names_str(h)).collect();
    let tournament_ids: Vec<u64> = hands
        .iter()
        .map(|h| match &h.game_type {
            GameType::Tournament { tournament_id, .. } => *tournament_id,
            _ => 0,
        })
        .collect();
    let pot_amounts: Vec<f64> = hands
        .iter()
        .map(|h| h.pot.map(|p| p.amount).unwrap_or(0.0))
        .collect();
    let tags: Vec<&str> = hands.iter().map(|_| "").collect();

    // Build embedding vectors
    let summary_vecs: Vec<Option<Vec<Option<f32>>>> = embeddings
        .iter()
        .map(|e| Some(e.summary.iter().map(|&v| Some(v)).collect()))
        .collect();
    let action_vecs: Vec<Option<Vec<Option<f32>>>> = embeddings
        .iter()
        .map(|e| Some(e.action.iter().map(|&v| Some(v)).collect()))
        .collect();

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(UInt64Array::from(ids)),
            Arc::new(StringArray::from(hand_jsons)),
            Arc::new(StringArray::from(summary_texts)),
            Arc::new(StringArray::from(action_texts)),
            Arc::new(StringArray::from(sites)),
            Arc::new(StringArray::from(game_types)),
            Arc::new(StringArray::from(variants)),
            Arc::new(StringArray::from(betting_limits)),
            Arc::new(BooleanArray::from(is_hi_los)),
            Arc::new(BooleanArray::from(is_bomb_pots)),
            Arc::new(StringArray::from(
                stakes_vec.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )),
            Arc::new(UInt8Array::from(table_sizes)),
            Arc::new(StringArray::from(
                heroes.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                hero_positions
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(
                hero_cards_vec
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(StringArray::from(hero_results)),
            Arc::new(StringArray::from(
                boards.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(num_players)),
            Arc::new(BooleanArray::from(showdowns)),
            Arc::new(StringArray::from(timestamps)),
            Arc::new(StringArray::from(pot_types)),
            Arc::new(StringArray::from(
                opponent_names
                    .iter()
                    .map(|s| s.as_str())
                    .collect::<Vec<_>>(),
            )),
            Arc::new(UInt64Array::from(tournament_ids)),
            Arc::new(Float64Array::from(pot_amounts)),
            Arc::new(StringArray::from(tags)),
            Arc::new(
                FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                    summary_vecs,
                    EMBEDDING_DIM,
                ),
            ),
            Arc::new(
                FixedSizeListArray::from_iter_primitive::<Float32Type, _, _>(
                    action_vecs,
                    EMBEDDING_DIM,
                ),
            ),
        ],
    )
    .context("Failed to build RecordBatch")?;

    Ok(batch)
}

impl VectorStore {
    pub async fn new(data_dir: &str, table_name: &str) -> Result<Self> {
        let db = lancedb::connect(data_dir)
            .execute()
            .await
            .context("Failed to connect to LanceDB")?;

        let names = db
            .table_names()
            .execute()
            .await
            .context("Failed to list tables")?;

        let table = if names.contains(&table_name.to_string()) {
            db.open_table(table_name)
                .execute()
                .await
                .context("Failed to open table")?
        } else {
            db.create_empty_table(table_name, table_schema())
                .execute()
                .await
                .context("Failed to create table")?
        };

        // Migrate: add tags column if missing (existing tables won't have it)
        let schema = table
            .schema()
            .await
            .context("Failed to get table schema")?;
        if schema.field_with_name("tags").is_err() {
            table
                .add_columns(
                    NewColumnTransform::SqlExpressions(vec![(
                        "tags".to_string(),
                        "''".to_string(),
                    )]),
                    None,
                )
                .await
                .context("Failed to add tags column")?;
        }

        Ok(Self {
            db,
            table_name: table_name.to_string(),
            table,
        })
    }

    fn table(&self) -> &LanceTable {
        &self.table
    }

    pub async fn upsert_hand(
        &self,
        hand: &Hand,
        summary: &str,
        action_encoding: &str,
        embeddings: HandEmbeddings,
    ) -> Result<()> {
        self.upsert_hands_batch(vec![(hand, summary, action_encoding, embeddings)])
            .await
    }

    pub async fn upsert_hands_batch(
        &self,
        items: Vec<(&Hand, &str, &str, HandEmbeddings)>,
    ) -> Result<()> {
        let table = self.table();

        let hands: Vec<&Hand> = items.iter().map(|(h, _, _, _)| *h).collect();
        let summaries: Vec<&str> = items.iter().map(|(_, s, _, _)| *s).collect();
        let action_encodings: Vec<&str> = items.iter().map(|(_, _, a, _)| *a).collect();
        let embeddings: Vec<HandEmbeddings> = items.into_iter().map(|(_, _, _, e)| e).collect();

        let batch = build_record_batch(&hands, &summaries, &action_encodings, embeddings)?;
        let schema = batch.schema();

        let reader = RecordBatchIterator::new(vec![Ok(batch)], schema);

        let mut merge = table.merge_insert(&["id"]);
        merge
            .when_matched_update_all(None)
            .when_not_matched_insert_all();
        merge
            .execute(Box::new(reader))
            .await
            .context("Failed to upsert hands")?;

        Ok(())
    }

    pub async fn search(
        &self,
        vector_name: &str,
        query_embedding: Vec<f32>,
        limit: u64,
        filter: Option<String>,
    ) -> Result<Vec<SearchResult>> {
        let table = self.table();

        let mut query = table
            .query()
            .nearest_to(query_embedding)?
            .column(vector_name)
            .limit(limit as usize)
            .select(Select::columns(&[
                "id",
                "summary_text",
                "hero_position",
                "hero_cards",
                "stakes",
                "hero_result",
                "pot_type",
            ]));

        if let Some(ref f) = filter {
            query = query.only_if(f);
        }

        let batches: Vec<RecordBatch> = query
            .execute()
            .await
            .context("Vector search failed")?
            .try_collect()
            .await
            .context("Failed to collect search results")?;

        let mut results = Vec::new();
        for batch in &batches {
            let ids = batch
                .column_by_name("id")
                .unwrap()
                .as_any()
                .downcast_ref::<UInt64Array>()
                .unwrap();
            let distances = batch
                .column_by_name("_distance")
                .unwrap()
                .as_any()
                .downcast_ref::<arrow_array::Float32Array>()
                .unwrap();
            let summaries = batch
                .column_by_name("summary_text")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let positions = batch
                .column_by_name("hero_position")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let cards = batch
                .column_by_name("hero_cards")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let stakes_col = batch
                .column_by_name("stakes")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let hero_results = batch
                .column_by_name("hero_result")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();
            let pot_types = batch
                .column_by_name("pot_type")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..batch.num_rows() {
                results.push(SearchResult {
                    hand_id: ids.value(i),
                    score: 1.0 - distances.value(i),
                    summary: summaries.value(i).to_string(),
                    hero_position: positions.value(i).to_string(),
                    hero_cards: cards.value(i).to_string(),
                    stakes: stakes_col.value(i).to_string(),
                    hero_result: hero_results.value(i).to_string(),
                    pot_type: pot_types.value(i).to_string(),
                });
            }
        }

        Ok(results)
    }

    pub async fn get_hand_vector(
        &self,
        hand_id: u64,
        vector_name: &str,
    ) -> Result<Option<Vec<f32>>> {
        let table = self.table();

        let batches: Vec<RecordBatch> = table
            .query()
            .only_if(format!("id = {}", hand_id))
            .select(Select::columns(&[vector_name]))
            .limit(1)
            .execute()
            .await
            .context("Failed to query hand vector")?
            .try_collect()
            .await
            .context("Failed to collect hand vector")?;

        if batches.is_empty() || batches[0].num_rows() == 0 {
            return Ok(None);
        }

        let fsl = batches[0]
            .column_by_name(vector_name)
            .unwrap()
            .as_any()
            .downcast_ref::<FixedSizeListArray>()
            .unwrap();

        let value_arr = fsl.value(0);
        let values = value_arr
            .as_any()
            .downcast_ref::<arrow_array::Float32Array>()
            .unwrap();

        let vec: Vec<f32> = (0..values.len()).map(|i| values.value(i)).collect();
        Ok(Some(vec))
    }

    pub async fn hand_exists(&self, hand_id: u64) -> Result<bool> {
        let table = self.table();

        let count = table
            .count_rows(Some(format!("id = {}", hand_id)))
            .await
            .context("Failed to check hand existence")?;

        Ok(count > 0)
    }

    pub async fn count(&self) -> Result<u64> {
        let table = self.table();

        let count = table
            .count_rows(None)
            .await
            .context("Failed to count rows")?;

        Ok(count as u64)
    }

    pub async fn count_filtered(&self, filter: Option<String>) -> Result<u64> {
        let table = self.table();
        let count = table
            .count_rows(filter)
            .await
            .context("Failed to count filtered rows")?;
        Ok(count as u64)
    }

    pub async fn get_hand(&self, hand_id: u64) -> Result<Option<Hand>> {
        let table = self.table();

        let batches: Vec<RecordBatch> = table
            .query()
            .only_if(format!("id = {}", hand_id))
            .select(Select::columns(&["hand_json"]))
            .limit(1)
            .execute()
            .await
            .context("Failed to query hand")?
            .try_collect()
            .await
            .context("Failed to collect hand")?;

        if batches.is_empty() || batches[0].num_rows() == 0 {
            return Ok(None);
        }

        let json_col = batches[0]
            .column_by_name("hand_json")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();

        let hand: Hand = serde_json::from_str(json_col.value(0))
            .context("Failed to deserialize hand JSON")?;

        Ok(Some(hand))
    }

    pub async fn get_tags(&self, hand_id: u64) -> Result<Option<String>> {
        let table = self.table();

        let batches: Vec<RecordBatch> = table
            .query()
            .only_if(format!("id = {}", hand_id))
            .select(Select::columns(&["tags"]))
            .limit(1)
            .execute()
            .await
            .context("Failed to query tags")?
            .try_collect()
            .await
            .context("Failed to collect tags")?;

        if batches.is_empty() || batches[0].num_rows() == 0 {
            return Ok(None);
        }

        let tags_col = batches[0]
            .column_by_name("tags")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();

        Ok(Some(tags_col.value(0).to_string()))
    }

    pub async fn update_tags(&self, hand_id: u64, tags: &str) -> Result<()> {
        let table = self.table();

        table
            .update()
            .only_if(format!("id = {}", hand_id))
            .column("tags", format!("'{}'", tags.replace('\'', "''")))
            .execute()
            .await
            .context("Failed to update tags")?;

        Ok(())
    }

    pub async fn scroll_hands(&self, filter: Option<String>) -> Result<Vec<Hand>> {
        let table = self.table();

        let mut query = table
            .query()
            .select(Select::columns(&["hand_json"]));

        if let Some(ref f) = filter {
            query = query.only_if(f);
        }

        let batches: Vec<RecordBatch> = query
            .execute()
            .await
            .context("Failed to scroll hands")?
            .try_collect()
            .await
            .context("Failed to collect hands")?;

        let mut hands = Vec::new();
        for batch in &batches {
            let json_col = batch
                .column_by_name("hand_json")
                .unwrap()
                .as_any()
                .downcast_ref::<StringArray>()
                .unwrap();

            for i in 0..batch.num_rows() {
                let hand: Hand = serde_json::from_str(json_col.value(i))
                    .context("Failed to deserialize hand JSON")?;
                hands.push(hand);
            }
        }

        Ok(hands)
    }
}

#[cfg(test)]
mod tests;

