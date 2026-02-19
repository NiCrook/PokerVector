use anyhow::{Context, Result};
use ndarray::Array2;
use ort::session::Session;
use ort::value::Tensor;
use tokenizers::Tokenizer;

pub struct Embedder {
    session: Session,
    tokenizer: Tokenizer,
}

impl Embedder {
    /// Create a new embedder, downloading the model on first run via hf-hub.
    pub fn new() -> Result<Self> {
        let api = hf_hub::api::sync::Api::new().context("Failed to create HuggingFace API")?;
        let repo = api.model("sentence-transformers/all-MiniLM-L6-v2".to_string());

        let model_path = repo
            .get("onnx/model.onnx")
            .context("Failed to download ONNX model")?;
        let tokenizer_path = repo
            .get("tokenizer.json")
            .context("Failed to download tokenizer")?;

        let session = Session::builder()
            .context("Failed to create ONNX session builder")?
            .commit_from_file(model_path)
            .context("Failed to load ONNX model")?;

        let tokenizer = Tokenizer::from_file(tokenizer_path)
            .map_err(|e| anyhow::anyhow!("Failed to load tokenizer: {}", e))?;

        Ok(Self { session, tokenizer })
    }

    /// Embed a single text string into a 384-dimensional vector.
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>> {
        let batch = self.embed_batch(&[text])?;
        Ok(batch.into_iter().next().unwrap())
    }

    /// Embed a batch of texts into 384-dimensional vectors.
    pub fn embed_batch(&mut self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        // Tokenize all texts
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| anyhow::anyhow!("Tokenization failed: {}", e))?;

        let batch_size = encodings.len();
        let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(0);

        // Build padded input tensors as i64
        let mut input_ids = Array2::<i64>::zeros((batch_size, max_len));
        let mut attention_mask = Array2::<i64>::zeros((batch_size, max_len));

        for (i, encoding) in encodings.iter().enumerate() {
            let ids = encoding.get_ids();
            let mask = encoding.get_attention_mask();
            for (j, (&id, &m)) in ids.iter().zip(mask.iter()).enumerate() {
                input_ids[[i, j]] = id as i64;
                attention_mask[[i, j]] = m as i64;
            }
        }

        // token_type_ids: all zeros (single-sentence encoding)
        let token_type_ids = Array2::<i64>::zeros((batch_size, max_len));

        // Create ort Tensor values from ndarrays
        let input_ids_tensor = Tensor::from_array(input_ids)
            .context("Failed to create input_ids tensor")?;
        let attention_mask_tensor = Tensor::from_array(attention_mask)
            .context("Failed to create attention_mask tensor")?;
        let token_type_ids_tensor = Tensor::from_array(token_type_ids)
            .context("Failed to create token_type_ids tensor")?;

        // Run inference (session.run needs &self in ort 2.0)
        let inputs = ort::inputs! {
            "input_ids" => input_ids_tensor,
            "attention_mask" => attention_mask_tensor,
            "token_type_ids" => token_type_ids_tensor,
        };

        let outputs = self
            .session
            .run(inputs)
            .context("ONNX inference failed")?;

        // Extract token embeddings: shape [batch, seq_len, 384]
        let (shape, data) = outputs[0]
            .try_extract_tensor::<f32>()
            .context("Failed to extract embeddings tensor")?;

        let dim = shape[2] as usize; // 384
        let seq_len_total = shape[1] as usize;

        // Mean pooling + L2 normalize
        let mut results = Vec::with_capacity(batch_size);

        for i in 0..batch_size {
            let mask = encodings[i].get_attention_mask();

            let mut pooled = vec![0.0f32; dim];
            let mut count = 0.0f32;

            for (t, &m) in mask.iter().enumerate() {
                if m > 0 {
                    let m_f = m as f32;
                    let offset = (i * seq_len_total + t) * dim;
                    for d in 0..dim {
                        pooled[d] += data[offset + d] * m_f;
                    }
                    count += m_f;
                }
            }

            if count > 0.0 {
                for d in 0..dim {
                    pooled[d] /= count;
                }
            }

            // L2 normalize
            let norm: f32 = pooled.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 {
                for d in 0..dim {
                    pooled[d] /= norm;
                }
            }

            results.push(pooled);
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore] // Requires model download
    fn test_embed_produces_384_dim_vector() {
        let mut embedder = Embedder::new().unwrap();
        let vec = embedder.embed("Hero raises to $0.04 from the button").unwrap();
        assert_eq!(vec.len(), 384);

        // Check L2 normalized (magnitude ~1.0)
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-5, "Expected unit vector, got norm {}", norm);
    }

    #[test]
    #[ignore] // Requires model download
    fn test_embed_batch() {
        let mut embedder = Embedder::new().unwrap();
        let vecs = embedder
            .embed_batch(&["Hero raises", "Hero folds", "Hero checks"])
            .unwrap();
        assert_eq!(vecs.len(), 3);
        for v in &vecs {
            assert_eq!(v.len(), 384);
        }
    }

    #[test]
    #[ignore] // Requires model download
    fn test_similar_texts_have_higher_cosine_similarity() {
        let mut embedder = Embedder::new().unwrap();
        let v1 = embedder.embed("Hero raises to $0.04 from the button").unwrap();
        let v2 = embedder.embed("Player raises from BTN position").unwrap();
        let v3 = embedder.embed("The weather is sunny today").unwrap();

        let sim_related: f32 = v1.iter().zip(v2.iter()).map(|(a, b)| a * b).sum();
        let sim_unrelated: f32 = v1.iter().zip(v3.iter()).map(|(a, b)| a * b).sum();

        assert!(
            sim_related > sim_unrelated,
            "Related texts should be more similar: {} vs {}",
            sim_related,
            sim_unrelated
        );
    }
}
