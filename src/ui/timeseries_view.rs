use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    symbols::{self, border},
    text::{Line, Span, Text},
    widgets::{Axis, Block, Chart, Dataset, GraphType, Paragraph, Widget},
};

use crate::App;

/// Renders the time series analysis tab into the given content area.
pub fn render(app: &App, area: Rect, buf: &mut Buffer) {
    let [config_area, main_area] = Layout::vertical([
        Constraint::Length(4),
        Constraint::Min(6),
    ])
    .areas(area);

    render_config_panel(app, config_area, buf);

    if app.ts_has_data() {
        let active_view = app.ts_active_view_name();
        if active_view == "Resumo por Período" {
            // Period summary is table-only, no chart
            render_results_panel(app, main_area, buf);
        } else {
            let [chart_area, results_area] = Layout::vertical([
                Constraint::Percentage(55),
                Constraint::Percentage(45),
            ])
            .areas(main_area);

            render_chart(app, chart_area, buf);
            render_results_panel(app, results_area, buf);
        }
    } else {
        render_empty_state(app, main_area, buf);
    }
}

fn render_config_panel(app: &App, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered()
        .title(Line::from(" Configuração de Séries Temporais ".bold()).centered())
        .border_set(border::ROUNDED);

    let date_col = app.ts_date_column_name();
    let value_col = app.ts_value_column_name();
    let view_name = app.ts_active_view_name();
    let window = app.ts_moving_avg_window();

    let config_lines = vec![
        Line::from(vec![
            Span::raw("Data: "),
            Span::styled(&date_col, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw("  │  Valor: "),
            Span::styled(&value_col, Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw("  │  Análise: "),
            Span::styled(&view_name, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(format!("  │  Janela MA: {}", window)),
        ]),
        Line::from(vec![
            Span::styled("←→", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(" Análise  "),
            Span::styled("↑↓", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(" Col. valor  "),
            Span::styled("+/-", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(" Janela MA  "),
            Span::styled("Tab", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(" Col. data  "),
            Span::styled("Enter", Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)),
            Span::raw(" Calcular"),
        ]),
    ];

    Paragraph::new(Text::from(config_lines))
        .block(block)
        .render(area, buf);
}

fn render_chart(app: &App, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered()
        .title(Line::from(" Gráfico ".bold()).centered())
        .border_set(border::ROUNDED);

    let chart_data = app.ts_chart_points();

    if chart_data.is_empty() {
        Paragraph::new("Sem dados para exibir no gráfico.")
            .centered()
            .block(block)
            .render(area, buf);
        return;
    }

    let (x_min, x_max, y_min, y_max) = compute_bounds(&chart_data);

    let dataset_data: Vec<(f64, f64)> = chart_data.iter().map(|&(x, y)| (x, y)).collect();

    let mut datasets = vec![
        Dataset::default()
            .name("Dados")
            .marker(symbols::Marker::Braille)
            .graph_type(GraphType::Line)
            .style(Style::default().fg(Color::Cyan))
            .data(&dataset_data),
    ];

    let overlay_data = app.ts_overlay_points();
    if !overlay_data.is_empty() {
        datasets.push(
            Dataset::default()
                .name("Overlay")
                .marker(symbols::Marker::Braille)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(Color::Yellow))
                .data(&overlay_data),
        );
    }

    let x_label_start = app.ts_chart_x_label_start();
    let x_label_end = app.ts_chart_x_label_end();

    let x_axis = Axis::default()
        .title("Período")
        .style(Style::default().fg(Color::Gray))
        .bounds([x_min, x_max])
        .labels([x_label_start, x_label_end]);

    let y_axis = Axis::default()
        .title("Valor")
        .style(Style::default().fg(Color::Gray))
        .bounds([y_min, y_max])
        .labels([
            format!("{:.0}", y_min),
            format!("{:.0}", (y_min + y_max) / 2.0),
            format!("{:.0}", y_max),
        ]);

    let chart = Chart::new(datasets)
        .block(block)
        .x_axis(x_axis)
        .y_axis(y_axis);

    chart.render(area, buf);
}

fn render_results_panel(app: &App, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered()
        .title(Line::from(" Resultados ".bold()).centered())
        .border_set(border::ROUNDED);

    let result_lines = app.ts_result_lines();

    if result_lines.is_empty() {
        Paragraph::new("Pressione Enter para calcular a análise.")
            .centered()
            .block(block)
            .render(area, buf);
        return;
    }

    let lines: Vec<Line> = result_lines
        .iter()
        .map(|line| {
            if line.contains(':') {
                let parts: Vec<&str> = line.splitn(2, ':').collect();
                Line::from(vec![
                    Span::styled(parts[0], Style::default().add_modifier(Modifier::BOLD)),
                    Span::raw(":"),
                    Span::styled(
                        parts.get(1).unwrap_or(&"").to_string(),
                        Style::default().fg(Color::Yellow),
                    ),
                ])
            } else if line.starts_with("───") {
                Line::from(Span::styled(
                    line.clone(),
                    Style::default().fg(Color::DarkGray),
                ))
            } else {
                Line::from(line.clone())
            }
        })
        .collect();

    Paragraph::new(Text::from(lines))
        .block(block)
        .render(area, buf);
}

fn render_empty_state(_app: &App, area: Rect, buf: &mut Buffer) {
    let block = Block::bordered()
        .title(Line::from(" Séries Temporais ".bold()).centered())
        .border_set(border::ROUNDED);

    let msg = Text::from(vec![
        Line::from(""),
        Line::from("Nenhuma série temporal configurada."),
        Line::from(""),
        Line::from("Selecione uma coluna de data e uma coluna de valor,"),
        Line::from("depois pressione Enter para calcular."),
        Line::from(""),
        Line::from("A coluna de data será detectada automaticamente se possível."),
    ]);

    Paragraph::new(msg)
        .centered()
        .block(block)
        .render(area, buf);
}

fn compute_bounds(data: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    if data.is_empty() {
        return (0.0, 1.0, 0.0, 1.0);
    }

    let x_min = data.iter().map(|(x, _)| *x).fold(f64::INFINITY, f64::min);
    let x_max = data.iter().map(|(x, _)| *x).fold(f64::NEG_INFINITY, f64::max);
    let y_min = data.iter().map(|(_, y)| *y).fold(f64::INFINITY, f64::min);
    let y_max = data.iter().map(|(_, y)| *y).fold(f64::NEG_INFINITY, f64::max);

    // Add some padding to y axis
    let y_range = y_max - y_min;
    let padding = if y_range.abs() < f64::EPSILON { 1.0 } else { y_range * 0.1 };

    let x_range = x_max - x_min;
    let x_padding = if x_range.abs() < f64::EPSILON { 1.0 } else { 0.0 };

    (
        x_min - x_padding,
        x_max + x_padding,
        y_min - padding,
        y_max + padding,
    )
}
