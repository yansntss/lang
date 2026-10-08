use std::path::Path;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension, Row};

use super::migrations::migrate;
use super::{HistoryEntry, ListQuery, NewEntry, MAX_ENTRIES, PAGE_SIZE};
use crate::error::AppError;
use crate::services::explain::MAX_TRANSLATION_CHARS;
use crate::services::translation::MAX_INPUT_CHARS;

const ENABLED_KEY: &str = "history_enabled";
const MAX_SOURCE_LANG_CHARS: usize = 16;
/// Idiomas de destino que o app produz (ver `TargetLang`).
const TARGET_LANGS: [&str; 2] = ["PT-BR", "EN-US"];

const COLUMNS: &str = "id, source_text, translated_text, source_lang, target_lang, favorite, \
                       created_at, last_used_at, use_count, last_reviewed_at";

type Clock = Box<dyn Fn() -> i64 + Send + Sync>;

/// Histórico em um arquivo SQLite. A conexão é única e protegida por mutex: as operações são
/// curtas e o app tem um usuário só.
pub struct SqliteHistory {
    conn: Mutex<Connection>,
    clock: Clock,
    max_entries: u32,
}

/// O detalhe do SQLite fica só no `Debug`; ao front chega uma frase genérica.
pub(super) fn db_error(error: rusqlite::Error) -> AppError {
    AppError::History(error.to_string())
}

fn io_error(error: std::io::Error) -> AppError {
    AppError::History(error.to_string())
}

/// Instante atual, em milissegundos desde 1970-01-01 UTC.
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

impl SqliteHistory {
    /// Abre (ou cria) o banco em `path`, criando as pastas que faltam, e aplica as migrações.
    pub fn open(path: &Path) -> Result<Self, AppError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(io_error)?;
        }
        let conn = Connection::open(path).map_err(db_error)?;
        Self::from_connection(conn, Box::new(now_ms), MAX_ENTRIES)
    }

    fn from_connection(
        mut conn: Connection,
        clock: Clock,
        max_entries: u32,
    ) -> Result<Self, AppError> {
        migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            clock,
            max_entries,
        })
    }

    fn conn(&self) -> MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Grava a tradução, ou atualiza a que já existe para o mesmo texto e idioma de destino.
    /// Com o histórico desligado não grava nada e não é erro.
    pub fn record(&self, entry: &NewEntry<'_>) -> Result<(), AppError> {
        let valid = validate(entry)?;
        let conn = self.conn();
        if !is_enabled(&conn)? {
            return Ok(());
        }

        let now = (self.clock)();
        conn.execute(
            "INSERT INTO history (source_text, translated_text, source_lang, target_lang, \
                                  created_at, last_used_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)
             ON CONFLICT (source_text, target_lang) DO UPDATE SET
                 translated_text = excluded.translated_text,
                 source_lang     = excluded.source_lang,
                 last_used_at    = excluded.last_used_at,
                 use_count       = use_count + 1",
            params![
                valid.source_text,
                valid.translated_text,
                valid.source_lang,
                valid.target_lang,
                now
            ],
        )
        .map_err(db_error)?;

        // Só os não favoritos contam para o teto: favorito nunca é apagado sozinho.
        conn.execute(
            "DELETE FROM history WHERE favorite = 0 AND id NOT IN (
                 SELECT id FROM history WHERE favorite = 0
                 ORDER BY last_used_at DESC, id DESC LIMIT ?1)",
            params![self.max_entries],
        )
        .map_err(db_error)?;
        Ok(())
    }

    /// Uma página do histórico, do uso mais recente para o mais antigo.
    pub fn list(&self, query: &ListQuery) -> Result<Vec<HistoryEntry>, AppError> {
        let pattern = query
            .query
            .as_deref()
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(like_pattern);
        let conn = self.conn();
        let mut statement = conn
            .prepare(&format!(
                "SELECT {COLUMNS} FROM history
                 WHERE (?1 IS NULL OR source_text LIKE ?1 ESCAPE '\\'
                                   OR translated_text LIKE ?1 ESCAPE '\\')
                   AND (?2 = 0 OR favorite = 1)
                 ORDER BY last_used_at DESC, id DESC
                 LIMIT ?3 OFFSET ?4"
            ))
            .map_err(db_error)?;
        let rows = statement
            .query_map(
                params![pattern, query.favorites_only, PAGE_SIZE, query.offset],
                map_row,
            )
            .map_err(db_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
    }

    /// Todos os itens, para exportar.
    pub fn all(&self) -> Result<Vec<HistoryEntry>, AppError> {
        let conn = self.conn();
        let mut statement = conn
            .prepare(&format!(
                "SELECT {COLUMNS} FROM history ORDER BY last_used_at DESC, id DESC"
            ))
            .map_err(db_error)?;
        let rows = statement.query_map([], map_row).map_err(db_error)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(db_error)
    }

    pub fn delete(&self, id: i64) -> Result<(), AppError> {
        self.conn()
            .execute("DELETE FROM history WHERE id = ?1", params![id])
            .map_err(db_error)?;
        Ok(())
    }

    /// Apaga tudo, favoritos inclusive.
    pub fn clear(&self) -> Result<(), AppError> {
        self.conn()
            .execute("DELETE FROM history", [])
            .map_err(db_error)?;
        Ok(())
    }

    /// Inverte o favorito e devolve o novo estado.
    pub fn toggle_favorite(&self, id: i64) -> Result<bool, AppError> {
        let conn = self.conn();
        let changed = conn
            .execute(
                "UPDATE history SET favorite = 1 - favorite WHERE id = ?1",
                params![id],
            )
            .map_err(db_error)?;
        if changed == 0 {
            return Err(not_found());
        }
        conn.query_row(
            "SELECT favorite FROM history WHERE id = ?1",
            params![id],
            |row| row.get::<_, i64>(0),
        )
        .map(|favorite| favorite != 0)
        .map_err(db_error)
    }

    /// O favorito que está há mais tempo sem revisão (os nunca revisados vêm primeiro).
    pub fn next_review(&self) -> Result<Option<HistoryEntry>, AppError> {
        self.conn()
            .query_row(
                &format!(
                    "SELECT {COLUMNS} FROM history WHERE favorite = 1
                     ORDER BY last_reviewed_at IS NOT NULL, last_reviewed_at, id
                     LIMIT 1"
                ),
                [],
                map_row,
            )
            .optional()
            .map_err(db_error)
    }

    pub fn mark_reviewed(&self, id: i64) -> Result<(), AppError> {
        let changed = self
            .conn()
            .execute(
                "UPDATE history SET last_reviewed_at = ?1 WHERE id = ?2",
                params![(self.clock)(), id],
            )
            .map_err(db_error)?;
        if changed == 0 {
            return Err(not_found());
        }
        Ok(())
    }

    /// Ligado por padrão: só deixa de gravar depois de desligado explicitamente.
    pub fn enabled(&self) -> Result<bool, AppError> {
        is_enabled(&self.conn())
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<(), AppError> {
        self.conn()
            .execute(
                "INSERT INTO preferences (key, value) VALUES (?1, ?2)
                 ON CONFLICT (key) DO UPDATE SET value = excluded.value",
                params![ENABLED_KEY, if enabled { "1" } else { "0" }],
            )
            .map_err(db_error)?;
        Ok(())
    }
}

fn not_found() -> AppError {
    AppError::InvalidInput("Item não encontrado no histórico.".into())
}

fn is_enabled(conn: &Connection) -> Result<bool, AppError> {
    let value: Option<String> = conn
        .query_row(
            "SELECT value FROM preferences WHERE key = ?1",
            params![ENABLED_KEY],
            |row| row.get(0),
        )
        .optional()
        .map_err(db_error)?;
    Ok(value.as_deref() != Some("0"))
}

fn map_row(row: &Row<'_>) -> rusqlite::Result<HistoryEntry> {
    Ok(HistoryEntry {
        id: row.get(0)?,
        source_text: row.get(1)?,
        translated_text: row.get(2)?,
        source_lang: row.get(3)?,
        target_lang: row.get(4)?,
        favorite: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        last_used_at: row.get(7)?,
        use_count: row.get(8)?,
        last_reviewed_at: row.get(9)?,
    })
}

/// `LIKE` com `%`, `_` e `\` tratados como texto comum, e não como curingas.
fn like_pattern(text: &str) -> String {
    let mut pattern = String::with_capacity(text.len() + 2);
    pattern.push('%');
    for character in text.chars() {
        if matches!(character, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(character);
    }
    pattern.push('%');
    pattern
}

fn validate<'a>(entry: &NewEntry<'a>) -> Result<NewEntry<'a>, AppError> {
    let source_text = entry.source_text.trim();
    let translated_text = entry.translated_text.trim();
    let source_lang = entry.source_lang.trim();

    if source_text.is_empty() || translated_text.is_empty() {
        return Err(AppError::InvalidInput(
            "Não há tradução para guardar no histórico.".into(),
        ));
    }
    if source_text.chars().count() > MAX_INPUT_CHARS
        || translated_text.chars().count() > MAX_TRANSLATION_CHARS
    {
        return Err(AppError::InvalidInput(
            "O texto é grande demais para o histórico.".into(),
        ));
    }
    if source_lang.is_empty()
        || source_lang.chars().count() > MAX_SOURCE_LANG_CHARS
        || !TARGET_LANGS.contains(&entry.target_lang)
    {
        return Err(AppError::InvalidInput(
            "Idioma inválido para o histórico.".into(),
        ));
    }

    Ok(NewEntry {
        source_text,
        translated_text,
        source_lang,
        target_lang: entry.target_lang,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicI64, Ordering};
    use std::sync::Arc;

    use super::*;

    struct Fixture {
        history: SqliteHistory,
        now: Arc<AtomicI64>,
    }

    impl Fixture {
        fn new() -> Self {
            Self::with_limit(MAX_ENTRIES)
        }

        fn with_limit(max_entries: u32) -> Self {
            let now = Arc::new(AtomicI64::new(1_000));
            let clock = Arc::clone(&now);
            let history = SqliteHistory::from_connection(
                Connection::open_in_memory().unwrap(),
                Box::new(move || clock.load(Ordering::SeqCst)),
                max_entries,
            )
            .unwrap();
            Self { history, now }
        }

        fn at(&self, millis: i64) -> &Self {
            self.now.store(millis, Ordering::SeqCst);
            self
        }

        fn record(&self, source: &str, translation: &str) {
            self.history.record(&entry(source, translation)).unwrap();
        }

        fn list(&self) -> Vec<HistoryEntry> {
            self.history.list(&ListQuery::default()).unwrap()
        }

        fn texts(&self) -> Vec<String> {
            self.list()
                .into_iter()
                .map(|item| item.source_text)
                .collect()
        }

        fn search(&self, text: &str) -> Vec<String> {
            self.history
                .list(&ListQuery {
                    query: Some(text.into()),
                    ..ListQuery::default()
                })
                .unwrap()
                .into_iter()
                .map(|item| item.source_text)
                .collect()
        }
    }

    fn entry<'a>(source: &'a str, translation: &'a str) -> NewEntry<'a> {
        NewEntry {
            source_text: source,
            translated_text: translation,
            source_lang: "EN",
            target_lang: "PT-BR",
        }
    }

    #[test]
    fn records_and_lists_an_entry() {
        let fixture = Fixture::new();

        fixture.at(5_000).record("hello", "olá");

        assert_eq!(
            fixture.list(),
            vec![HistoryEntry {
                id: 1,
                source_text: "hello".into(),
                translated_text: "olá".into(),
                source_lang: "EN".into(),
                target_lang: "PT-BR".into(),
                favorite: false,
                created_at: 5_000,
                last_used_at: 5_000,
                use_count: 1,
                last_reviewed_at: None,
            }]
        );
    }

    #[test]
    fn trims_the_texts_before_saving() {
        let fixture = Fixture::new();

        fixture.record("  hello \n", " olá ");

        assert_eq!(fixture.list()[0].source_text, "hello");
        assert_eq!(fixture.list()[0].translated_text, "olá");
    }

    #[test]
    fn repeating_a_translation_updates_it_instead_of_duplicating() {
        let fixture = Fixture::new();
        fixture.at(1_000).record("hello", "olá");

        fixture.at(9_000).record("hello", "oi");

        let items = fixture.list();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].translated_text, "oi");
        assert_eq!(items[0].use_count, 2);
        assert_eq!(items[0].created_at, 1_000);
        assert_eq!(items[0].last_used_at, 9_000);
    }

    #[test]
    fn the_same_text_for_another_target_language_is_a_separate_entry() {
        let fixture = Fixture::new();
        fixture.record("pain", "dor");

        fixture
            .history
            .record(&NewEntry {
                source_text: "pain",
                translated_text: "pão",
                source_lang: "FR",
                target_lang: "EN-US",
            })
            .unwrap();

        assert_eq!(fixture.list().len(), 2);
    }

    #[test]
    fn rejects_empty_oversized_and_invalid_entries() {
        let fixture = Fixture::new();
        let too_long = "a".repeat(MAX_INPUT_CHARS + 1);
        let invalid = [
            entry("", "olá"),
            entry("hello", "  "),
            entry(&too_long, "olá"),
            NewEntry {
                source_lang: "",
                ..entry("hello", "olá")
            },
            NewEntry {
                target_lang: "ES",
                ..entry("hello", "olá")
            },
        ];

        for candidate in &invalid {
            let error = fixture.history.record(candidate).unwrap_err();

            assert_eq!(error.code(), "invalid_input", "{candidate:?}");
        }
        assert!(fixture.list().is_empty());
    }

    #[test]
    fn is_enabled_by_default_and_stops_recording_when_turned_off() {
        let fixture = Fixture::new();
        assert!(fixture.history.enabled().unwrap());

        fixture.history.set_enabled(false).unwrap();
        fixture.record("hello", "olá");

        assert!(!fixture.history.enabled().unwrap());
        assert!(fixture.list().is_empty());

        fixture.history.set_enabled(true).unwrap();
        fixture.record("hello", "olá");
        assert_eq!(fixture.list().len(), 1);
    }

    #[test]
    fn turning_it_off_keeps_what_was_already_saved() {
        let fixture = Fixture::new();
        fixture.record("hello", "olá");

        fixture.history.set_enabled(false).unwrap();

        assert_eq!(fixture.list().len(), 1);
    }

    #[test]
    fn the_cap_removes_the_oldest_non_favorites_and_keeps_favorites() {
        let fixture = Fixture::with_limit(2);
        fixture.at(1).record("favorito antigo", "x");
        let favorite_id = fixture.list()[0].id;
        fixture.history.toggle_favorite(favorite_id).unwrap();

        fixture.at(2).record("a", "x");
        fixture.at(3).record("b", "x");
        fixture.at(4).record("c", "x");

        assert_eq!(fixture.texts(), vec!["c", "b", "favorito antigo"]);
    }

    #[test]
    fn lists_the_most_recently_used_first() {
        let fixture = Fixture::new();
        fixture.at(1).record("primeiro", "x");
        fixture.at(2).record("segundo", "x");
        fixture.at(3).record("primeiro", "x");

        assert_eq!(fixture.texts(), vec!["primeiro", "segundo"]);
    }

    #[test]
    fn searches_both_texts_ignoring_ascii_case() {
        let fixture = Fixture::new();
        fixture.at(1).record("Good morning", "bom dia");
        fixture.at(2).record("thank you", "OBRIGADO");

        assert_eq!(fixture.search("MORNING"), vec!["Good morning"]);
        assert_eq!(fixture.search("obrigado"), vec!["thank you"]);
        assert_eq!(fixture.search("  morning  "), vec!["Good morning"]);
        assert!(fixture.search("inexistente").is_empty());
        assert_eq!(fixture.search("   ").len(), 2);
    }

    #[test]
    fn search_treats_wildcards_as_plain_text() {
        let fixture = Fixture::new();
        fixture.at(1).record("100% sure", "x");
        fixture.at(2).record("snake_case", "x");
        fixture.at(3).record("path\\to", "x");
        fixture.at(4).record("sureX", "x");

        assert_eq!(fixture.search("%"), vec!["100% sure"]);
        assert_eq!(fixture.search("e_c"), vec!["snake_case"]);
        assert_eq!(fixture.search("\\"), vec!["path\\to"]);
    }

    #[test]
    fn search_is_not_open_to_sql_injection() {
        let fixture = Fixture::new();
        fixture.record("hello", "olá");

        assert!(fixture.search("'; DROP TABLE history; --").is_empty());
        assert_eq!(fixture.list().len(), 1);
    }

    #[test]
    fn paginates_in_pages_of_fifty() {
        let fixture = Fixture::new();
        for index in 0..55 {
            fixture.at(index).record(&format!("texto {index}"), "x");
        }

        let first = fixture.history.list(&ListQuery::default()).unwrap();
        let second = fixture
            .history
            .list(&ListQuery {
                offset: PAGE_SIZE,
                ..ListQuery::default()
            })
            .unwrap();

        assert_eq!(first.len(), 50);
        assert_eq!(first[0].source_text, "texto 54");
        assert_eq!(second.len(), 5);
        assert_eq!(second[4].source_text, "texto 0");
    }

    #[test]
    fn favorites_only_filters_the_list() {
        let fixture = Fixture::new();
        fixture.at(1).record("a", "x");
        fixture.at(2).record("b", "x");
        let id_a = fixture.list()[1].id;
        fixture.history.toggle_favorite(id_a).unwrap();

        let favorites = fixture
            .history
            .list(&ListQuery {
                favorites_only: true,
                ..ListQuery::default()
            })
            .unwrap();

        assert_eq!(favorites.len(), 1);
        assert_eq!(favorites[0].source_text, "a");
    }

    #[test]
    fn toggles_the_favorite_flag_both_ways() {
        let fixture = Fixture::new();
        fixture.record("hello", "olá");
        let id = fixture.list()[0].id;

        assert!(fixture.history.toggle_favorite(id).unwrap());
        assert!(fixture.list()[0].favorite);
        assert!(!fixture.history.toggle_favorite(id).unwrap());
        assert!(!fixture.list()[0].favorite);
    }

    #[test]
    fn toggling_or_reviewing_an_unknown_id_is_an_error() {
        let fixture = Fixture::new();

        assert_eq!(
            fixture.history.toggle_favorite(99).unwrap_err().code(),
            "invalid_input"
        );
        assert_eq!(
            fixture.history.mark_reviewed(99).unwrap_err().code(),
            "invalid_input"
        );
    }

    #[test]
    fn deletes_one_entry_and_clears_everything() {
        let fixture = Fixture::new();
        fixture.at(1).record("a", "x");
        fixture.at(2).record("b", "x");
        let id_b = fixture.list()[0].id;
        fixture.history.toggle_favorite(id_b).unwrap();

        fixture.history.delete(fixture.list()[1].id).unwrap();
        assert_eq!(fixture.list().len(), 1);
        fixture.history.delete(12_345).unwrap();

        fixture.history.clear().unwrap();
        assert!(fixture.list().is_empty());
        assert!(fixture.history.all().unwrap().is_empty());
    }

    #[test]
    fn review_offers_the_least_recently_reviewed_favorite_first() {
        let fixture = Fixture::new();
        fixture.at(1).record("a", "x");
        fixture.at(2).record("b", "x");
        fixture.at(3).record("c", "x");
        for item in fixture.list() {
            if item.source_text != "c" {
                fixture.history.toggle_favorite(item.id).unwrap();
            }
        }
        let first = fixture.history.next_review().unwrap().unwrap();
        assert_eq!(first.source_text, "a");

        fixture.at(10).history.mark_reviewed(first.id).unwrap();
        let second = fixture.history.next_review().unwrap().unwrap();
        assert_eq!(second.source_text, "b");

        fixture.at(20).history.mark_reviewed(second.id).unwrap();
        let third = fixture.history.next_review().unwrap().unwrap();
        assert_eq!(third.source_text, "a");
        assert_eq!(third.last_reviewed_at, Some(10));
    }

    #[test]
    fn review_is_empty_without_favorites() {
        let fixture = Fixture::new();
        fixture.record("hello", "olá");

        assert!(fixture.history.next_review().unwrap().is_none());
    }

    #[test]
    fn data_survives_reopening_the_file_and_missing_folders_are_created() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("novo").join("history.db");
        {
            let history = SqliteHistory::open(&path).unwrap();
            history.record(&entry("hello", "olá")).unwrap();
            history.set_enabled(false).unwrap();
        }

        let reopened = SqliteHistory::open(&path).unwrap();

        assert_eq!(reopened.list(&ListQuery::default()).unwrap().len(), 1);
        assert!(!reopened.enabled().unwrap());
    }

    #[test]
    fn opening_a_file_that_is_not_a_database_fails_with_a_history_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("history.db");
        std::fs::write(
            &path,
            "isto nao e um banco sqlite, e tem bytes suficientes\n".repeat(50),
        )
        .unwrap();

        let error = SqliteHistory::open(&path).err().unwrap();

        assert_eq!(error.code(), "history");
    }
}
