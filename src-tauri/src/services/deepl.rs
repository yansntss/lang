use std::sync::Arc;
use std::time::Duration;

use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

use super::translation::{RawTranslation, TargetLang, Translator};
use crate::error::AppError;
use crate::secrets::{SecretKind, SecretStore};

const FREE_BASE_URL: &str = "https://api-free.deepl.com";
const PRO_BASE_URL: &str = "https://api.deepl.com";
const FREE_KEY_SUFFIX: &str = ":fx";
const TRANSLATE_PATH: &str = "/v2/translate";
const USAGE_PATH: &str = "/v2/usage";
/// Status HTTP próprio do DeepL para cota esgotada.
const STATUS_QUOTA_EXCEEDED: u16 = 456;
pub const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Chaves do plano Free terminam em `:fx` e usam outro host que as do plano Pro.
pub fn base_url_for_key(key: &str) -> &'static str {
    if key.ends_with(FREE_KEY_SUFFIX) {
        FREE_BASE_URL
    } else {
        PRO_BASE_URL
    }
}

pub struct DeepLClient {
    http: Client,
    secrets: Arc<dyn SecretStore>,
    base_url_override: Option<String>,
}

#[derive(Serialize)]
struct TranslateRequest<'a> {
    text: [&'a str; 1],
    target_lang: &'static str,
}

#[derive(Deserialize)]
struct TranslateResponse {
    translations: Vec<TranslatedText>,
}

#[derive(Deserialize)]
struct TranslatedText {
    detected_source_language: String,
    text: String,
}

impl DeepLClient {
    pub fn new(secrets: Arc<dyn SecretStore>) -> Result<Self, AppError> {
        Self::build(secrets, None, REQUEST_TIMEOUT)
    }

    /// `base_url_override` existe para apontar o cliente a um servidor falso nos testes, por
    /// isso só é acessível dentro do crate. Sem override, só HTTPS é aceito. Redirects nunca
    /// são seguidos: o texto do usuário não deve ser reenviado a outro destino.
    pub(crate) fn build(
        secrets: Arc<dyn SecretStore>,
        base_url_override: Option<String>,
        timeout: Duration,
    ) -> Result<Self, AppError> {
        let mut builder = Client::builder()
            .timeout(timeout)
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

/// Consumo do mês, em caracteres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Usage {
    pub used: u64,
    pub limit: u64,
}

#[derive(Deserialize)]
struct UsageResponse {
    character_count: u64,
    #[serde(default)]
    character_limit: Option<u64>,
}

impl DeepLClient {
    /// Confere a chave pedindo o consumo do mês: não gasta caracteres da cota.
    pub async fn check_key(&self) -> Result<Usage, AppError> {
        let key = self.secrets.require(SecretKind::Deepl)?;
        let authorization = authorization_header(&key)?;
        let base_url = self
            .base_url_override
            .as_deref()
            .unwrap_or_else(|| base_url_for_key(&key));

        let response = self
            .http
            .get(format!("{base_url}{USAGE_PATH}"))
            .header(AUTHORIZATION, authorization)
            .send()
            .await
            .map_err(network_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(error_for_status(status));
        }
        let body: UsageResponse = response.json().await.map_err(|error| {
            if error.is_decode() {
                AppError::Upstream(status.as_u16())
            } else {
                network_error(error)
            }
        })?;
        Ok(Usage {
            used: body.character_count,
            limit: body.character_limit.unwrap_or(0),
        })
    }
}

impl Translator for DeepLClient {
    async fn translate(&self, text: &str, target: TargetLang) -> Result<RawTranslation, AppError> {
        let key = self.secrets.require(SecretKind::Deepl)?;
        let authorization = authorization_header(&key)?;
        let base_url = self
            .base_url_override
            .as_deref()
            .unwrap_or_else(|| base_url_for_key(&key));

        let response = self
            .http
            .post(format!("{base_url}{TRANSLATE_PATH}"))
            .header(AUTHORIZATION, authorization)
            .json(&TranslateRequest {
                text: [text],
                target_lang: target.deepl_code(),
            })
            .send()
            .await
            .map_err(network_error)?;

        let status = response.status();
        if !status.is_success() {
            return Err(error_for_status(status));
        }

        // Corpo ilegível é erro do serviço; queda de conexão ou timeout no meio dele é de rede.
        let body: TranslateResponse = response.json().await.map_err(|error| {
            if error.is_decode() {
                AppError::Upstream(status.as_u16())
            } else {
                network_error(error)
            }
        })?;

        body.translations
            .into_iter()
            .next()
            .map(|translated| RawTranslation {
                text: translated.text,
                detected_source: translated.detected_source_language,
            })
            .ok_or(AppError::Upstream(status.as_u16()))
    }
}

/// O detalhe fica só no `Debug` (log). A URL é removida por precaução.
pub(crate) fn network_error(error: reqwest::Error) -> AppError {
    AppError::Network(error.without_url().to_string())
}

/// Monta o header marcando-o como sensível, para que não apareça em `Debug` nem em logs.
fn authorization_header(key: &str) -> Result<HeaderValue, AppError> {
    let mut value = HeaderValue::from_str(&format!("DeepL-Auth-Key {key}"))
        .map_err(|_| AppError::InvalidApiKey(SecretKind::Deepl))?;
    value.set_sensitive(true);
    Ok(value)
}

fn error_for_status(status: StatusCode) -> AppError {
    match status.as_u16() {
        401 | 403 => AppError::InvalidApiKey(SecretKind::Deepl),
        STATUS_QUOTA_EXCEEDED => AppError::QuotaExceeded,
        429 => AppError::RateLimited,
        other => AppError::Upstream(other),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{body_json, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::secrets::InMemoryStore;

    const FREE_KEY: &str = "chave-de-teste:fx";

    fn store_with(key: Option<&str>) -> Arc<dyn SecretStore> {
        let store = InMemoryStore::new();
        if let Some(key) = key {
            store.set(SecretKind::Deepl, key).unwrap();
        }
        Arc::new(store)
    }

    fn client_with_timeout(
        server: &MockServer,
        key: Option<&str>,
        timeout: Duration,
    ) -> DeepLClient {
        DeepLClient::build(store_with(key), Some(server.uri()), timeout).unwrap()
    }

    fn client(server: &MockServer, key: Option<&str>) -> DeepLClient {
        client_with_timeout(server, key, Duration::from_secs(2))
    }

    fn translation_body(text: &str, detected: &str) -> serde_json::Value {
        json!({ "translations": [{ "detected_source_language": detected, "text": text }] })
    }

    async fn server_replying(response: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/translate"))
            .respond_with(response)
            .mount(&server)
            .await;
        server
    }

    async fn request_count(server: &MockServer) -> usize {
        server.received_requests().await.unwrap().len()
    }

    #[tokio::test]
    async fn sends_an_authenticated_json_request_and_parses_the_translation() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v2/translate"))
            .and(header(
                "authorization",
                format!("DeepL-Auth-Key {FREE_KEY}").as_str(),
            ))
            .and(body_json(
                json!({ "text": ["hello"], "target_lang": "PT-BR" }),
            ))
            .respond_with(ResponseTemplate::new(200).set_body_json(translation_body("olá", "EN")))
            .expect(1)
            .mount(&server)
            .await;

        let result = client(&server, Some(FREE_KEY))
            .translate("hello", TargetLang::PtBr)
            .await
            .unwrap();

        assert_eq!(
            result,
            RawTranslation {
                text: "olá".into(),
                detected_source: "EN".into(),
            }
        );
    }

    #[tokio::test]
    async fn requests_en_us_when_asked_to_translate_to_english() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_json(
                json!({ "text": ["bom dia"], "target_lang": "EN-US" }),
            ))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(translation_body("good morning", "PT")),
            )
            .expect(1)
            .mount(&server)
            .await;

        let result = client(&server, Some(FREE_KEY))
            .translate("bom dia", TargetLang::EnUs)
            .await
            .unwrap();

        assert_eq!(result.text, "good morning");
    }

    #[tokio::test]
    async fn maps_http_failures_to_typed_errors() {
        let cases = [
            (401, "invalid_api_key"),
            (403, "invalid_api_key"),
            (456, "quota_exceeded"),
            (429, "rate_limited"),
            (500, "upstream"),
            (503, "upstream"),
        ];

        for (status, expected_code) in cases {
            let server = server_replying(ResponseTemplate::new(status)).await;

            let error = client(&server, Some(FREE_KEY))
                .translate("hello", TargetLang::PtBr)
                .await
                .unwrap_err();

            assert_eq!(error.code(), expected_code, "status {status}");
        }
    }

    #[tokio::test]
    async fn does_not_follow_redirects() {
        let target = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(200).set_body_json(translation_body("olá", "EN")))
            .mount(&target)
            .await;
        let origin = server_replying(ResponseTemplate::new(307).insert_header(
            "location",
            format!("{}/v2/translate", target.uri()).as_str(),
        ))
        .await;

        let error = client(&origin, Some(FREE_KEY))
            .translate("hello", TargetLang::PtBr)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "upstream");
        assert_eq!(request_count(&target).await, 0);
    }

    #[tokio::test]
    async fn includes_the_status_in_the_upstream_message() {
        let server = server_replying(ResponseTemplate::new(502)).await;

        let error = client(&server, Some(FREE_KEY))
            .translate("hello", TargetLang::PtBr)
            .await
            .unwrap_err();

        assert!(error.to_string().contains("502"));
    }

    #[tokio::test]
    async fn times_out_on_slow_responses() {
        let server = server_replying(
            ResponseTemplate::new(200)
                .set_body_json(translation_body("olá", "EN"))
                .set_delay(Duration::from_millis(600)),
        )
        .await;

        let error = client_with_timeout(&server, Some(FREE_KEY), Duration::from_millis(100))
            .translate("hello", TargetLang::PtBr)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "network");
    }

    #[tokio::test]
    async fn reports_a_network_error_without_leaking_the_key() {
        let client = DeepLClient::build(
            store_with(Some(FREE_KEY)),
            Some("http://127.0.0.1:1".into()),
            Duration::from_secs(2),
        )
        .unwrap();

        let error = client
            .translate("hello", TargetLang::PtBr)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "network");
        assert!(!format!("{error:?}").contains(FREE_KEY));
    }

    #[tokio::test]
    async fn rejects_a_non_json_success_body_as_an_upstream_error() {
        let server =
            server_replying(ResponseTemplate::new(200).set_body_string("isto não é json")).await;

        let error = client(&server, Some(FREE_KEY))
            .translate("hello", TargetLang::PtBr)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "upstream");
    }

    #[tokio::test]
    async fn rejects_a_response_without_translations() {
        let server = server_replying(
            ResponseTemplate::new(200).set_body_json(json!({ "translations": [] })),
        )
        .await;

        let error = client(&server, Some(FREE_KEY))
            .translate("hello", TargetLang::PtBr)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "upstream");
    }

    #[tokio::test]
    async fn fails_without_a_key_and_makes_no_request() {
        let server = server_replying(ResponseTemplate::new(200)).await;

        let error = client(&server, None)
            .translate("hello", TargetLang::PtBr)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "missing_secret");
        assert_eq!(request_count(&server).await, 0);
    }

    #[tokio::test]
    async fn rejects_a_key_that_cannot_be_sent_as_a_header_and_makes_no_request() {
        let server = server_replying(ResponseTemplate::new(200)).await;

        let error = client(&server, Some("abc\ndef"))
            .translate("hello", TargetLang::PtBr)
            .await
            .unwrap_err();

        assert_eq!(error.code(), "invalid_api_key");
        assert_eq!(request_count(&server).await, 0);
    }

    async fn usage_server(response: ResponseTemplate) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/v2/usage"))
            .and(header(
                "authorization",
                format!("DeepL-Auth-Key {FREE_KEY}").as_str(),
            ))
            .respond_with(response)
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn check_key_reads_the_monthly_usage() {
        let server = usage_server(
            ResponseTemplate::new(200)
                .set_body_json(json!({ "character_count": 1234, "character_limit": 500000 })),
        )
        .await;

        let usage = client(&server, Some(FREE_KEY)).check_key().await.unwrap();

        assert_eq!(
            usage,
            Usage {
                used: 1234,
                limit: 500_000
            }
        );
    }

    #[tokio::test]
    async fn check_key_tolerates_an_account_without_a_limit() {
        let server =
            usage_server(ResponseTemplate::new(200).set_body_json(json!({ "character_count": 7 })))
                .await;

        let usage = client(&server, Some(FREE_KEY)).check_key().await.unwrap();

        assert_eq!(usage, Usage { used: 7, limit: 0 });
    }

    #[tokio::test]
    async fn check_key_maps_failures_to_typed_errors() {
        let cases = [
            (401, "invalid_api_key"),
            (403, "invalid_api_key"),
            (456, "quota_exceeded"),
            (429, "rate_limited"),
            (500, "upstream"),
        ];

        for (status, expected_code) in cases {
            let server = usage_server(ResponseTemplate::new(status)).await;

            let error = client(&server, Some(FREE_KEY))
                .check_key()
                .await
                .unwrap_err();

            assert_eq!(error.code(), expected_code, "status {status}");
        }
    }

    #[tokio::test]
    async fn check_key_without_a_key_makes_no_request() {
        let server = usage_server(ResponseTemplate::new(200)).await;

        let error = client(&server, None).check_key().await.unwrap_err();

        assert_eq!(error.code(), "missing_secret");
        assert_eq!(request_count(&server).await, 0);
    }

    #[tokio::test]
    async fn check_key_rejects_a_body_that_is_not_usage() {
        let server = usage_server(ResponseTemplate::new(200).set_body_string("nada")).await;

        let error = client(&server, Some(FREE_KEY))
            .check_key()
            .await
            .unwrap_err();

        assert_eq!(error.code(), "upstream");
    }

    #[test]
    fn picks_the_host_from_the_key_suffix() {
        assert_eq!(base_url_for_key("abc:fx"), "https://api-free.deepl.com");
        assert_eq!(base_url_for_key("abc"), "https://api.deepl.com");
    }
}
