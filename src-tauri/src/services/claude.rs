use std::sync::Arc;
use std::time::Duration;

use reqwest::header::HeaderValue;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

use super::deepl::network_error;
use super::explain::{Explainer, Prompt};
use super::sse::{SseEvent, SseParser};
use crate::error::AppError;
use crate::secrets::{SecretKind, SecretStore};

const BASE_URL: &str = "https://api.anthropic.com";
const MESSAGES_PATH: &str = "/v1/messages";
const API_VERSION: &str = "2023-06-01";
const API_KEY_HEADER: &str = "x-api-key";
const VERSION_HEADER: &str = "anthropic-version";
const MODELS_PATH: &str = "/v1/models";
/// Teto da resposta. A explicação pedida cabe folgadamente; o limite protege a cota.
pub const MAX_OUTPUT_TOKENS: u32 = 800;
/// Tempo máximo sem receber nenhum byte (o limite vale para cada leitura, não para o total).
pub const STREAM_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
/// Teto de uma explicação inteira, mesmo que os bytes continuem chegando.
const TOTAL_TIMEOUT: Duration = Duration::from_secs(90);
/// Código HTTP que a Anthropic usa para sobrecarga, repetido nos eventos de erro do stream.
const STATUS_OVERLOADED: u16 = 529;

pub struct ClaudeClient {
    http: Client,
    secrets: Arc<dyn SecretStore>,
    base_url_override: Option<String>,
}

#[derive(Serialize)]
struct MessagesRequest<'a> {
    model: &'static str,
    max_tokens: u32,
    stream: bool,
    system: &'a str,
    messages: [Message<'a>; 1],
}

#[derive(Serialize)]
struct Message<'a> {
    role: &'static str,
    content: &'a str,
}

#[derive(Deserialize)]
struct DeltaEvent {
    delta: Delta,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Delta {
    TextDelta {
        text: String,
    },
    #[serde(other)]
    Other,
}

/// Acrescentado ao texto quando o modelo bate no teto de tokens, para a resposta cortada não
/// parecer completa.
const TRUNCATED_MARK: &str = "…";

#[derive(Deserialize)]
struct MessageDeltaEvent {
    delta: MessageDelta,
}

#[derive(Deserialize)]
struct MessageDelta {
    stop_reason: Option<String>,
}

#[derive(Deserialize)]
struct ErrorEvent {
    error: ErrorBody,
}

#[derive(Deserialize)]
struct ErrorBody {
    #[serde(rename = "type")]
    kind: String,
}

/// O que um evento do stream significa para quem consome o texto.
enum Step {
    Text(String),
    Stop,
    Skip,
}

impl ClaudeClient {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Result<Self, AppError> {
        Self::build(secrets, None, STREAM_IDLE_TIMEOUT)
    }

    /// `base_url_override` existe para apontar o cliente a um servidor falso nos testes, por
    /// isso `build` é privado a este módulo: código de produção só chega aqui por `new`. Sem
    /// override, só HTTPS é aceito. Redirects nunca são seguidos: o texto do usuário não deve
    /// ser reenviado a outro destino.
    fn build(
        secrets: Arc<dyn SecretStore>,
        base_url_override: Option<String>,
        idle_timeout: Duration,
    ) -> Result<Self, AppError> {
        let mut builder = Client::builder()
            .read_timeout(idle_timeout)
            .timeout(TOTAL_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none());
        if base_url_override.is_none() {
            builder = builder.https_only(true);
        }
        let http = builder.build().map_err(network_error)?;
        Ok(Self {
            http,
            secrets,
            base_url_override,
        })
    }
}

impl ClaudeClient {
    /// Confere a chave listando os modelos: não gasta tokens.
    pub async fn check_key(&self) -> Result<(), AppError> {
        let key = self.secrets.require(SecretKind::Anthropic)?;
        let key_header = api_key_header(&key)?;
        let base_url = self.base_url_override.as_deref().unwrap_or(BASE_URL);

        let response = self
            .http
            .get(format!("{base_url}{MODELS_PATH}"))
            .header(API_KEY_HEADER, key_header)
            .header(VERSION_HEADER, API_VERSION)
            .send()
            .await
            .map_err(network_error)?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            Err(error_for_status(status))
        }
    }
}

impl Explainer for ClaudeClient {
    async fn explain<F>(&self, prompt: &Prompt, mut on_delta: F) -> Result<(), AppError>
    where
        F: FnMut(String) + Send,
    {
        let key = self.secrets.require(SecretKind::Anthropic)?;
        let key_header = api_key_header(&key)?;
        let base_url = self.base_url_override.as_deref().unwrap_or(BASE_URL);

        let mut response = self
            .http
            .post(format!("{base_url}{MESSAGES_PATH}"))
            .header(API_KEY_HEADER, key_header)
            .header(VERSION_HEADER, API_VERSION)
            .json(&MessagesRequest {
                model: prompt.model.id(),
                max_tokens: MAX_OUTPUT_TOKENS,
                stream: true,
                system: prompt.system,
                messages: [Message {
                    role: "user",
                    content: &prompt.user,
                }],
            })
            .send()
            .await
            .map_err(network_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(error_for_status(status));
        }

        let mut parser = SseParser::default();
        while let Some(chunk) = response.chunk().await.map_err(network_error)? {
            for event in parser.feed(&chunk)? {
                match interpret(&event, status.as_u16())? {
                    Step::Text(text) => on_delta(text),
                    Step::Stop => return Ok(()),
                    Step::Skip => {}
                }
            }
        }

        // A conexão fechou sem `message_stop`: a explicação está incompleta.
        Err(AppError::Network("stream encerrado antes do fim".into()))
    }
}

fn interpret(event: &SseEvent, status: u16) -> Result<Step, AppError> {
    match event.event.as_deref() {
        Some("content_block_delta") => {
            let payload: DeltaEvent =
                serde_json::from_str(&event.data).map_err(|_| AppError::Upstream(status))?;
            Ok(match payload.delta {
                Delta::TextDelta { text } => Step::Text(text),
                Delta::Other => Step::Skip,
            })
        }
        Some("message_delta") => {
            let truncated = serde_json::from_str::<MessageDeltaEvent>(&event.data)
                .is_ok_and(|payload| payload.delta.stop_reason.as_deref() == Some("max_tokens"));
            Ok(if truncated {
                Step::Text(TRUNCATED_MARK.to_owned())
            } else {
                Step::Skip
            })
        }
        Some("message_stop") => Ok(Step::Stop),
        Some("error") => Err(error_for_stream_event(&event.data, status)),
        _ => Ok(Step::Skip),
    }
}

fn error_for_stream_event(data: &str, status: u16) -> AppError {
    let kind = serde_json::from_str::<ErrorEvent>(data)
        .map(|event| event.error.kind)
        .unwrap_or_default();
    match kind.as_str() {
        "overloaded_error" => AppError::Upstream(STATUS_OVERLOADED),
        "rate_limit_error" => AppError::RateLimited,
        "authentication_error" | "permission_error" => {
            AppError::InvalidApiKey(SecretKind::Anthropic)
        }
        _ => AppError::Upstream(status),
    }
}

/// Monta o header marcando-o como sensível, para que não apareça em `Debug` nem em logs.
fn api_key_header(key: &str) -> Result<HeaderValue, AppError> {
    let mut value =
        HeaderValue::from_str(key).map_err(|_| AppError::InvalidApiKey(SecretKind::Anthropic))?;
    value.set_sensitive(true);
    Ok(value)
}

fn error_for_status(status: StatusCode) -> AppError {
    match status.as_u16() {
        401 | 403 => AppError::InvalidApiKey(SecretKind::Anthropic),
        429 => AppError::RateLimited,
        other => AppError::Upstream(other),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Value};
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::secrets::InMemoryStore;
    use crate::services::explain::build_prompt;
    use crate::settings::ExplainModel;

    const KEY: &str = "sk-ant-chave-de-teste";

    fn store_with(key: Option<&str>) -> Arc<dyn SecretStore> {
        let store = InMemoryStore::new();
        if let Some(key) = key {
            store.set(SecretKind::Anthropic, key).unwrap();
        }
        Arc::new(store)
    }

    fn client_with_timeout(
        server: &MockServer,
        key: Option<&str>,
        timeout: Duration,
    ) -> ClaudeClient {
        ClaudeClient::build(store_with(key), Some(server.uri()), timeout).unwrap()
    }

    fn client(server: &MockServer, key: Option<&str>) -> ClaudeClient {
        client_with_timeout(server, key, Duration::from_secs(2))
    }

    fn prompt() -> Prompt {
        build_prompt("hello", "olá").unwrap()
    }

    fn event(name: &str, data: &Value) -> String {
        format!("event: {name}\ndata: {data}\n\n")
    }

    fn text_delta(text: &str) -> String {
        event(
            "content_block_delta",
            &json!({
                "type": "content_block_delta",
                "index": 0,
                "delta": { "type": "text_delta", "text": text }
            }),
        )
    }

    fn stop() -> String {
        event("message_stop", &json!({ "type": "message_stop" }))
    }

    fn stream_response(body: String) -> ResponseTemplate {
        ResponseTemplate::new(200).set_body_raw(body, "text/event-stream")
    }

    async fn server_replying(response: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .respond_with(response)
            .mount(&server)
            .await;
        server
    }

    async fn request_count(server: &MockServer) -> usize {
        server.received_requests().await.unwrap().len()
    }

    async fn run(client: &ClaudeClient) -> (Vec<String>, Result<(), AppError>) {
        let mut deltas = Vec::new();
        let result = client.explain(&prompt(), |text| deltas.push(text)).await;
        (deltas, result)
    }

    #[tokio::test]
    async fn streams_the_text_deltas_in_order() {
        let body = [
            event("message_start", &json!({ "type": "message_start" })),
            event("ping", &json!({ "type": "ping" })),
            text_delta("Olá"),
            text_delta(", mundo"),
            event(
                "content_block_stop",
                &json!({ "type": "content_block_stop" }),
            ),
            stop(),
        ]
        .concat();
        let server = server_replying(stream_response(body)).await;

        let (deltas, result) = run(&client(&server, Some(KEY))).await;

        result.unwrap();
        assert_eq!(deltas, vec!["Olá", ", mundo"]);
    }

    #[tokio::test]
    async fn sends_an_authenticated_streaming_request_without_tools() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/messages"))
            .and(header("x-api-key", KEY))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(stream_response(stop()))
            .expect(1)
            .mount(&server)
            .await;

        run(&client(&server, Some(KEY))).await.1.unwrap();

        let requests = server.received_requests().await.unwrap();
        let body: Value = requests[0].body_json().unwrap();
        let prompt = prompt();
        assert_eq!(body["model"], ExplainModel::default().id());
        assert_eq!(body["max_tokens"], MAX_OUTPUT_TOKENS);
        assert_eq!(body["stream"], true);
        assert_eq!(body["system"], prompt.system);
        assert_eq!(
            body["messages"],
            json!([{ "role": "user", "content": prompt.user }])
        );
        assert!(body.get("tools").is_none());
    }

    #[tokio::test]
    async fn ignores_unknown_events_and_non_text_deltas() {
        let body = [
            event("future_event", &json!({ "anything": true })),
            event(
                "content_block_delta",
                &json!({ "delta": { "type": "thinking_delta", "thinking": "..." } }),
            ),
            text_delta("ok"),
            stop(),
        ]
        .concat();
        let server = server_replying(stream_response(body)).await;

        let (deltas, result) = run(&client(&server, Some(KEY))).await;

        result.unwrap();
        assert_eq!(deltas, vec!["ok"]);
    }

    #[tokio::test]
    async fn sends_the_model_chosen_in_the_prompt() {
        let server = server_replying(stream_response(stop())).await;
        let prompt = Prompt {
            model: ExplainModel::Sonnet,
            ..prompt()
        };

        client(&server, Some(KEY))
            .explain(&prompt, |_| {})
            .await
            .unwrap();

        let requests = server.received_requests().await.unwrap();
        let body: Value = requests[0].body_json().unwrap();
        assert_eq!(body["model"], "claude-sonnet-5-5");
    }

    async fn models_server(response: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v1/models"))
            .and(header("x-api-key", KEY))
            .and(header("anthropic-version", "2023-06-01"))
            .respond_with(response)
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn check_key_accepts_a_valid_key_without_sending_a_prompt() {
        let server =
            models_server(ResponseTemplate::new(200).set_body_json(json!({"data": []}))).await;

        client(&server, Some(KEY)).check_key().await.unwrap();

        let requests = server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        assert!(requests[0].body.is_empty());
    }

    #[tokio::test]
    async fn check_key_maps_failures_to_typed_errors() {
        let cases = [
            (401, "invalid_api_key"),
            (403, "invalid_api_key"),
            (429, "rate_limited"),
            (500, "upstream"),
        ];

        for (status, expected_code) in cases {
            let server = models_server(ResponseTemplate::new(status)).await;

            let error = client(&server, Some(KEY)).check_key().await.unwrap_err();

            assert_eq!(error.code(), expected_code, "status {status}");
        }
    }

    #[tokio::test]
    async fn check_key_without_a_key_makes_no_request() {
        let server = models_server(ResponseTemplate::new(200)).await;

        let error = client(&server, None).check_key().await.unwrap_err();

        assert_eq!(error.code(), "missing_secret");
        assert_eq!(request_count(&server).await, 0);
    }

    #[tokio::test]
    async fn check_key_does_not_leak_the_key_on_network_failure() {
        let client = ClaudeClient::build(
            store_with(Some(KEY)),
            Some("http://127.0.0.1:1".into()),
            Duration::from_secs(2),
        )
        .unwrap();

        let error = client.check_key().await.unwrap_err();

        assert_eq!(error.code(), "network");
        assert!(!format!("{error:?}").contains(KEY));
    }

    #[tokio::test]
    async fn marks_an_answer_cut_by_the_token_limit() {
        let cut = event(
            "message_delta",
            &json!({ "type": "message_delta", "delta": { "stop_reason": "max_tokens" } }),
        );
        let server = server_replying(stream_response(
            [text_delta("Meio da fr"), cut, stop()].concat(),
        ))
        .await;

        let (deltas, result) = run(&client(&server, Some(KEY))).await;

        result.unwrap();
        assert_eq!(deltas, vec!["Meio da fr", "…"]);
    }

    #[tokio::test]
    async fn does_not_mark_a_normal_end_of_turn() {
        let normal = event(
            "message_delta",
            &json!({ "type": "message_delta", "delta": { "stop_reason": "end_turn" } }),
        );
        let server = server_replying(stream_response(
            [text_delta("Pronto."), normal, stop()].concat(),
        ))
        .await;

        let (deltas, result) = run(&client(&server, Some(KEY))).await;

        result.unwrap();
        assert_eq!(deltas, vec!["Pronto."]);
    }

    #[tokio::test]
    async fn maps_http_failures_to_typed_errors() {
        let cases = [
            (401, "invalid_api_key"),
            (403, "invalid_api_key"),
            (429, "rate_limited"),
            (500, "upstream"),
            (529, "upstream"),
        ];

        for (status, expected_code) in cases {
            let server = server_replying(ResponseTemplate::new(status)).await;

            let (deltas, result) = run(&client(&server, Some(KEY))).await;

            assert_eq!(result.unwrap_err().code(), expected_code, "status {status}");
            assert!(deltas.is_empty());
        }
    }

    #[tokio::test]
    async fn maps_an_error_event_in_the_middle_of_the_stream() {
        let cases = [
            ("overloaded_error", "upstream"),
            ("rate_limit_error", "rate_limited"),
            ("authentication_error", "invalid_api_key"),
            ("api_error", "upstream"),
        ];

        for (kind, expected_code) in cases {
            let body = [
                text_delta("parcial"),
                event(
                    "error",
                    &json!({ "type": "error", "error": { "type": kind, "message": "x" } }),
                ),
            ]
            .concat();
            let server = server_replying(stream_response(body)).await;

            let (deltas, result) = run(&client(&server, Some(KEY))).await;

            assert_eq!(result.unwrap_err().code(), expected_code, "{kind}");
            assert_eq!(deltas, vec!["parcial"]);
        }
    }

    #[tokio::test]
    async fn fails_when_the_stream_ends_before_message_stop() {
        let server = server_replying(stream_response(text_delta("cortado"))).await;

        let (deltas, result) = run(&client(&server, Some(KEY))).await;

        assert_eq!(result.unwrap_err().code(), "network");
        assert_eq!(deltas, vec!["cortado"]);
    }

    #[tokio::test]
    async fn rejects_a_malformed_delta_as_an_upstream_error() {
        let body = "event: content_block_delta\ndata: isto não é json\n\n".to_owned();
        let server = server_replying(stream_response(body)).await;

        let (_, result) = run(&client(&server, Some(KEY))).await;

        assert_eq!(result.unwrap_err().code(), "upstream");
    }

    #[tokio::test]
    async fn does_not_follow_redirects() {
        let target = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(stream_response(stop()))
            .mount(&target)
            .await;
        let origin = server_replying(
            ResponseTemplate::new(307)
                .insert_header("location", format!("{}/v1/messages", target.uri()).as_str()),
        )
        .await;

        let (_, result) = run(&client(&origin, Some(KEY))).await;

        assert_eq!(result.unwrap_err().code(), "upstream");
        assert_eq!(request_count(&target).await, 0);
    }

    #[tokio::test]
    async fn times_out_on_a_stalled_response() {
        let server =
            server_replying(stream_response(stop()).set_delay(Duration::from_millis(600))).await;

        let (_, result) = run(&client_with_timeout(
            &server,
            Some(KEY),
            Duration::from_millis(100),
        ))
        .await;

        assert_eq!(result.unwrap_err().code(), "network");
    }

    #[tokio::test]
    async fn fails_without_a_key_and_makes_no_request() {
        let server = server_replying(stream_response(stop())).await;

        let (_, result) = run(&client(&server, None)).await;

        assert_eq!(result.unwrap_err().code(), "missing_secret");
        assert_eq!(request_count(&server).await, 0);
    }

    #[tokio::test]
    async fn rejects_a_key_that_cannot_be_sent_as_a_header_and_makes_no_request() {
        let server = server_replying(stream_response(stop())).await;

        let (_, result) = run(&client(&server, Some("abc\ndef"))).await;

        assert_eq!(result.unwrap_err().code(), "invalid_api_key");
        assert_eq!(request_count(&server).await, 0);
    }

    #[tokio::test]
    async fn reports_a_network_error_without_leaking_the_key() {
        let client = ClaudeClient::build(
            store_with(Some(KEY)),
            Some("http://127.0.0.1:1".into()),
            Duration::from_secs(2),
        )
        .unwrap();

        let (_, result) = run(&client).await;

        let error = result.unwrap_err();
        assert_eq!(error.code(), "network");
        assert!(!format!("{error:?}").contains(KEY));
        assert!(!error.to_string().contains(KEY));
    }
}
