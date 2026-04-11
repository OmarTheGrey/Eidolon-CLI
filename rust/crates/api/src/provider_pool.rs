//! Provider pool — fallback chains and credential rotation for LLM providers.
//!
//! A [`ProviderPool`] wraps one or more [`ProviderClient`] instances and tries
//! each in order on fallback-eligible errors (overload, rate limit, auth).
//! After a successful fallback, the primary provider is automatically restored
//! on the next call.

use std::sync::atomic::{AtomicUsize, Ordering};

use crate::client::{MessageStream, ProviderClient};
use crate::error::ApiError;
use crate::types::MessageRequest;

/// A pool of [`ProviderClient`] instances with ordered fallback.
///
/// The pool always tries the primary (index 0) first. On a fallback-eligible
/// error, it walks the remaining providers in order until one succeeds or all
/// fail. After a successful fallback, the pool remembers the working index for
/// the current request, but automatically resets to the primary for the next
/// call (primary restoration).
#[derive(Debug)]
pub struct ProviderPool {
    providers: Vec<ProviderClient>,
    /// Tracks how many fallback events have occurred (observable for metrics).
    fallback_count: AtomicUsize,
}

impl ProviderPool {
    /// Create a pool with a primary provider and zero or more fallbacks.
    /// The primary is always at index 0.
    #[must_use]
    pub fn new(primary: ProviderClient, fallbacks: Vec<ProviderClient>) -> Self {
        let mut providers = Vec::with_capacity(1 + fallbacks.len());
        providers.push(primary);
        providers.extend(fallbacks);
        Self {
            providers,
            fallback_count: AtomicUsize::new(0),
        }
    }

    /// Create a pool with only a primary provider (no fallbacks).
    #[must_use]
    pub fn single(primary: ProviderClient) -> Self {
        Self::new(primary, Vec::new())
    }

    /// Number of providers in the pool (primary + fallbacks).
    #[must_use]
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Whether the pool has no providers (should never happen in practice).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Total number of fallback events that have occurred.
    #[must_use]
    pub fn fallback_count(&self) -> usize {
        self.fallback_count.load(Ordering::Relaxed)
    }

    /// Send a message, trying fallback providers on eligible errors.
    pub async fn send_message(&self, request: &MessageRequest) -> Result<crate::types::MessageResponse, ApiError> {
        let mut last_error = None;
        for (i, provider) in self.providers.iter().enumerate() {
            match provider.send_message(request).await {
                Ok(response) => {
                    if i > 0 {
                        self.fallback_count.fetch_add(1, Ordering::Relaxed);
                    }
                    return Ok(response);
                }
                Err(error) => {
                    if !error.is_fallback_eligible() || i + 1 == self.providers.len() {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            ApiError::Auth("provider pool is empty".to_string())
        }))
    }

    /// Stream a message, trying fallback providers on eligible errors.
    pub async fn stream_message(&self, request: &MessageRequest) -> Result<MessageStream, ApiError> {
        let mut last_error = None;
        for (i, provider) in self.providers.iter().enumerate() {
            match provider.stream_message(request).await {
                Ok(stream) => {
                    if i > 0 {
                        self.fallback_count.fetch_add(1, Ordering::Relaxed);
                    }
                    return Ok(stream);
                }
                Err(error) => {
                    if !error.is_fallback_eligible() || i + 1 == self.providers.len() {
                        return Err(error);
                    }
                    last_error = Some(error);
                }
            }
        }
        Err(last_error.unwrap_or_else(|| {
            ApiError::Auth("provider pool is empty".to_string())
        }))
    }

    /// Reference to the primary provider (for prompt cache stats etc.).
    #[must_use]
    pub fn primary(&self) -> &ProviderClient {
        &self.providers[0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_client() -> ProviderClient {
        ProviderClient::Xai(crate::providers::openai_compat::OpenAiCompatClient::new(
            "test-key",
            crate::providers::openai_compat::OpenAiCompatConfig::xai(),
        ))
    }

    #[test]
    fn single_provider_pool_has_len_one() {
        let pool = ProviderPool::single(make_test_client());
        assert_eq!(pool.len(), 1);
        assert_eq!(pool.fallback_count(), 0);
    }

    #[test]
    fn pool_with_fallbacks_has_correct_len() {
        let pool = ProviderPool::new(
            make_test_client(),
            vec![make_test_client(), make_test_client()],
        );
        assert_eq!(pool.len(), 3);
    }

    #[test]
    fn is_fallback_eligible_covers_expected_errors() {
        let overloaded = ApiError::Api {
            status: reqwest::StatusCode::from_u16(529).unwrap(),
            error_type: None,
            message: None,
            request_id: None,
            body: String::new(),
            retryable: false,
        };
        assert!(overloaded.is_fallback_eligible());

        let rate_limited = ApiError::Api {
            status: reqwest::StatusCode::TOO_MANY_REQUESTS,
            error_type: None,
            message: None,
            request_id: None,
            body: String::new(),
            retryable: true,
        };
        assert!(rate_limited.is_fallback_eligible());

        let context_window = ApiError::ContextWindowExceeded {
            model: "test".to_string(),
            estimated_input_tokens: 0,
            requested_output_tokens: 0,
            estimated_total_tokens: 0,
            context_window_tokens: 0,
        };
        assert!(!context_window.is_fallback_eligible());
    }
}
