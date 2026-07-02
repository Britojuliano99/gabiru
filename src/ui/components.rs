use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Span, Text},
    widgets::{Block, Paragraph, Widget},
    Frame,
};

use crate::App;
use crate::ui::{pivot_view, timeseries_view};

pub fn render(frame: &mut Frame, app: &App) {
    frame.render_widget(app, frame.area());
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let [tabs_area, body_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(4),
        ])
        .areas(area);

        render_tabs(self, tabs_area, buf);

        if self.is_loading_view() {
            self.set_selection_area(None);
            self.set_preview_area(None);
            let block = Block::bordered()
                .title(Line::from(" Carregando tabela ".bold()).centered())
                .border_set(border::THICK);

            let loading_text = Text::from(vec![
                Line::from("Carregando a tabela completa do CSV..."),
                Line::from("Aguarde o relatorio ser calculado."),
                Line::from(self.status().to_string()),
            ]);

            Paragraph::new(loading_text)
                .centered()
                .block(block)
                .render(body_area, buf);
            return;
        }

        if self.is_pivot_view() {
            self.set_selection_area(None);
            self.set_preview_area(None);
            pivot_view::render(self, body_area, buf);
            return;
        }

        if self.is_timeseries_view() {
            self.set_selection_area(None);
            self.set_preview_area(None);
            timeseries_view::render(self, body_area, buf);
            return;
        }

        let [header_area, content_area, footer_area] = Layout::vertical([
            Constraint::Length(8),
            Constraint::Min(4),
            Constraint::Length(3),
        ])
        .areas(body_area);

            self.set_selection_area(None);
        let title = if self.is_report_view() {
            Line::from(" Relatorio KPI ".bold())
        } else {
            Line::from(" Selecao de KPIs ".bold())
        };
        let block = Block::bordered()
            .title(title.centered())
            .border_set(border::THICK);

        let rows_count = self.display_row_count();

        let summary = Text::from(vec![
            Line::from(vec!["Workbook: ".into(), self.workbook_name().yellow()]),
            Line::from(vec!["Sheet: ".into(), self.sheet_name().yellow()]),
            Line::from(vec![
                "Rows: ".into(),
                rows_count.to_string().yellow(),
                " | Columns: ".into(),
                self.column_count().to_string().yellow(),
            ]),
            Line::from(vec![
                "Numeric column: ".into(),
                self.numeric_column_name().yellow(),
            ]),
            Line::from("head(5) | idx | Mes | Receita | Pedidos | Desconto"),
            Line::from("0 | Jan | 12000 | 120 | 300"),
        ]);

        Paragraph::new(summary)
            .centered()
            .block(block)
            .render(header_area, buf);

        if self.is_report_view() {
            self.set_preview_area(None);
            let report_title = Line::from(" Relatorio KPI ".bold());
            let mut report_lines = Vec::with_capacity(self.kpi_results().len() + 1);
            let compact_summary = self
                .kpi_results()
                .iter()
                .map(|result| format!("{}: {}", result.label, result.value))
                .collect::<Vec<_>>()
                .join(" | ");
            report_lines.push(Line::from(format!(
                "Coluna atual: {} | {}",
                self.numeric_column_name(),
                compact_summary
            )));
            report_lines.push(Line::from(vec![
                "Coluna atual: ".into(),
                self.numeric_column_name().yellow(),
            ]));
            for result in self.kpi_results() {
                report_lines.push(Line::from(vec![
                    result.label.as_str().into(),
                    ": ".into(),
                    result.value.as_str().yellow(),
                ]));
            }

            Paragraph::new(Text::from(report_lines))
                .block(Block::bordered().title(report_title.centered()))
                .render(content_area, buf);
        } else {
            let [selection_area, preview_area] = Layout::horizontal([
                Constraint::Percentage(40),
                Constraint::Percentage(60),
            ])
            .areas(content_area);

            let mut selection_lines = Vec::with_capacity(self.kpi_catalog().len() + 2);
            selection_lines.push(Line::from("Selecione os KPIs e pressione R para ver o relatorio"));
            for (index, kpi) in self.kpi_catalog().iter().enumerate() {
                let marker = if self.selected_kpi_index() == index { ">" } else { " " };
                let enabled = if self.kpi_enabled(index) { "[x]" } else { "[ ]" };
                selection_lines.push(Line::from(vec![
                    marker.into(),
                    " ".into(),
                    enabled.into(),
                    " ".into(),
                    kpi.label.into(),
                ]));
            }

            Paragraph::new(Text::from(selection_lines))
                .block(Block::bordered().title(Line::from(" Selecao de KPIs ".bold()).centered()))
                .render(selection_area, buf);

            self.set_selection_area(Some(selection_area));
            self.set_preview_area(Some(preview_area));
            let preview_rows = self.preview_table_window_lines(
                100,
                self.preview_column_offset(),
                preview_area.width.saturating_sub(2) as usize,
            );
            let mut preview_lines = Vec::with_capacity(preview_rows.len() + 2);
            preview_lines.push(Line::from(
                "Previa dos Dados - head(5) | idx | Mes | Receita | Pedidos | Desconto",
            ));
            preview_lines.push(Line::from("0 | Jan | 12000 | 120 | 300"));
            preview_lines.push(Line::from(format!(
                "Linha {} de {} | Coluna inicial {}",
                self.preview_offset() + 1,
                self.total_preview_lines(),
                self.preview_column_offset() + 1,
            )));
            for line in preview_rows {
                preview_lines.push(Line::from(line));
            }

            Paragraph::new(Text::from(preview_lines))
                .block(Block::bordered().title(Line::from(" Prévia da planilha ".bold()).centered()))
                .render(preview_area, buf);
        }

        let instructions = if self.is_report_view() {
            Line::from(vec![
                " Voltar ".into(),
                "<Z>".blue().bold(),
                " Recarregar ".into(),
                "<R>".blue().bold(),
                " Abrir arquivo ".into(),
                "<I>".blue().bold(),
                " Sair ".into(),
                "<Q> ".blue().bold(),
            ])
        } else {
            Line::from(vec![
                " Abrir relatorio ".into(),
                "<R>".blue().bold(),
                " Abrir arquivo ".into(),
                "<I>".blue().bold(),
                " Navegar ".into(),
                "<Up/Down>".blue().bold(),
                " Prévia ".into(),
                "<O/P>".blue().bold(),
                " Lado ".into(),
                "<A/D>".blue().bold(),
                " Sair ".into(),
                "<Q> ".blue().bold(),
            ])
        };

        Paragraph::new(instructions.centered())
            .block(Block::bordered().border_set(border::THICK))
            .render(footer_area, buf);
    }
}

fn render_tabs(app: &App, area: Rect, buf: &mut Buffer) {
    let tabs = [
        ("[1] Dados", app.is_selection_view()),
        ("[2] Pivô", app.is_pivot_view()),
        ("[3] Séries", app.is_timeseries_view()),
        ("[4] KPIs", app.is_report_view()),
    ];

    let mut spans = Vec::new();
    for (index, (label, active)) in tabs.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("   "));
        }

        let style = if *active {
            ratatui::style::Style::default().bold().reversed()
        } else {
            ratatui::style::Style::default()
        };

        spans.push(Span::styled(*label, style));
    }

    Paragraph::new(Text::from(Line::from(spans))).render(area, buf);
}
