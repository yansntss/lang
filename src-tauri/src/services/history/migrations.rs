use rusqlite::Connection;

use super::sqlite::db_error;
use crate::error::AppError;

/// Cada item leva o banco da versão `índice` para `índice + 1` (`PRAGMA user_version`).
/// Nunca edite uma migração já lançada: acrescente outra.
const MIGRATIONS: &[&str] = &["
CREATE TABLE history (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    source_text      TEXT    NOT NULL,
    translated_text  TEXT    NOT NULL,
    source_lang      TEXT    NOT NULL,
    target_lang      TEXT    NOT NULL,
    favorite         INTEGER NOT NULL DEFAULT 0,
    created_at       INTEGER NOT NULL,
    last_used_at     INTEGER NOT NULL,
    use_count        INTEGER NOT NULL DEFAULT 1,
    last_reviewed_at INTEGER,
    UNIQUE (source_text, target_lang)
);
CREATE INDEX idx_history_last_used ON history (last_used_at DESC);
CREATE TABLE preferences (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
"];

pub fn latest_version() -> u32 {
    u32::try_from(MIGRATIONS.len()).unwrap_or(u32::MAX)
}

/// Leva o banco até a versão mais recente. Um banco de versão futura é recusado: abri-lo
/// poderia corrompê-lo.
pub fn migrate(conn: &mut Connection) -> Result<(), AppError> {
    let current: u32 = conn
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(db_error)?;

    if current > latest_version() {
        return Err(AppError::History(format!(
            "banco na versão {current}, mais nova que a {} suportada",
            latest_version()
        )));
    }

    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let transaction = conn.transaction().map_err(db_error)?;
        transaction.execute_batch(sql).map_err(db_error)?;
        transaction
            .execute_batch(&format!("PRAGMA user_version = {}", index + 1))
            .map_err(db_error)?;
        transaction.commit().map_err(db_error)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(conn: &Connection) -> u32 {
        conn.query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap()
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
    }

    #[test]
    fn a_new_database_is_brought_to_the_latest_version() {
        let mut conn = Connection::open_in_memory().unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(version(&conn), latest_version());
        assert_eq!(count(&conn, "history"), 0);
        assert_eq!(count(&conn, "preferences"), 0);
    }

    #[test]
    fn migrating_twice_keeps_the_data() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        conn.execute(
            "INSERT INTO history (source_text, translated_text, source_lang, target_lang, \
             created_at, last_used_at) VALUES ('a', 'b', 'EN', 'PT-BR', 1, 1)",
            [],
        )
        .unwrap();

        migrate(&mut conn).unwrap();

        assert_eq!(count(&conn, "history"), 1);
        assert_eq!(version(&conn), latest_version());
    }

    #[test]
    fn a_database_from_a_future_version_is_refused() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(&format!("PRAGMA user_version = {}", latest_version() + 1))
            .unwrap();

        let error = migrate(&mut conn).unwrap_err();

        assert_eq!(error.code(), "history");
    }
}
