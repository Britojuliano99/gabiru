use super::super::analytics::excel::{ImportedSheet, ImportedWorkbook};
use super::super::analytics::timeseries::{
    detect_date_column, extract_time_series, growth_rate, linear_trend, moving_average,
    period_summary,
};

fn sample_sheet() -> ImportedSheet {
    ImportedSheet {
        name: "Série".to_string(),
        headers: vec!["Data".to_string(), "Valor".to_string()],
        rows: vec![
            vec!["2024-03-01".to_string(), "30".to_string()],
            vec!["2024-01-01".to_string(), "10".to_string()],
            vec!["2024-02-01".to_string(), "20".to_string()],
            vec!["2024-02-15".to_string(), "0".to_string()],
        ],
    }
}

#[test]
fn detects_and_sorts_time_series_dates() {
    let sheet = sample_sheet();
    assert_eq!(detect_date_column(&sheet), Some(0));

    let series = extract_time_series(&sheet, 0, 1).unwrap();
    assert_eq!(series.points.len(), 4);
    assert_eq!(series.points[0].date.to_string(), "2024-01-01");
    assert_eq!(series.points[1].date.to_string(), "2024-02-01");
}

#[test]
fn moving_average_growth_and_trend_are_computed() {
    let sheet = sample_sheet();
    let series = extract_time_series(&sheet, 0, 1).unwrap();

    let moving = moving_average(&series, 2);
    assert_eq!(moving.len(), 3);
    assert_eq!(moving[0].value, 15.0);
    assert_eq!(moving[1].value, 10.0);

    let growth = growth_rate(&series);
    assert_eq!(growth.len(), 4);
    assert_eq!(growth[0].growth_pct, None);

    let trend = linear_trend(&series);
    assert!(trend.slope > 0.0);
    assert_eq!(trend.direction.to_string(), "↑ Crescente");
}

#[test]
fn period_summary_groups_by_month_and_empty_series_fails() {
    let sheet = sample_sheet();
    let series = extract_time_series(&sheet, 0, 1).unwrap();
    let summary = period_summary(&series);

    assert_eq!(summary.len(), 3);
    assert_eq!(summary[0].period_label, "2024-01");
    assert_eq!(summary[1].period_label, "2024-02");
    assert_eq!(summary[2].period_label, "2024-03");

    let bad_sheet = ImportedWorkbook {
        file_name: "bad.xlsx".to_string(),
        sheets: vec![ImportedSheet {
            name: "Dados".to_string(),
            headers: vec!["A".to_string(), "B".to_string()],
            rows: vec![vec!["x".to_string(), "1".to_string()]],
        }],
    };

    assert!(extract_time_series(&bad_sheet.sheets[0], 0, 1).is_err());
}