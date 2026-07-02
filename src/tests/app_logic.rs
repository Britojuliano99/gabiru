use super::super::App;
use super::super::analytics::excel::{ImportedSheet, ImportedWorkbook};
use crossterm::event::{KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use ratatui::layout::Rect;

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

    app.handle_key_event(KeyCode::Char('d').into());
    assert_eq!(app.preview_column_offset(), 1);

    app.handle_key_event(KeyCode::Char('a').into());
    assert_eq!(app.preview_column_offset(), 0);

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
fn preview_table_pads_two_digit_indices() {
    let mut app = App::default();
    app.workbook = ImportedWorkbook {
        file_name: "teste.xlsx".to_string(),
        sheets: vec![ImportedSheet {
            name: "Dados".to_string(),
            headers: vec!["Mes".to_string(), "Receita".to_string()],
            rows: (0..12)
                .map(|index| vec![format!("M{}", index), (index * 10).to_string()])
                .collect(),
        }],
    };
    app.active_sheet = 0;

    let preview = app.preview_table_lines(12);

    assert!(preview[10].starts_with(" 9 |"));
    assert!(preview[11].starts_with("10 |"));
}

#[test]
fn preview_table_window_shifts_columns() {
    let mut app = App::default();
    app.workbook = ImportedWorkbook {
        file_name: "teste.xlsx".to_string(),
        sheets: vec![ImportedSheet {
            name: "Dados".to_string(),
            headers: vec![
                "Mes".to_string(),
                "Receita".to_string(),
                "Pedidos".to_string(),
                "Desconto".to_string(),
            ],
            rows: vec![
                vec!["Jan".to_string(), "12000".to_string(), "120".to_string(), "300".to_string()],
                vec!["Fev".to_string(), "14500".to_string(), "140".to_string(), "410".to_string()],
            ],
        }],
    };
    app.active_sheet = 0;

    let left_window = app.preview_table_window_lines(2, 0, 30);
    let right_window = app.preview_table_window_lines(2, 2, 30);

    assert!(left_window[0].contains("Mes"));
    assert!(left_window[0].contains("Receita"));
    assert!(!left_window[0].contains("Desconto"));
    assert!(right_window[0].contains("Pedidos"));
    assert!(right_window[0].contains("Desconto"));
    assert!(!right_window[0].contains("Mes"));
}

#[test]
fn mouse_scroll_moves_preview_when_over_preview_area() {
    let mut app = App::default();
    app.set_preview_area(Some(Rect::new(40, 5, 30, 10)));

    app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 41,
        row: 6,
        modifiers: KeyModifiers::empty(),
    });
    assert_eq!(app.preview_offset(), 1);

    app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::ScrollRight,
        column: 41,
        row: 6,
        modifiers: KeyModifiers::empty(),
    });
    assert_eq!(app.preview_column_offset(), 1);
}

#[test]
fn mouse_scroll_and_click_work_on_selection_pane() {
    let mut app = App::default();
    app.set_selection_area(Some(Rect::new(1, 10, 30, 10)));

    app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 2,
        row: 11,
        modifiers: KeyModifiers::empty(),
    });
    assert_eq!(app.selected_kpi_index(), 1);

    app.handle_mouse_event(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 2,
        row: 13,
        modifiers: KeyModifiers::empty(),
    });

    assert_eq!(app.selected_kpi_index(), 1);
    assert!(!app.kpi_enabled(1));
}

#[test]
fn csv_preview_reports_loading_need() {
    let app = App::default();

    assert!(!app.has_csv_source());
    assert_eq!(app.csv_preview_rows(), 0);
}

#[test]
fn csv_preview_uses_total_rows_for_summary() {
    let mut app = App::default();
    app.csv_source = Some(PathBuf::from("teste.csv"));
    app.csv_preview_rows = 1000;
    app.csv_total_rows = 1050;

    assert_eq!(app.display_row_count(), 1050);
}

#[test]
fn timeseries_window_increases_and_clamps() {
    let mut app = App::default();
    app.workbook = ImportedWorkbook {
        file_name: "serie.xlsx".to_string(),
        sheets: vec![ImportedSheet {
            name: "Série".to_string(),
            headers: vec!["Data".to_string(), "Valor".to_string()],
            rows: vec![
                vec!["2024-01-01".to_string(), "10".to_string()],
                vec!["2024-02-01".to_string(), "20".to_string()],
                vec!["2024-03-01".to_string(), "30".to_string()],
                vec!["2024-04-01".to_string(), "40".to_string()],
                vec!["2024-05-01".to_string(), "50".to_string()],
            ],
        }],
    };
    app.active_sheet = 0;
    app.refresh_numeric_columns();
    app.ts_moving_avg_window = 2;

    app.handle_key_event(KeyCode::Char('3').into());
    assert!(app.is_timeseries_view());
    assert_eq!(app.ts_moving_avg_window(), 2);

    app.handle_key_event(KeyCode::Char('=').into());
    assert_eq!(app.ts_moving_avg_window(), 3);

    app.handle_key_event(KeyCode::Char('=').into());
    app.handle_key_event(KeyCode::Char('=').into());
    app.handle_key_event(KeyCode::Char('=').into());

    assert_eq!(app.ts_moving_avg_window(), 5);

    app.handle_key_event(KeyCode::Char('-').into());
    assert_eq!(app.ts_moving_avg_window(), 4);
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
