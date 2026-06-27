use super::super::App;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    widgets::Widget,
};

#[test]
fn render() {
    let app = App::default();
    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 16));

    app.render(buf.area, &mut buf);

    let rendered = format!("{buf:?}");

    assert!(rendered.contains("Prévia da planilha"));
    assert!(rendered.contains("Workbook: amostra.xlsx"));
    assert!(rendered.contains("Sheet: Vendas"));
    assert!(rendered.contains("Numeric column: Receita"));
    assert!(rendered.contains("head(5)"));
    assert!(rendered.contains("idx | Mes | Receita | Pedidos | Desconto"));
    assert!(rendered.contains("0 | Jan"));
    assert!(rendered.contains("12000"));
    assert!(rendered.contains("120"));
    assert!(rendered.contains("300"));
}

#[test]
fn render_report_view() {
    let mut app = App::default();
    app.handle_key_event(ratatui::crossterm::event::KeyCode::Char('r').into());

    let mut buf = Buffer::empty(Rect::new(0, 0, 80, 16));
    app.render(buf.area, &mut buf);

    let rendered = format!("{buf:?}");

    assert!(rendered.contains("Relatorio KPI"));
    assert!(rendered.contains("Coluna atual: Receita"));
    assert!(rendered.contains("Total de linhas: 4"));
    assert!(rendered.contains("Soma: 55400"));
}
