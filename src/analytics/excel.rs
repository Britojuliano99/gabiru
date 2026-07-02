use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use calamine::{Data, Reader, open_workbook_auto};
use csv::ReaderBuilder;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CsvDialect {
    delimiter: u8,
    decimal_separator: char,
}

#[derive(Clone, Debug)]
pub struct ImportedWorkbook {
    pub file_name: String,
    pub sheets: Vec<ImportedSheet>,
}

#[derive(Clone, Debug)]
pub struct ImportedSheet {
    pub name: String,
    pub headers: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

impl ImportedSheet {
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn column_count(&self) -> usize {
        self.headers.len().max(self.rows.first().map_or(0, Vec::len))
    }

    pub fn missing_values_count(&self) -> usize {
        self.rows
            .iter()
            .flat_map(|row| row.iter())
            .filter(|cell| cell.trim().is_empty())
            .count()
    }

    pub fn numeric_column_indices(&self) -> Vec<usize> {
        let column_count = self.column_count();
        (0..column_count)
            .filter(|&column| {
                self.rows.iter().any(|row| {
                    row.get(column)
                        .and_then(|value| value.trim().parse::<f64>().ok())
                        .is_some()
                })
            })
            .collect()
    }

    pub fn numeric_values(&self, column: usize) -> Vec<f64> {
        self.rows
            .iter()
            .filter_map(|row| row.get(column))
            .filter_map(|value| value.trim().parse::<f64>().ok())
            .collect()
    }

    pub fn column_name(&self, column: usize) -> String {
        self.headers
            .get(column)
            .cloned()
            .unwrap_or_else(|| format!("Coluna {}", column + 1))
    }
}

pub fn import_workbook(path: impl AsRef<Path>) -> Result<ImportedWorkbook, String> {
    let path = path.as_ref();
    let mut workbook = open_workbook_auto(path)
        .map_err(|err| format!("Falha ao abrir o arquivo {}: {err}", path.display()))?;

    let sheet_names = workbook.sheet_names().to_vec();
    if sheet_names.is_empty() {
        return Err("Arquivo Excel sem abas".to_string());
    }

    let mut sheets = Vec::new();
    for name in sheet_names {
        if let Ok(range) = workbook.worksheet_range(&name) {
            let mut rows_iter = range.rows();
            let headers = rows_iter
                .next()
                .map(|row| row.iter().map(cell_to_string).collect())
                .unwrap_or_default();

            let rows = rows_iter
                .map(|row| row.iter().map(cell_to_string).collect())
                .collect();

            sheets.push(ImportedSheet {
                name,
                headers,
                rows,
            });
        }
    }

    if sheets.is_empty() {
        return Err("Nenhuma aba legivel encontrada".to_string());
    }

    Ok(ImportedWorkbook {
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("desconhecido")
            .to_string(),
        sheets,
    })
}

pub fn import_csv(
    path: impl AsRef<Path>,
    delimiter: u8,
    decimal_separator: char,
) -> Result<ImportedWorkbook, String> {
    let path = path.as_ref();
    let file = std::fs::File::open(path)
        .map_err(|err| format!("Falha ao abrir o arquivo {}: {err}", path.display()))?;

    import_csv_reader(file, delimiter, decimal_separator, path)
}

pub fn import_csv_preview(
    path: impl AsRef<Path>,
    preview_limit: usize,
) -> Result<(ImportedWorkbook, bool), String> {
    let (workbook, truncated, _) = import_csv_preview_with_total_rows(path, preview_limit)?;
    Ok((workbook, truncated))
}

pub fn import_csv_preview_with_total_rows(
    path: impl AsRef<Path>,
    preview_limit: usize,
) -> Result<(ImportedWorkbook, bool, usize), String> {
    let path = path.as_ref();
    let dialect = infer_csv_dialect(path)?;
    import_csv_reader_preview(path, dialect.delimiter, dialect.decimal_separator, preview_limit)
}

pub fn import_csv_auto(path: impl AsRef<Path>) -> Result<ImportedWorkbook, String> {
    let path = path.as_ref();
    let dialect = infer_csv_dialect(path)?;
    import_csv(path, dialect.delimiter, dialect.decimal_separator)
}

pub fn sample_workbook() -> ImportedWorkbook {
    ImportedWorkbook {
        file_name: "amostra.xlsx".to_string(),
        sheets: vec![ImportedSheet {
            name: "Vendas".to_string(),
            headers: vec![
                "Mes".to_string(),
                "Receita".to_string(),
                "Pedidos".to_string(),
                "Desconto".to_string(),
            ],
            rows: vec![
                vec!["Jan".to_string(), "12000".to_string(), "120".to_string(), "300".to_string()],
                vec!["Fev".to_string(), "14500".to_string(), "140".to_string(), "410".to_string()],
                vec!["Mar".to_string(), "13100".to_string(), "132".to_string(), "360".to_string()],
                vec!["Abr".to_string(), "15800".to_string(), "151".to_string(), "390".to_string()],
            ],
        }],
    }
}

fn cell_to_string(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(value) => value.to_string(),
        Data::Float(value) => format_float(*value),
        Data::Int(value) => value.to_string(),
        Data::Bool(value) => value.to_string(),
        Data::DateTime(value) => value.to_string(),
        Data::DateTimeIso(value) => value.to_string(),
        Data::DurationIso(value) => value.to_string(),
        Data::Error(err) => format!("Erro({err:?})"),
    }
}

fn format_float(value: f64) -> String {
    if (value.fract() - 0.0).abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn import_csv_reader<R: Read>(
    reader: R,
    delimiter: u8,
    decimal_separator: char,
    path: &Path,
) -> Result<ImportedWorkbook, String> {
    let mut csv_reader = ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .from_reader(reader);

    let mut records = csv_reader.records();
    let headers = match records.next() {
        Some(record) => record
            .map_err(|err| format!("Falha ao ler o arquivo {}: {err}", path.display()))?
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        None => return Err("Arquivo CSV vazio".to_string()),
    };

    let rows = records
        .map(|record| {
            record
                .map_err(|err| format!("Falha ao ler o arquivo {}: {err}", path.display()))
                .map(|record| {
                    record
                        .iter()
                        .map(|value| normalize_csv_cell(value, decimal_separator))
                        .collect::<Vec<_>>()
                })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ImportedWorkbook {
        file_name: path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("desconhecido")
            .to_string(),
        sheets: vec![ImportedSheet {
            name: "CSV".to_string(),
            headers,
            rows,
        }],
    })
}

fn import_csv_reader_preview(
    path: &Path,
    delimiter: u8,
    decimal_separator: char,
    preview_limit: usize,
) -> Result<(ImportedWorkbook, bool, usize), String> {
    let file = File::open(path)
        .map_err(|err| format!("Falha ao abrir o arquivo {}: {err}", path.display()))?;

    let mut csv_reader = ReaderBuilder::new()
        .has_headers(false)
        .delimiter(delimiter)
        .from_reader(file);

    let mut records = csv_reader.records();
    let headers = match records.next() {
        Some(record) => record
            .map_err(|err| format!("Falha ao ler o arquivo {}: {err}", path.display()))?
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>(),
        None => return Err("Arquivo CSV vazio".to_string()),
    };

    let mut rows = Vec::new();
    let mut truncated = false;
    let mut total_rows = 0usize;

    while let Some(record) = records.next() {
        total_rows += 1;
        let record = record.map_err(|err| format!("Falha ao ler o arquivo {}: {err}", path.display()))?;

        if preview_limit > 0 && rows.len() >= preview_limit {
            truncated = true;
            continue;
        }

        rows.push(
            record
                .iter()
                .map(|value| normalize_csv_cell(value, decimal_separator))
                .collect::<Vec<_>>(),
        );
    }

    Ok((
        ImportedWorkbook {
            file_name: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("desconhecido")
                .to_string(),
            sheets: vec![ImportedSheet {
                name: "CSV".to_string(),
                headers,
                rows,
            }],
        },
        truncated,
        total_rows,
    ))
}

fn infer_csv_dialect(path: &Path) -> Result<CsvDialect, String> {
    let file = File::open(path)
        .map_err(|err| format!("Falha ao abrir o arquivo {}: {err}", path.display()))?;
    let reader = BufReader::new(file);

    let sample_lines = reader
        .lines()
        .take(16)
        .filter_map(|line| line.ok())
        .map(|line| line.trim().to_string())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if sample_lines.is_empty() {
        return Err("Arquivo CSV vazio".to_string());
    }

    let delimiter = infer_delimiter(&sample_lines).unwrap_or(b',');
    let decimal_separator = infer_decimal_separator(&sample_lines, delimiter);

    Ok(CsvDialect {
        delimiter,
        decimal_separator,
    })
}

fn infer_delimiter(lines: &[String]) -> Option<u8> {
    let candidates = [b';', b',', b'\t', b'|'];
    candidates
        .iter()
        .copied()
        .map(|candidate| {
            let mut counts = std::collections::BTreeMap::new();
            for line in lines {
                let field_count = line.split(candidate as char).count();
                if field_count > 1 {
                    *counts.entry(field_count).or_insert(0usize) += 1;
                }
            }

            let (mode_count, mode_hits) = counts
                .into_iter()
                .max_by_key(|(field_count, hits)| (*hits, *field_count))
                .map(|(field_count, hits)| (field_count, hits))
                .unwrap_or((1, 0));

            (candidate, mode_hits, mode_count)
        })
        .max_by_key(|(_, mode_hits, mode_count)| (*mode_hits, *mode_count))
        .map(|(candidate, _, _)| candidate)
}

fn infer_decimal_separator(lines: &[String], delimiter: u8) -> char {
    let mut comma_votes = 0usize;
    let mut dot_votes = 0usize;

    for line in lines {
        for field in line.split(delimiter as char).map(str::trim) {
            if field.is_empty() {
                continue;
            }

            if field.contains(',') && !field.contains('.') {
                if field.replace(',', ".").parse::<f64>().is_ok() {
                    comma_votes += 1;
                }
            } else if field.contains('.') && !field.contains(',') {
                if field.parse::<f64>().is_ok() {
                    dot_votes += 1;
                }
            }
        }
    }

    if comma_votes > dot_votes {
        ','
    } else {
        '.'
    }
}

fn normalize_csv_cell(value: &str, decimal_separator: char) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() || decimal_separator == '.' {
        return value.to_string();
    }

    let normalized = trimmed.replace(decimal_separator, ".");
    if normalized.parse::<f64>().is_ok() {
        normalized
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn import_csv_with_custom_delimiter_and_decimal() {
        let path = temp_file_path("ratatui_excel_analyser_csv_test.csv");
        fs::write(
            &path,
            "Mes;Receita;Pedidos\nJan;12000,5;120\nFev;14500,25;140\n",
        )
        .unwrap();

        let workbook = import_csv(&path, b';', ',').unwrap();
        let sheet = &workbook.sheets[0];

        assert_eq!(workbook.file_name, path.file_name().unwrap().to_string_lossy());
        assert_eq!(sheet.name, "CSV");
        assert_eq!(sheet.headers, vec!["Mes", "Receita", "Pedidos"]);
        assert_eq!(sheet.row_count(), 2);
        assert_eq!(sheet.numeric_values(1), vec![12000.5, 14500.25]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn import_csv_auto_detects_semicolon_and_decimal_comma() {
        let path = temp_file_path("ratatui_excel_analyser_csv_auto_semicolon.csv");
        fs::write(
            &path,
            "Mes;Receita;Pedidos\nJan;12000,5;120\nFev;14500,25;140\n",
        )
        .unwrap();

        let workbook = import_csv_auto(&path).unwrap();
        let sheet = &workbook.sheets[0];

        assert_eq!(sheet.headers, vec!["Mes", "Receita", "Pedidos"]);
        assert_eq!(sheet.numeric_values(1), vec![12000.5, 14500.25]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn import_csv_auto_detects_comma_and_decimal_point() {
        let path = temp_file_path("ratatui_excel_analyser_csv_auto_comma.csv");
        fs::write(
            &path,
            "Mes,Receita,Pedidos\nJan,12000.5,120\nFev,14500.25,140\n",
        )
        .unwrap();

        let workbook = import_csv_auto(&path).unwrap();
        let sheet = &workbook.sheets[0];

        assert_eq!(sheet.headers, vec!["Mes", "Receita", "Pedidos"]);
        assert_eq!(sheet.numeric_values(1), vec![12000.5, 14500.25]);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn import_csv_preview_limits_rows() {
        let path = temp_file_path("ratatui_excel_analyser_csv_preview.csv");
        let mut content = String::from("Mes;Receita;Pedidos\n");
        for index in 0..1050 {
            content.push_str(&format!("M{};{};{}\n", index, index as f64 + 0.5, index * 2));
        }
        fs::write(&path, content).unwrap();

        let (workbook, truncated) = import_csv_preview(&path, 1000).unwrap();
        let sheet = &workbook.sheets[0];

        assert!(truncated);
        assert_eq!(sheet.row_count(), 1000);
        assert_eq!(sheet.headers, vec!["Mes", "Receita", "Pedidos"]);

        let _ = fs::remove_file(path);
    }

    fn temp_file_path(file_name: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique_suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        path.push(format!("{}_{}_{}", file_name, std::process::id(), unique_suffix));
        path
    }
}
