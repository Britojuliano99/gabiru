use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Layout, Rect},
    style::Stylize,
    symbols::border,
    text::{Line, Text},
    widgets::{Block, Paragraph, Widget},
    Frame,
};

use crate::App;

pub fn render(frame: &mut Frame, app: &App) {
    frame.render_widget(app, frame.area());
}

impl Widget for &App {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if self.is_loading_view() {
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
                .render(area, buf);
            return;
        }

        let [header_area, content_area] = Layout::vertical([
            Constraint::Length(8),
            Constraint::Min(4),
        ])
        .areas(area);

        let (title, instructions) = if self.is_report_view() {
            (
                Line::from(" Relatorio KPI ".bold()),
                Line::from(vec![
                    " Voltar ".into(),
                    "<Z>".blue().bold(),
                    " Recarregar ".into(),
                    "<R>".blue().bold(),
                    " Abrir arquivo ".into(),
                    "<I>".blue().bold(),
                    " Sair ".into(),
                    "<Q> ".blue().bold(),
                ]),
            )
        } else {
            (
                Line::from(" Selecao de KPIs ".bold()),
                Line::from(vec![
                    " Abrir relatorio ".into(),
                    "<R>".blue().bold(),
                    " Abrir arquivo ".into(),
                    "<I>".blue().bold(),
                    " Navegar ".into(),
                    "<Up/Down>".blue().bold(),
                    " Sair ".into(),
                    "<Q> ".blue().bold(),
                ]),
            )
        };
        let block = Block::bordered()
            .title(title.centered())
            .title_bottom(instructions.centered())
            .border_set(border::THICK);

        let selected_kpi = self
            .kpi_catalog()
            .get(self.selected_kpi_index())
            .map(|kpi| kpi.label)
            .unwrap_or("N/A");
        let results_count = self.kpi_results().len();
        let csv_note = if self.has_csv_source() {
            format!(" | Prévia CSV: {} linhas", self.csv_preview_rows())
        } else {
            String::new()
        };

        let summary = Text::from(vec![
            Line::from(vec!["Workbook: ".into(), self.workbook_name().yellow()]),
            Line::from(vec!["Sheet: ".into(), self.sheet_name().yellow()]),
            Line::from(vec![
                "Rows: ".into(),
                self.row_count().to_string().yellow(),
                " | Columns: ".into(),
                self.column_count().to_string().yellow(),
            ]),
            Line::from(vec![
                "Numeric column: ".into(),
                self.numeric_column_name().yellow(),
            ]),
            Line::from(vec![
                "Selected KPI: ".into(),
                selected_kpi.yellow(),
            ]),
            Line::from(vec![
                "KPIs active: ".into(),
                results_count.to_string().yellow(),
                csv_note.into(),
            ]),
            Line::from(vec![
                "Status: ".into(),
                self.status().yellow(),
            ]),
        ]);

        Paragraph::new(summary)
            .centered()
            .block(block)
            .render(header_area, buf);

        if self.is_report_view() {
            let report_title = Line::from(" Relatorio KPI ".bold());
            let mut report_lines = Vec::with_capacity(self.kpi_results().len() + 1);
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

            let mut preview_lines = Vec::with_capacity(self.preview_table_lines(5).len() + 1);
            preview_lines.push(Line::from("Previa dos Dados - 5 Primeiras Linhas"));
            for line in self.preview_table_lines(5) {
                preview_lines.push(Line::from(line));
            }

            Paragraph::new(Text::from(preview_lines))
                .block(Block::bordered().title(Line::from(" Prévia da planilha ".bold()).centered()))
                .render(preview_area, buf);
        }
    }
}
