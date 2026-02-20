use anyhow::{Context, Result};
use qdrant_client::qdrant::{
    CreateCollectionBuilder, Distance, Filter, GetPointsBuilder, PointStruct,
    SearchParamsBuilder, SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
    ScrollPointsBuilder, PointId, vectors_output::VectorsOptions,
};
use qdrant_client::qdrant::VectorsConfigBuilder;
use qdrant_client::Qdrant;
use serde_json::json;
use std::collections::HashMap;

use crate::stats;
use crate::types::*;

pub struct VectorStore {
    client: Qdrant,
    collection: String,
}

pub struct SearchResult {
    pub hand_id: u64,
    pub score: f32,
    pub summary: String,
    pub payload: HashMap<String, serde_json::Value>,
}

pub struct HandEmbeddings {
    pub summary: Vec<f32>,
    pub action: Vec<f32>,
}

impl VectorStore {
    pub async fn new(url: &str, collection: &str) -> Result<Self> {
        let client = Qdrant::from_url(url)
            .build()
            .context("Failed to connect to Qdrant")?;
        Ok(Self {
            client,
            collection: collection.to_string(),
        })
    }

    /// Create collection if it doesn't already exist.
    /// Uses named vectors ("summary" and "action"), both 384-dim cosine.
    pub async fn ensure_collection(&self) -> Result<()> {
        let exists = self
            .client
            .collection_exists(&self.collection)
            .await
            .context("Failed to check collection existence")?;

        if !exists {
            let mut vectors_config = VectorsConfigBuilder::default();
            vectors_config.add_named_vector_params(
                "summary",
                VectorParamsBuilder::new(384, Distance::Cosine),
            );
            vectors_config.add_named_vector_params(
                "action",
                VectorParamsBuilder::new(384, Distance::Cosine),
            );

            self.client
                .create_collection(
                    CreateCollectionBuilder::new(&self.collection)
                        .vectors_config(vectors_config),
                )
                .await
                .context("Failed to create collection")?;
        } else {
            // Check if the existing collection uses named vectors
            let info = self
                .client
                .collection_info(&self.collection)
                .await
                .context("Failed to get collection info")?;

            if let Some(result) = &info.result {
                if let Some(ref config) = result.config {
                    if let Some(ref vectors_config) = config.params {
                        if let Some(ref vc) = vectors_config.vectors_config {
                            use qdrant_client::qdrant::vectors_config::Config;
                            match &vc.config {
                                Some(Config::Params(_)) => {
                                    anyhow::bail!(
                                        "Collection '{}' uses the old single-vector schema. \
                                         Delete it and re-import:\n  \
                                         curl -X DELETE http://localhost:6333/collections/{}\n  \
                                         cargo run -- import",
                                        self.collection,
                                        self.collection
                                    );
                                }
                                Some(Config::ParamsMap(_)) => {
                                    // Named vectors — correct schema
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Upsert a single hand with its summary, action encoding, and embeddings.
    pub async fn upsert_hand(
        &self,
        hand: &Hand,
        summary: &str,
        action_encoding: &str,
        embeddings: HandEmbeddings,
    ) -> Result<()> {
        let point = build_point(hand, summary, action_encoding, embeddings);
        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, vec![point]).wait(true))
            .await
            .context("Failed to upsert hand")?;
        Ok(())
    }

    /// Upsert a batch of hands.
    pub async fn upsert_hands_batch(
        &self,
        items: Vec<(&Hand, &str, &str, HandEmbeddings)>,
    ) -> Result<()> {
        let points: Vec<PointStruct> = items
            .into_iter()
            .map(|(hand, summary, action_encoding, embeddings)| {
                build_point(hand, summary, action_encoding, embeddings)
            })
            .collect();

        self.client
            .upsert_points(UpsertPointsBuilder::new(&self.collection, points).wait(true))
            .await
            .context("Failed to upsert batch")?;
        Ok(())
    }

    /// Search over stored hands using a named vector.
    pub async fn search(
        &self,
        vector_name: &str,
        query_embedding: Vec<f32>,
        limit: u64,
        filter: Option<Filter>,
    ) -> Result<Vec<SearchResult>> {
        let mut builder = SearchPointsBuilder::new(&self.collection, query_embedding, limit)
            .vector_name(vector_name)
            .with_payload(true)
            .params(SearchParamsBuilder::default().exact(false));

        if let Some(f) = filter {
            builder = builder.filter(f);
        }

        let results = self
            .client
            .search_points(builder)
            .await
            .context("Search failed")?;

        Ok(results
            .result
            .into_iter()
            .map(|point| {
                let payload: HashMap<String, serde_json::Value> = point
                    .payload
                    .iter()
                    .map(|(k, v)| (k.clone(), qdrant_value_to_json(v)))
                    .collect();

                let hand_id = match point.id {
                    Some(PointId { point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Num(n)) }) => n,
                    _ => 0,
                };

                let summary = payload
                    .get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                SearchResult {
                    hand_id,
                    score: point.score,
                    summary,
                    payload,
                }
            })
            .collect())
    }

    /// Retrieve a specific named vector for a hand by its point ID.
    pub async fn get_hand_vector(
        &self,
        hand_id: u64,
        vector_name: &str,
    ) -> Result<Option<Vec<f32>>> {
        let point_id: PointId = hand_id.into();
        let result = self
            .client
            .get_points(
                GetPointsBuilder::new(&self.collection, vec![point_id])
                    .with_payload(false)
                    .with_vectors(true),
            )
            .await
            .context("Failed to get hand vector")?;

        if let Some(point) = result.result.into_iter().next() {
            if let Some(vectors) = point.vectors {
                match vectors.vectors_options {
                    Some(VectorsOptions::Vectors(named)) => {
                        if let Some(vector_output) = named.vectors.into_iter()
                            .find(|(k, _)| k == vector_name)
                            .map(|(_, v)| v)
                        {
                            // Use deprecated .data field — into_vector() returns
                            // vector::Vector which is a different type
                            #[allow(deprecated)]
                            let data = vector_output.data;
                            if !data.is_empty() {
                                return Ok(Some(data));
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        Ok(None)
    }

    /// Check if a hand already exists by ID.
    pub async fn hand_exists(&self, hand_id: u64) -> Result<bool> {
        let result = self
            .client
            .scroll(
                ScrollPointsBuilder::new(&self.collection)
                    .filter(Filter::must([qdrant_client::qdrant::Condition::matches(
                        "hand_id",
                        hand_id as i64,
                    )]))
                    .limit(1),
            )
            .await
            .context("Failed to check hand existence")?;
        Ok(!result.result.is_empty())
    }

    /// Count total points in the collection.
    pub async fn count(&self) -> Result<u64> {
        let info = self
            .client
            .collection_info(&self.collection)
            .await
            .context("Failed to get collection info")?;
        Ok(info.result.map(|r| r.points_count.unwrap_or(0)).unwrap_or(0))
    }

    /// Retrieve a single hand by its point ID.
    pub async fn get_hand(&self, hand_id: u64) -> Result<Option<Hand>> {
        let point_id: PointId = hand_id.into();
        let result = self
            .client
            .get_points(
                GetPointsBuilder::new(&self.collection, vec![point_id])
                    .with_payload(true),
            )
            .await
            .context("Failed to get hand by ID")?;

        if let Some(point) = result.result.first() {
            if let Some(val) = point.payload.get("hand_json") {
                let json_str = qdrant_value_to_json(val);
                if let Some(s) = json_str.as_str() {
                    let hand: Hand = serde_json::from_str(s)
                        .context("Failed to deserialize hand_json")?;
                    return Ok(Some(hand));
                }
            }
        }
        Ok(None)
    }

    /// Scroll through all hands matching a filter, deserializing from hand_json.
    pub async fn scroll_hands(&self, filter: Option<Filter>) -> Result<Vec<Hand>> {
        let mut hands = Vec::new();
        let mut offset: Option<PointId> = None;
        let page_size = 100u32;

        loop {
            let mut builder = ScrollPointsBuilder::new(&self.collection)
                .with_payload(true)
                .limit(page_size);

            if let Some(ref f) = filter {
                builder = builder.filter(f.clone());
            }
            if let Some(ref o) = offset {
                builder = builder.offset(o.clone());
            }

            let result = self
                .client
                .scroll(builder)
                .await
                .context("Failed to scroll hands")?;

            for point in &result.result {
                if let Some(val) = point.payload.get("hand_json") {
                    let json_str = qdrant_value_to_json(val);
                    if let Some(s) = json_str.as_str() {
                        if let Ok(hand) = serde_json::from_str::<Hand>(s) {
                            hands.push(hand);
                        }
                    }
                }
            }

            match result.next_page_offset {
                Some(next) => offset = Some(next),
                None => break,
            }
        }

        Ok(hands)
    }
}

fn build_point(
    hand: &Hand,
    summary: &str,
    action_encoding: &str,
    embeddings: HandEmbeddings,
) -> PointStruct {
    let hero_name = hand.hero.as_deref().unwrap_or("");
    let hero_pos = hand
        .hero_position
        .map(|p| p.to_string())
        .unwrap_or_default();
    let hero_cards: String = hand
        .hero_cards
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ");
    let board: String = hand
        .board
        .iter()
        .map(|c| c.to_string())
        .collect::<Vec<_>>()
        .join(" ");

    let hero_result = match &hand.result.hero_result {
        HeroResult::Won => "won",
        HeroResult::Lost => "lost",
        HeroResult::Folded => "folded",
        HeroResult::SatOut => "sat_out",
    };

    let went_to_showdown = hand
        .actions
        .iter()
        .any(|a| a.street == Street::Showdown);

    let (game_type_str, stakes, tournament_id) = match &hand.game_type {
        GameType::Cash {
            small_blind,
            big_blind,
            ..
        } => (
            "cash".to_string(),
            format!("{}/{}", small_blind, big_blind),
            None,
        ),
        GameType::Tournament {
            tournament_id,
            level,
            small_blind,
            big_blind,
            ..
        } => (
            "tournament".to_string(),
            format!("L{} {}/{}", level, small_blind, big_blind),
            Some(*tournament_id),
        ),
    };

    let pot_amount = hand.pot.map(|p| p.amount);
    let num_players = hand.players.iter().filter(|p| !p.is_sitting_out).count() as u64;

    let hand_json = serde_json::to_string(hand).unwrap_or_default();
    let pot_type = stats::classify_pot_type(hand);
    let opponent_names: Vec<String> = hand
        .players
        .iter()
        .filter(|p| !p.is_sitting_out && p.name != hero_name)
        .map(|p| p.name.clone())
        .collect();

    let variant_str = match hand.variant {
        PokerVariant::Holdem => "holdem",
        PokerVariant::Omaha => "omaha",
        PokerVariant::FiveCardOmaha => "five_card_omaha",
        PokerVariant::SevenCardStud => "seven_card_stud",
    };
    let betting_limit_str = match hand.betting_limit {
        BettingLimit::NoLimit => "no_limit",
        BettingLimit::PotLimit => "pot_limit",
        BettingLimit::FixedLimit => "fixed_limit",
    };

    let mut payload = json!({
        "hand_id": hand.id,
        "site": "ACR",
        "game_type": game_type_str,
        "variant": variant_str,
        "betting_limit": betting_limit_str,
        "is_hi_lo": hand.is_hi_lo,
        "is_bomb_pot": hand.is_bomb_pot,
        "stakes": stakes,
        "table_size": hand.table_size,
        "hero": hero_name,
        "hero_position": hero_pos,
        "hero_cards": hero_cards,
        "hero_result": hero_result,
        "board": board,
        "num_players": num_players,
        "went_to_showdown": went_to_showdown,
        "timestamp": hand.timestamp,
        "summary": summary,
        "action_encoding": action_encoding,
        "hand_json": hand_json,
        "pot_type": pot_type,
        "opponent_names": opponent_names,
    });

    if let Some(tid) = tournament_id {
        payload["tournament_id"] = json!(tid);
    }
    if let Some(pot) = pot_amount {
        payload["pot_amount"] = json!(pot);
    }

    // Named vectors
    let vectors: HashMap<String, Vec<f32>> = HashMap::from([
        ("summary".to_string(), embeddings.summary),
        ("action".to_string(), embeddings.action),
    ]);

    PointStruct::new(
        hand.id,
        vectors,
        payload
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| (k.clone(), v.clone().into()))
            .collect::<HashMap<String, qdrant_client::qdrant::Value>>(),
    )
}

fn qdrant_value_to_json(value: &qdrant_client::qdrant::Value) -> serde_json::Value {
    use qdrant_client::qdrant::value::Kind;
    match &value.kind {
        Some(Kind::StringValue(s)) => json!(s),
        Some(Kind::IntegerValue(i)) => json!(i),
        Some(Kind::DoubleValue(d)) => json!(d),
        Some(Kind::BoolValue(b)) => json!(b),
        Some(Kind::NullValue(_)) => serde_json::Value::Null,
        Some(Kind::ListValue(list)) => {
            let items: Vec<serde_json::Value> = list.values.iter().map(qdrant_value_to_json).collect();
            json!(items)
        }
        Some(Kind::StructValue(s)) => {
            let map: serde_json::Map<String, serde_json::Value> = s.fields.iter()
                .map(|(k, v)| (k.clone(), qdrant_value_to_json(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        _ => serde_json::Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires Qdrant running
    async fn test_ensure_collection_named_vectors() {
        let store = VectorStore::new("http://localhost:6334", "test_named_vectors")
            .await
            .unwrap();

        // Clean up if exists from previous run
        let _ = store.client.delete_collection("test_named_vectors").await;

        store.ensure_collection().await.unwrap();
        let count = store.count().await.unwrap();
        assert_eq!(count, 0);

        // Clean up
        let _ = store.client.delete_collection("test_named_vectors").await;
    }
}
