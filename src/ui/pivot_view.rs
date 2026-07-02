use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Widget},
};

use crate::App;

/// Renders the pivot table tab into the given content area.
pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    let [config_area, table_area] = Layout::vertical([
        Constraint::Length(5),
        Constraint::Min(4),
    ])
    .areas(area);

    render_config_panel(app, config_area, buf);
    render_pivot_grid(app, table_area, buf);
}

fn render_config_panel(app: &App, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered()
        .title(Line::from(" Configuração do Pivô ".bold()).centered())
        .border_set(border::ROUNDED);

    let row_field_name = app.pivot_row_field_name();
    let col_field_name = app.pivot_col_field_name();
    let val_field_name = app.pivot_value_field_name();
    let agg_name = app.pivot_aggregation_name();

    let config_lines = vec![
        Line::from(vec![
            Span::raw("Linha: "),
            Span::styled(&row_field_name, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("  │  Coluna: "),
            Span::styled(&col_field_name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw("  │  Valor: "),
            Span::styled(&val_field_name, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("  │  Agregação: "),
            Span::styled(&agg_name, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::raw("← → Linha  │  Shift+← → Coluna  │  "),
            Span::styled("Tab", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(" Valor  │  "),
            Span::styled("Espaço", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(" Agregação  │  "),
            Span::styled("Enter", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(" Calcular"),
        ]),
    ];

    Paragraph::new(Text::from(config_lines))
        .block(block)
        .render(area, buf);
}

fn render_pivot_grid(app: &App, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered()
        .title(Line::from(" Tabela Pivô ".bold()).centered())
        .border_set(border::ROUNDED);

    let pivot = app.pivot_result();

    let Some(pivot) = pivot else {
        let empty_msg = Text::from(vec![
            Line::from(""),
            Line::from("Nenhuma tabela pivô calculada."),
            Line::from("Configure os campos e pressione Enter para calcular."),
        ]);
        Paragraph::new(empty_msg)
            .centered()
            .block(block)
            .render(area, buf);
        return;
    };

    if pivot.row_labels.is_empty() || pivot.column_labels.is_empty() {
        let empty_msg = Text::from(vec![
            Line::from(""),
            Line::from("Tabela pivô vazia — sem dados para os campos selecionados."),
        ]);
        Paragraph::new(empty_msg)
            .centered()
            .block(block)
            .render(area, buf);
        return;
    }

    let inner_area = block.inner(area);
    block.render(area, buf);

    let scroll_offset = app.pivot_scroll_offset();
    let col_scroll = app.pivot_column_scroll();

    // Calculate column widths
    let row_label_width = pivot
        .row_labels
        .iter()
        .map(|l| l.len())
        .max()
        .unwrap_or(4)
        .max(5); // min "Total" width

    let visible_cols: Vec<usize> = (col_scroll..pivot.column_labels.len()).collect();
    let total_label = "Total";

    // Build header line
    let mut header_spans = vec![
        Span::styled(
            format!("{:<width$}", "", width = row_label_width),
            Style::default().add_modifier(Modifier::BOLD),
        ),
    ];

    let col_value_width = 12;
    let max_visible_cols = ((inner_area.width as usize).saturating_sub(row_label_width + 3 + col_value_width + 3))
        / (col_value_width + 3);
    let end_col = visible_cols.len().min(max_visible_cols.max(1));

    for &col_idx in &visible_cols[..end_col] {
        let label = &pivot.column_labels[col_idx];
        header_spans.push(Span::raw(" │ "));
        header_spans.push(Span::styled(
            format!("{:>width$}", truncate_str(label, col_value_width), width = col_value_width),
            Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
        ));
    }
    header_spans.push(Span::raw(" │ "));
    header_spans.push(Span::styled(
        format!("{:>width$}", total_label, width = col_value_width),
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    ));

    let mut lines = Vec::new();
    lines.push(Line::from(header_spans));

    // Separator
    let sep_width = inner_area.width as usize;
    lines.push(Line::from("─".repeat(sep_width)));

    // Data rows
    let visible_rows = pivot.row_labels.len().saturating_sub(scroll_offset);
    let max_rows = (inner_area.height as usize).saturating_sub(4); // header + sep + totals row + sep
    let display_rows = visible_rows.min(max_rows);

    for row_offset in 0..display_rows {
        let row_idx = scroll_offset + row_offset;
        let label = &pivot.row_labels[row_idx];

        let mut row_spans = vec![Span::styled(
            format!("{:<width$}", truncate_str(label, row_label_width), width = row_label_width),
            Style::default().add_modifier(Modifier::BOLD),
        )];

        for &col_idx in &visible_cols[..end_col] {
            let cell_val = pivot.cell_value(row_idx, col_idx);
            let formatted = match cell_val {
                Some(v) => format_value(v),
                None => "—".to_string(),
            };
            row_spans.push(Span::raw(" │ "));
            row_spans.push(Span::raw(format!(
                "{:>width$}",
                formatted,
                width = col_value_width
            )));
        }

        // Row total
        let row_total = pivot.row_totals.get(row_idx).copied().unwrap_or(0.0);
        row_spans.push(Span::raw(" │ "));
        row_spans.push(Span::styled(
            format!("{:>width$}", format_value(row_total), width = col_value_width),
            Style::default().fg(Color::Yellow),
        ));

        lines.push(Line::from(row_spans));
    }

    // Bottom separator
    lines.push(Line::from("─".repeat(sep_width)));

    // Column totals row
    let mut totals_spans = vec![Span::styled(
        format!("{:<width$}", "Total", width = row_label_width),
        Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
    )];

    for &col_idx in &visible_cols[..end_col] {
        let col_total = pivot.column_totals.get(col_idx).copied().unwrap_or(0.0);
        totals_spans.push(Span::raw(" │ "));
        totals_spans.push(Span::styled(
            format!("{:>width$}", format_value(col_total), width = col_value_width),
            Style::default().fg(Color::Yellow),
        ));
    }

    totals_spans.push(Span::raw(" │ "));
    totals_spans.push(Span::styled(
        format!("{:>width$}", format_value(pivot.grand_total), width = col_value_width),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));

    lines.push(Line::from(totals_spans));

    // Scroll indicator
    if scroll_offset > 0 || scroll_offset + display_rows < pivot.row_labels.len() {
        lines.push(Line::from(vec![
            Span::raw(format!(
                "Linhas {}-{} de {} │ Colunas {}-{} de {}",
                scroll_offset + 1,
                scroll_offset + display_rows,
                pivot.row_labels.len(),
                col_scroll + 1,
                col_scroll + end_col,
                pivot.column_labels.len(),
            )),
        ]));
    }

    Paragraph::new(Text::from(lines)).render(inner_area, buf);
}

fn format_value(value: f64) -> String {
    if (value.fract() - 0.0).abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len.saturating_sub(1)])
    }
}
