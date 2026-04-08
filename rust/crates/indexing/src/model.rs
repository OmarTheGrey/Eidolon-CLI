use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config as BertConfig};
use std::path::Path;
use tokenizers::Tokenizer;

/// Download (or locate cached) model files from Hugging Face Hub.
///
/// Returns the local directory containing `config.json`, `tokenizer.json`,
/// and `model.safetensors`.
pub fn ensure_model(model_id: &str) -> Result<std::path::PathBuf, ModelError> {
    let api = hf_hub::api::sync::Api::new().map_err(|e| ModelError::Download(e.to_string()))?;
    let repo = api.model(model_id.to_string());

    // Trigger download / cache-hit for the three required files.
    let config_path = repo
        .get("config.json")
        .map_err(|e| ModelError::Download(e.to_string()))?;
    let _tokenizer_path = repo
        .get("tokenizer.json")
        .map_err(|e| ModelError::Download(e.to_string()))?;
    let _weights_path = repo
        .get("model.safetensors")
        .map_err(|e| ModelError::Download(e.to_string()))?;

    // All files land in the same directory.
    let model_dir = config_path
        .parent()
        .ok_or_else(|| ModelError::Download("unexpected hub layout".into()))?
        .to_path_buf();

    Ok(model_dir)
}

/// Load a BERT model and its tokenizer from a local directory.
pub fn load_model(model_dir: &Path) -> Result<(BertModel, Tokenizer), ModelError> {
    let device = Device::Cpu;

    // --- tokenizer ---
    let tokenizer_path = model_dir.join("tokenizer.json");
    let tokenizer = Tokenizer::from_file(&tokenizer_path)
        .map_err(|e| ModelError::Load(format!("tokenizer: {e}")))?;

    // --- config ---
    let config_path = model_dir.join("config.json");
    let config_text =
        std::fs::read_to_string(&config_path).map_err(|e| ModelError::Load(e.to_string()))?;
    let config: BertConfig =
        serde_json::from_str(&config_text).map_err(|e| ModelError::Load(e.to_string()))?;

    // --- weights ---
    let weights_path = model_dir.join("model.safetensors");
    let weights_bytes =
        std::fs::read(&weights_path).map_err(|e| ModelError::Load(e.to_string()))?;
    let vb = VarBuilder::from_buffered_safetensors(weights_bytes, DType::F32, &device)
        .map_err(|e| ModelError::Load(format!("weights: {e}")))?;
    let model =
        BertModel::load(vb, &config).map_err(|e| ModelError::Load(format!("model: {e}")))?;

    Ok((model, tokenizer))
}

/// Errors that can occur during model management.
#[derive(Debug)]
pub enum ModelError {
    Download(String),
    Load(String),
}

impl std::fmt::Display for ModelError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Download(e) => write!(f, "model download failed: {e}"),
            Self::Load(e) => write!(f, "model load failed: {e}"),
        }
    }
}

impl std::error::Error for ModelError {}

/// Helper: build a `Tensor` of token-type IDs (all zeros) matching `input_ids`.
pub fn zeros_like(t: &Tensor) -> Result<Tensor, candle_core::Error> {
    Tensor::zeros(t.shape(), t.dtype(), t.device())
}
