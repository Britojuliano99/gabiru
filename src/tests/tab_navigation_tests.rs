use super::super::App;
use ratatui::crossterm::event::KeyCode;

#[test]
fn number_keys_switch_tabs_and_preserve_state() {
    let mut app = App::default();

    app.handle_key_event(KeyCode::Right.into());
    assert_eq!(app.numeric_column_name(), "Pedidos");

    app.handle_key_event(KeyCode::Char('2').into());
    assert!(app.is_pivot_view());

    app.handle_key_event(KeyCode::Char('3').into());
    assert!(app.is_timeseries_view());

    app.handle_key_event(KeyCode::Char('4').into());
    assert!(app.is_report_view());

    app.handle_key_event(KeyCode::Char('1').into());
    assert!(app.is_selection_view());
    assert_eq!(app.numeric_column_name(), "Pedidos");
}

#[test]
fn global_keys_still_work_in_other_tabs() {
    let mut app = App::default();

    app.handle_key_event(KeyCode::Char('2').into());
    app.handle_key_event(KeyCode::Char('i').into());
    assert!(!app.should_exit());

    app.handle_key_event(KeyCode::Char('q').into());
    assert!(app.should_exit());
}