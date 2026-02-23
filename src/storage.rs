use anyhow::Result;

use crate::types::*;

pub struct VectorStore {
    db: lancedb::Connection,
    table_name: String,
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

impl VectorStore {
    pub async fn new(data_dir: &str, table_name: &str) -> Result<Self> {
        let db = lancedb::connect(data_dir).execute().await?;
        Ok(Self {
            db,
            table_name: table_name.to_string(),
        })
    }

    /// Create table if it doesn't already exist.
    pub async fn ensure_table(&self) -> Result<()> {
        todo!()
    }

    /// Upsert a single hand with its summary, action encoding, and embeddings.
    pub async fn upsert_hand(
        &self,
        hand: &Hand,
        summary: &str,
        action_encoding: &str,
        embeddings: HandEmbeddings,
    ) -> Result<()> {
        todo!()
    }

    /// Upsert a batch of hands.
    pub async fn upsert_hands_batch(
        &self,
        items: Vec<(&Hand, &str, &str, HandEmbeddings)>,
    ) -> Result<()> {
        todo!()
    }

    /// Search over stored hands using a named vector.
    pub async fn search(
        &self,
        vector_name: &str,
        query_embedding: Vec<f32>,
        limit: u64,
        filter: Option<String>,
    ) -> Result<Vec<SearchResult>> {
        todo!()
    }

    /// Retrieve a specific named vector for a hand by its point ID.
    pub async fn get_hand_vector(
        &self,
        hand_id: u64,
        vector_name: &str,
    ) -> Result<Option<Vec<f32>>> {
        todo!()
    }

    /// Check if a hand already exists by ID.
    pub async fn hand_exists(&self, hand_id: u64) -> Result<bool> {
        todo!()
    }

    /// Count total rows in the table.
    pub async fn count(&self) -> Result<u64> {
        todo!()
    }

    /// Retrieve a single hand by its ID.
    pub async fn get_hand(&self, hand_id: u64) -> Result<Option<Hand>> {
        todo!()
    }

    /// Scroll through all hands matching a filter, deserializing from hand_json.
    pub async fn scroll_hands(&self, filter: Option<String>) -> Result<Vec<Hand>> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Stub bodies use todo!() — will be implemented in Phase 2
    async fn test_ensure_table_creates_new() {
        let dir = tempfile::tempdir().unwrap();
        let store = VectorStore::new(dir.path().to_str().unwrap(), "test")
            .await
            .unwrap();
        store.ensure_table().await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }
}
