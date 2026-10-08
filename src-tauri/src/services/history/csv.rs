use super::HistoryEntry;

/// Marca de ordem de bytes UTF-8: sem ela, o Excel abre o CSV com os acentos quebrados.
const BOM: &str = "\u{FEFF}";
const HEADER: [&str; 7] = [
    "original",
    "traducao",
    "idioma_origem",
    "idioma_destino",
    "favorito",
    "usos",
    "ultimo_uso",
];
const MILLIS_PER_SECOND: i64 = 1000;
const SECONDS_PER_DAY: i64 = 86_400;

/// Gera o CSV do histórico (UTF-8 com BOM, CRLF, todos os campos entre aspas).
pub fn export(entries: &[HistoryEntry]) -> String {
    let mut out = String::from(BOM);
    push_row(&mut out, HEADER.iter().map(|name| (*name).to_owned()));
    for entry in entries {
        push_row(
            &mut out,
            [
                entry.source_text.clone(),
                entry.translated_text.clone(),
                entry.source_lang.clone(),
                entry.target_lang.clone(),
                if entry.favorite { "sim" } else { "nao" }.to_owned(),
                entry.use_count.to_string(),
                iso_utc(entry.last_used_at),
            ],
        );
    }
    out
}

/// Nome do arquivo exportado, ex.: `traducoes-20231114-221320.csv`.
pub fn file_name(now_ms: i64) -> String {
    let compact: String = iso_utc(now_ms)
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    format!("traducoes-{}-{}.csv", &compact[..8], &compact[8..])
}

fn push_row(out: &mut String, fields: impl IntoIterator<Item = String>) {
    let line: Vec<String> = fields.into_iter().map(|field| quote(&field)).collect();
    out.push_str(&line.join(","));
    out.push_str("\r\n");
}

/// Planilhas executam células que começam com `=`, `+`, `-` ou `@` como fórmula: um texto
/// copiado de qualquer lugar poderia rodar código ao abrir o arquivo. O apóstrofo neutraliza.
fn quote(field: &str) -> String {
    let guard = if field.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        "'"
    } else {
        ""
    };
    format!("\"{guard}{}\"", field.replace('"', "\"\""))
}

/// Data e hora UTC no formato `2023-11-14T22:13:20Z`, sem depender de crate de calendário.
fn iso_utc(millis: i64) -> String {
    let seconds = millis.div_euclid(MILLIS_PER_SECOND);
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let in_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        in_day / 3600,
        in_day % 3600 / 60,
        in_day % 60
    )
}

/// Dias desde 1970-01-01 para (ano, mês, dia) do calendário gregoriano (algoritmo de H. Hinnant).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_index = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_index + 2) / 5 + 1;
    let month = if month_index < 10 {
        month_index + 3
    } else {
        month_index - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(source: &str, translation: &str) -> HistoryEntry {
        HistoryEntry {
            id: 1,
            source_text: source.into(),
            translated_text: translation.into(),
            source_lang: "EN".into(),
            target_lang: "PT-BR".into(),
            favorite: true,
            created_at: 0,
            last_used_at: 1_700_000_000_000,
            use_count: 3,
            last_reviewed_at: None,
        }
    }

    #[test]
    fn starts_with_the_bom_and_a_header_and_uses_crlf() {
        let csv = export(&[]);

        assert_eq!(
            csv,
            "\u{FEFF}\"original\",\"traducao\",\"idioma_origem\",\"idioma_destino\",\
             \"favorito\",\"usos\",\"ultimo_uso\"\r\n"
        );
    }

    #[test]
    fn writes_one_quoted_row_per_entry() {
        let csv = export(&[entry("hello", "olá")]);

        assert!(csv.ends_with(
            "\"hello\",\"olá\",\"EN\",\"PT-BR\",\"sim\",\"3\",\"2023-11-14T22:13:20Z\"\r\n"
        ));
    }

    #[test]
    fn escapes_quotes_commas_and_line_breaks_inside_a_field() {
        let csv = export(&[entry("say \"hi\", please\nnow", "ok")]);

        assert!(csv.contains("\"say \"\"hi\"\", please\nnow\",\"ok\""));
    }

    #[test]
    fn neutralizes_spreadsheet_formulas() {
        for dangerous in ["=1+1", "+cmd", "-2", "@SUM(A1)", "\tx", "\rx"] {
            let csv = export(&[entry(dangerous, "ok")]);

            assert!(
                csv.contains(&format!("\"'{dangerous}\"")),
                "{dangerous:?} deveria ter apóstrofo"
            );
        }
    }

    #[test]
    fn leaves_harmless_text_untouched() {
        let csv = export(&[entry("a=b", "x-y")]);

        assert!(csv.contains("\"a=b\",\"x-y\""));
    }

    #[test]
    fn formats_utc_timestamps() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_700_000_000_000), "2023-11-14T22:13:20Z");
        assert_eq!(iso_utc(951_782_400_000), "2000-02-29T00:00:00Z");
        assert_eq!(iso_utc(-1000), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn builds_a_sortable_file_name() {
        assert_eq!(
            file_name(1_700_000_000_000),
            "traducoes-20231114-221320.csv"
        );
    }
}
