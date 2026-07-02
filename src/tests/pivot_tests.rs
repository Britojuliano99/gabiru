use super::super::analytics::excel::{ImportedSheet, ImportedWorkbook};
use super::super::analytics::pivot::{
    PivotAggregation, PivotConfig, aggregate_average, aggregate_count_distinct,
    aggregate_median, aggregate_stddev, compute_pivot,
};
use super::super::App;

fn sample_workbook() -> ImportedWorkbook {
    ImportedWorkbook {
        file_name: "pivot.xlsx".to_string(),
        sheets: vec![ImportedSheet {
            name: "Dados".to_string(),
            headers: vec!["Regiao".to_string(), "Produto".to_string(), "Valor".to_string()],
            rows: vec![
                vec!["Norte".to_string(), "A".to_string(), "10".to_string()],
                vec!["Norte".to_string(), "B".to_string(), "20".to_string()],
                vec!["Sul".to_string(), "A".to_string(), "15".to_string()],
                vec!["Sul".to_string(), "B".to_string(), "5".to_string()],
            ],
        }],
    }
}

#[test]
fn pivot_sum_and_totals_are_correct() {
    let workbook = sample_workbook();
    let config = PivotConfig {
        row_field: 0,
        column_field: 1,
        value_field: 2,
        aggregation: PivotAggregation::Sum,
    };

    let pivot = compute_pivot(&workbook.sheets[0], &config);

    assert_eq!(pivot.row_labels, vec!["Norte", "Sul"]);
    assert_eq!(pivot.column_labels, vec!["A", "B"]);
    assert_eq!(pivot.cell_value(0, 0), Some(10.0));
    assert_eq!(pivot.cell_value(0, 1), Some(20.0));
    assert_eq!(pivot.cell_value(1, 0), Some(15.0));
    assert_eq!(pivot.cell_value(1, 1), Some(5.0));
    assert_eq!(pivot.row_totals, vec![30.0, 20.0]);
    assert_eq!(pivot.column_totals, vec![25.0, 25.0]);
    assert_eq!(pivot.grand_total, 50.0);
}

#[test]
fn pivot_aggregations_cover_core_statistics() {
    assert_eq!(aggregate_average(&[10.0, 20.0, 30.0]), Some(20.0));
    assert_eq!(aggregate_median(&[1.0, 3.0, 5.0]), Some(3.0));
    assert_eq!(aggregate_stddev(&[2.0, 4.0]), Some(1.0));
    assert_eq!(aggregate_count_distinct(&[1.0, 1.0, 2.0, 2.0]), 2.0);
    assert_eq!(aggregate_average(&[]), None);
}

#[test]
fn pivot_field_cycling_changes_active_field() {
    let mut app = App::default();

    app.handle_key_event(ratatui::crossterm::event::KeyCode::Char('2').into());
    let initial_row_field = app.pivot_row_field_name();

    app.handle_key_event(ratatui::crossterm::event::KeyCode::Right.into());
    assert_ne!(app.pivot_row_field_name(), initial_row_field);

    app.handle_key_event(ratatui::crossterm::event::KeyCode::Tab.into());
    let initial_col_field = app.pivot_col_field_name();
    app.handle_key_event(ratatui::crossterm::event::KeyCode::Right.into());
    assert_ne!(app.pivot_col_field_name(), initial_col_field);
}