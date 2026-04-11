//! OpenAI-compatible API server for Eidolon.
//!
//! Exposes `/v1/chat/completions` (POST), `/v1/models` (GET), and `/health`
//! (GET) endpoints that make Eidolon usable as a backend for any
//! OpenAI-compatible frontend.

mod types;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, Sse};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use tokio_stream::StreamExt;

use api::{resolve_model_alias, AuthSource, ProviderClient};
use runtime::{
    ApiClient as RuntimeApiClient, ApiRequest, AssistantEvent, ConversationRuntime, PermissionMode,
    PermissionPolicy, RuntimeError, Session, StaticToolExecutor,
};

pub use types::*;

const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Shared state for the API server.
#[derive(Clone)]
pub struct AppState {
    /// The Anthropic auth source for creating provider clients.
    auth_source: AuthSource,
    /// Default model to use when none is specified.
    default_model: String,
    /// Optional bearer token for authenticating API requests.
    api_token: Option<String>,
    /// Persistent sessions keyed by session ID (via X-Eidolon-Session-Id header).
    sessions: Arc<Mutex<BTreeMap<String, Session>>>,
}

impl AppState {
    #[must_use]
    pub fn new(auth_source: AuthSource, default_model: String) -> Self {
        Self {
            auth_source,
            default_model,
            api_token: None,
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    #[must_use]
    pub fn with_api_token(mut self, token: String) -> Self {
        self.api_token = Some(token);
        self
    }
}

/// Build the axum router with all endpoints.
pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/v1/chat/completions", post(chat_completions))
        .route("/v1/models", get(list_models))
        .route("/health", get(health))
        .with_state(state)
}

/// Start the server on the given address.
pub async fn serve(state: AppState, bind_addr: &str) -> Result<(), Box<dyn std::error::Error>> {
    let app = build_router(state);
    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to listen for ctrl-c");
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Check bearer token if configured.
fn check_auth(state: &AppState, headers: &HeaderMap) -> Result<(), StatusCode> {
    let Some(expected) = &state.api_token else {
        return Ok(());
    };
    let auth = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if auth.strip_prefix("Bearer ").unwrap_or_default() == expected.as_str() {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

/// Extract or create a session for this request.
fn resolve_session(state: &AppState, headers: &HeaderMap) -> (Session, Option<String>) {
    let session_id = headers
        .get("x-eidolon-session-id")
        .and_then(|v| v.to_str().ok())
        .map(ToString::to_string);

    if let Some(ref id) = session_id {
        let mut sessions = state
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let session = sessions.entry(id.clone()).or_default();
        (session.clone(), Some(id.clone()))
    } else {
        (Session::new(), None)
    }
}

/// Save session back if it was persistent.
fn save_session(state: &AppState, session_id: Option<&str>, session: Session) {
    if let Some(id) = session_id {
        let mut sessions = state
            .sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        sessions.insert(id.to_string(), session);
    }
}

/// Bridge that adapts an `api::ProviderClient` into a `runtime::ApiClient`.
///
/// Spawns a dedicated tokio runtime on a background thread so the async
/// provider methods can be called from the sync [`runtime::ToolExecutor`]
/// contract.
struct ProviderBridge {
    rt: tokio::runtime::Runtime,
    client: ProviderClient,
    model: String,
}

impl RuntimeApiClient for ProviderBridge {
    fn stream(&mut self, request: ApiRequest) -> Result<Vec<AssistantEvent>, RuntimeError> {
        let message_request = api::MessageRequest {
            model: self.model.clone(),
            max_tokens: api::max_tokens_for_model(&self.model),
            messages: convert_chat_to_api_messages(&request.messages),
            system: (!request.system_prompt.is_empty())
                .then(|| request.system_prompt.join("\n\n")),
            tools: None,
            tool_choice: None,
            stream: true,
        };

        self.rt.block_on(async {
            let mut stream = self
                .client
                .stream_message(&message_request)
                .await
                .map_err(|e| RuntimeError::new(e.to_string()))?;

            let mut events = Vec::new();
            loop {
                match stream.next_event().await {
                    Ok(Some(api::StreamEvent::ContentBlockDelta(delta))) => {
                        if let api::ContentBlockDelta::TextDelta { text } = delta.delta {
                            events.push(AssistantEvent::TextDelta(text));
                        }
                    }
                    Ok(Some(api::StreamEvent::MessageStop(_)) | None) => {
                        events.push(AssistantEvent::MessageStop);
                        break;
                    }
                    Ok(Some(_)) => {}
                    Err(e) => return Err(RuntimeError::new(e.to_string())),
                }
            }
            Ok(events)
        })
    }
}

/// Convert runtime `ConversationMessages` to API `InputMessages`.
fn convert_chat_to_api_messages(
    messages: &[runtime::ConversationMessage],
) -> Vec<api::InputMessage> {
    messages
        .iter()
        .map(|msg| {
            let role = match msg.role {
                runtime::MessageRole::User | runtime::MessageRole::Tool => "user",
                runtime::MessageRole::Assistant => "assistant",
                runtime::MessageRole::System => "system",
            };
            let content: Vec<api::InputContentBlock> = msg
                .blocks
                .iter()
                .filter_map(|block| match block {
                    runtime::ContentBlock::Text { text } => {
                        Some(api::InputContentBlock::Text { text: text.clone() })
                    }
                    _ => None,
                })
                .collect();
            api::InputMessage {
                role: role.to_string(),
                content,
            }
        })
        .collect()
}

// ─── Handlers ───────────────────────────────────────────────────────────────

#[allow(clippy::too_many_lines)]
async fn chat_completions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ChatCompletionRequest>,
) -> Response {
    if let Err(status) = check_auth(&state, &headers) {
        return status.into_response();
    }

    let model = if request.model == "eidolon" || request.model.is_empty() {
        state.default_model.clone()
    } else {
        resolve_model_alias(&request.model)
    };

    let (session, session_id) = resolve_session(&state, &headers);

    // Extract inputs before moving into the blocking closure.
    let user_text = request
        .messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .map(|m| m.content.as_text())
        .unwrap_or_default();

    let system_messages: Vec<String> = request
        .messages
        .iter()
        .filter(|m| m.role == "system")
        .map(|m| m.content.as_text())
        .collect();

    let auth = state.auth_source.clone();
    let is_stream = request.stream;
    let model_for_response = model.clone();

    // Run the entire LLM turn on a blocking thread since ProviderClient
    // contains !Send types (Cell/RefCell for prompt cache).
    let result = tokio::task::spawn_blocking(move || -> Result<(String, runtime::TokenUsage, Session), String> {
        let client = ProviderClient::from_model_with_anthropic_auth(&model, Some(auth))
            .map_err(|e| e.to_string())?;
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        let bridge = ProviderBridge { rt, client, model: model.clone() };

        let mut runtime = ConversationRuntime::new(
            session,
            bridge,
            StaticToolExecutor::new(),
            PermissionPolicy::new(PermissionMode::ReadOnly),
            system_messages,
        );

        let summary = runtime.run_turn(&user_text, None).map_err(|e| e.to_string())?;

        let text: String = summary
            .assistant_messages
            .iter()
            .flat_map(|msg| &msg.blocks)
            .filter_map(|block| match block {
                runtime::ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        Ok((text, summary.usage, runtime.into_session()))
    })
    .await;

    let (assistant_text, usage, finished_session) = match result {
        Ok(Ok(tuple)) => tuple,
        Ok(Err(e)) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
        }
        Err(e) => {
            return (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response();
        }
    };

    save_session(&state, session_id.as_deref(), finished_session);

    let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());

    if is_stream {
        // Split completed text into word-level chunks for SSE delivery.
        let created = now_epoch();
        let model_owned = model_for_response.clone();
        let words: Vec<String> = assistant_text
            .split_inclusive(char::is_whitespace)
            .map(ToString::to_string)
            .collect();

        let stream = tokio_stream::iter(words.into_iter().enumerate().map(
            move |(i, text)| -> Result<Event, std::convert::Infallible> {
                let chunk = ChatCompletionChunk {
                    id: response_id.clone(),
                    object: "chat.completion.chunk",
                    created,
                    model: model_owned.clone(),
                    choices: vec![ChatChunkChoice {
                        index: 0,
                        delta: ChatChunkDelta {
                            role: (i == 0).then(|| "assistant".to_string()),
                            content: Some(text),
                        },
                        finish_reason: None,
                    }],
                };
                Ok(Event::default()
                    .data(serde_json::to_string(&chunk).unwrap_or_default()))
            },
        ))
        .chain(tokio_stream::once(Ok::<_, std::convert::Infallible>(
            Event::default().data("[DONE]"),
        )));

        return Sse::new(stream).into_response();
    }

    Json(ChatCompletionResponse {
        id: response_id,
        object: "chat.completion",
        created: now_epoch(),
        model: model_for_response,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text(assistant_text),
            },
            finish_reason: "stop".to_string(),
        }],
        usage: ChatUsage {
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
            total_tokens: usage.total_tokens(),
        },
    })
    .into_response()
}

async fn list_models(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    if let Err(status) = check_auth(&state, &headers) {
        return status.into_response();
    }

    let models = vec![
        ("claude-opus-4-6", "anthropic"),
        ("claude-sonnet-4-6", "anthropic"),
        ("claude-haiku-4-5-20251213", "anthropic"),
        ("grok-3", "xai"),
        ("grok-3-mini", "xai"),
        ("eidolon", "eidolon"),
    ];

    Json(ModelListResponse {
        object: "list",
        data: models
            .into_iter()
            .map(|(id, owner)| ModelEntry {
                id: id.to_string(),
                object: "model",
                created: 0,
                owned_by: owner.to_string(),
            })
            .collect(),
    })
    .into_response()
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: SERVER_VERSION,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::util::ServiceExt;

    fn test_state() -> AppState {
        AppState::new(
            AuthSource::ApiKey("test-key".to_string()),
            "claude-sonnet-4-6".to_string(),
        )
    }

    #[tokio::test]
    async fn health_returns_ok() {
        let app = build_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
    }

    #[tokio::test]
    async fn models_returns_list() {
        let app = build_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["object"], "list");
        assert!(json["data"].as_array().unwrap().len() >= 5);
    }

    #[tokio::test]
    async fn auth_rejects_wrong_token() {
        let state = test_state().with_api_token("secret123".to_string());
        let app = build_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .header("authorization", "Bearer wrong")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn auth_allows_correct_token() {
        let state = test_state().with_api_token("secret123".to_string());
        let app = build_router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/models")
                    .header("authorization", "Bearer secret123")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn completions_rejects_without_messages() {
        let app = build_router(test_state());
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/chat/completions")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"model":"eidolon"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        // Should fail with 422 (missing messages field).
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
