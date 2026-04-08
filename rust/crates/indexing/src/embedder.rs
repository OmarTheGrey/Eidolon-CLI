use candle_core::{Device, Tensor};
use candle_transformers::models::bert::BertModel;
use tokenizers::{PaddingDirection, PaddingParams, PaddingStrategy, Tokenizer, TruncationParams};

/// Wraps a loaded BERT model and tokenizer for batch embedding inference.
pub struct Embedder {
    model: BertModel,
    tokenizer: Tokenizer,
}

impl Embedder {
    pub fn new(model: BertModel, mut tokenizer: Tokenizer) -> Self {
        // Configure tokenizer for batch encoding with padding + truncation.
        let _ = tokenizer.with_truncation(Some(TruncationParams {
            max_length: 512,
            ..Default::default()
        }));
        tokenizer.with_padding(Some(PaddingParams {
            strategy: PaddingStrategy::BatchLongest,
            direction: PaddingDirection::Right,
            pad_id: 0,
            pad_token: "[PAD]".into(),
            ..Default::default()
        }));
        Self { model, tokenizer }
    }

    /// Embed a batch of texts, returning one 384-dim L2-normalised vector per
    /// input.
    pub fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }

        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| EmbedError(format!("tokenize: {e}")))?;

        let device = Device::Cpu;
        let len = encodings.len();
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);

        // Build input tensors.
        let mut all_ids = Vec::with_capacity(len * max_len);
        let mut all_mask = Vec::with_capacity(len * max_len);
        let mut all_type = Vec::with_capacity(len * max_len);

        for enc in &encodings {
            let ids = enc.get_ids();
            let mask = enc.get_attention_mask();
            let tids = enc.get_type_ids();

            all_ids.extend_from_slice(ids);
            all_mask.extend_from_slice(mask);
            all_type.extend_from_slice(tids);

            // Pad to max_len (in case the tokenizer didn't).
            let pad = max_len.saturating_sub(ids.len());
            all_ids.extend(std::iter::repeat_n(0u32, pad));
            all_mask.extend(std::iter::repeat_n(0u32, pad));
            all_type.extend(std::iter::repeat_n(0u32, pad));
        }

        let shape = (len, max_len);
        let input_ids =
            Tensor::from_vec(all_ids, shape, &device).map_err(|e| EmbedError(e.to_string()))?;
        let attention_mask =
            Tensor::from_vec(all_mask, shape, &device).map_err(|e| EmbedError(e.to_string()))?;
        let token_type_ids =
            Tensor::from_vec(all_type, shape, &device).map_err(|e| EmbedError(e.to_string()))?;

        // Forward pass → (batch, seq_len, hidden_size).
        let embeddings = self
            .model
            .forward(&input_ids, &token_type_ids, Some(&attention_mask))
            .map_err(|e| EmbedError(format!("forward: {e}")))?;

        // Mean pooling (mask-aware).
        let mask_f = attention_mask
            .unsqueeze(2)
            .and_then(|t| t.to_dtype(candle_core::DType::F32))
            .map_err(|e| EmbedError(e.to_string()))?;

        let masked = embeddings
            .broadcast_mul(&mask_f)
            .map_err(|e| EmbedError(e.to_string()))?;

        let summed = masked.sum(1).map_err(|e| EmbedError(e.to_string()))?;
        let counts = mask_f.sum(1).map_err(|e| EmbedError(e.to_string()))?;
        let pooled = summed
            .broadcast_div(&counts)
            .map_err(|e| EmbedError(e.to_string()))?;

        // L2-normalise each vector.
        let norms = pooled
            .sqr()
            .and_then(|t| t.sum(1))
            .and_then(|t| t.sqrt())
            .and_then(|t| t.unsqueeze(1))
            .map_err(|e| EmbedError(e.to_string()))?;
        let normed = pooled
            .broadcast_div(&norms)
            .map_err(|e| EmbedError(e.to_string()))?;

        // Extract as Vec<Vec<f32>>.
        let flat: Vec<f32> = normed
            .flatten_all()
            .and_then(|t| t.to_vec1())
            .map_err(|e| EmbedError(e.to_string()))?;

        let dim = flat.len() / len;
        let vectors: Vec<Vec<f32>> = flat.chunks(dim).map(<[f32]>::to_vec).collect();

        Ok(vectors)
    }

    /// Embed a single text string.
    pub fn embed_one(&self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let mut vecs = self.embed_batch(&[text])?;
        vecs.pop().ok_or_else(|| EmbedError("empty result".into()))
    }
}

#[derive(Debug)]
pub struct EmbedError(pub String);

impl std::fmt::Display for EmbedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "embedding error: {}", self.0)
    }
}

impl std::error::Error for EmbedError {}
