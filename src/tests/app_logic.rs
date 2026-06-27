use super::super::App;
use crossterm::event::KeyCode;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn handle_key_event() {
    let mut app = App::default();
    assert_eq!(app.selected_kpi_index(), 0);
    assert_eq!(app.numeric_column_name(), "Receita");
    assert!(!app.is_report_view());

    app.handle_key_event(KeyCode::Char('r').into());
    assert!(app.is_report_view());

    app.handle_key_event(KeyCode::Down.into());
    assert_eq!(app.selected_kpi_index(), 0);

    app.handle_key_event(KeyCode::Char('z').into());
    assert!(!app.is_report_view());

    app.handle_key_event(KeyCode::Down.into());
    assert_eq!(app.selected_kpi_index(), 1);

    app.handle_key_event(KeyCode::Up.into());
    assert_eq!(app.selected_kpi_index(), 0);

    app.handle_key_event(KeyCode::Right.into());
    assert_eq!(app.numeric_column_name(), "Pedidos");

    app.handle_key_event(KeyCode::Char('q').into());
    assert!(app.should_exit());
}

#[test]
fn preview_table_has_headers() {
    let app = App::default();
    let preview = app.preview_table_lines(5);

    assert!(!preview.is_empty());
    assert!(preview[0].contains("Mes"));
    assert!(preview[0].contains("Receita"));
    assert!(preview.iter().any(|line| line.contains("Jan")));
}

#[test]
fn csv_preview_reports_loading_need() {
    let app = App::default();

    assert!(!app.has_csv_source());
    assert_eq!(app.csv_preview_rows(), 0);
}

#[test]
fn kpi_selection_updates_results() {
    let mut app = App::default();

    assert_eq!(app.kpi_results().len(), 4);
    assert_eq!(app.kpi_results()[0].label, "Total de linhas");
    assert_eq!(app.kpi_results()[0].value, "4");

    app.handle_key_event(KeyCode::Right.into());
    assert_eq!(app.numeric_column_name(), "Pedidos");
    assert_eq!(app.kpi_results()[2].label, "Soma");
    assert_eq!(app.kpi_results()[2].value, "543");
}

#[test]
fn large_csv_returns_to_preview_after_report() {
    let path = temp_file_path("ratatui_excel_analyser_roundtrip.csv");
    let mut content = String::from("Mes;Receita;Pedidos\n");
    for index in 0..1050 {
        content.push_str(&format!("M{};{};{}\n", index, index as f64 + 0.5, index * 2));
    }
    fs::write(&path, content).unwrap();

    let (workbook, truncated) = super::super::analytics::excel::import_csv_preview(&path, 1000).unwrap();
    assert!(truncated);

    let mut app = App::default();
    app.workbook = workbook;
    app.csv_source = Some(path.clone());
    app.csv_preview_rows = 1000;
    app.refresh_numeric_columns();

    assert_eq!(app.row_count(), 1000);

    app.handle_key_event(KeyCode::Char('r').into());
    assert!(app.is_loading_view());

    app.finish_pending_csv_load();
    assert!(app.is_report_view());
    assert_eq!(app.row_count(), 1000);

    app.handle_key_event(KeyCode::Char('z').into());
    assert!(!app.is_loading_view());
    assert_eq!(app.row_count(), 1000);

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
