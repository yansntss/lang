//! Histórico de traduções em SQLite local. Só o Rust acessa o banco.

pub mod csv;
mod migrations;
mod sqlite;

use serde::{Deserialize, Serialize};

pub use sqlite::{now_ms, SqliteHistory};

/// Itens por página na listagem.
pub const PAGE_SIZE: u32 = 50;
/// Teto de itens **não favoritos**: ao passar, os mais antigos são apagados.
pub const MAX_ENTRIES: u32 = 5000;

/// De onde veio o texto traduzido: digitado ou selecionado (`text`) ou lido da tela (`ocr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistorySource {
    #[default]
    Text,
    Ocr,
}

impl HistorySource {
    /// Como fica gravado no banco.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Ocr => "ocr",
        }
    }

    /// Lê o valor do banco. Um valor desconhecido (banco editado à mão) conta como `text`.
    pub fn from_db(value: &str) -> Self {
        match value {
            "ocr" => Self::Ocr,
            _ => Self::Text,
        }
    }
}

/// Item do histórico, como o front o recebe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: i64,
    pub source_text: String,
    pub translated_text: String,
    pub source_lang: String,
    pub target_lang: String,
    pub favorite: bool,
    /// Milissegundos desde 1970-01-01 UTC.
    pub created_at: i64,
    pub last_used_at: i64,
    pub use_count: i64,
    pub last_reviewed_at: Option<i64>,
    pub source: HistorySource,
}

/// Uma tradução a ser gravada.
#[derive(Debug, Clone, Copy)]
pub struct NewEntry<'a> {
    pub source_text: &'a str,
    pub translated_text: &'a str,
    pub source_lang: &'a str,
    pub target_lang: &'a str,
    pub source: HistorySource,
}

/// Filtros da listagem. `query` vazio ou só com espaços não filtra.
#[derive(Debug, Clone, Default)]
pub struct ListQuery {
    pub query: Option<String>,
    pub favorites_only: bool,
    pub offset: u32,
}
