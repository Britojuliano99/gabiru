use std::io;
use std::path::PathBuf;
use std::process::Command;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::{DefaultTerminal, Frame};

mod analytics;
mod ui;

use analytics::excel::{ImportedSheet, ImportedWorkbook, import_csv_auto, import_csv_preview, import_workbook, sample_workbook};
use analytics::kpi::{KpiDefinition, KpiResult, compute_selected, default_catalog};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewMode {
    Selection,
    Report,
    Loading,
}

#[derive(Debug)]
pub struct App {
    exit: bool,
    view_mode: ViewMode,
    workbook: ImportedWorkbook,
    active_sheet: usize,
    numeric_columns: Vec<usize>,
    active_numeric_column: usize,
    selected_kpi: usize,
    catalog: Vec<KpiDefinition>,
    enabled_kpis: Vec<bool>,
    results: Vec<KpiResult>,
    status: String,
    csv_source: Option<PathBuf>,
    pending_view_after_load: Option<ViewMode>,
    csv_preview_rows: usize,
}

impl Default for App {
    fn default() -> Self {
        let workbook = sample_workbook();
        let catalog = default_catalog();
        let enabled_kpis = vec![true, true, true, true, false, false];
        let mut app = Self {
            exit: false,
            view_mode: ViewMode::Selection,
            workbook,
            active_sheet: 0,
            numeric_columns: Vec::new(),
            active_numeric_column: 0,
            selected_kpi: 0,
            catalog,
            enabled_kpis,
            results: Vec::new(),
            status: "Usando dados de amostra. Pressione I para importar Excel ou CSV".to_string(),
            csv_source: None,
            pending_view_after_load: None,
            csv_preview_rows: 0,
        };
        app.refresh_numeric_columns();
        app.recompute_kpis();
        app
    }
}

impl App {
    /// runs the application's main loop until the user quits
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> io::Result<()> {
        while !self.exit {
            if self.view_mode == ViewMode::Loading {
                terminal.draw(|frame| self.draw(frame))?;
                self.finish_pending_csv_load();
                continue;
            }

            terminal.draw(|frame| self.draw(frame))?;
            self.handle_events()?;
        }
        Ok(())
    }

    fn draw(&self, frame: &mut Frame) {
        ui::components::render(frame, self);
    }

    fn handle_key_event(&mut self, key_event: KeyEvent) {
        match key_event.code {
            KeyCode::Char('q') => self.exit(),
            KeyCode::Char('r') => self.open_report(),
            KeyCode::Char('z') => self.open_selection(),
            KeyCode::Up if self.view_mode == ViewMode::Selection => self.previous_kpi(),
            KeyCode::Down if self.view_mode == ViewMode::Selection => self.next_kpi(),
            KeyCode::Char(' ') | KeyCode::Enter if self.view_mode == ViewMode::Selection => {
                self.toggle_selected_kpi()
            }
            KeyCode::Left if self.view_mode == ViewMode::Selection => self.previous_numeric_column(),
            KeyCode::Right if self.view_mode == ViewMode::Selection => self.next_numeric_column(),
            KeyCode::Char('i') => self.try_import_file(),
            _ => {}
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn current_sheet(&self) -> &ImportedSheet {
        &self.workbook.sheets[self.active_sheet]
    }

    fn refresh_numeric_columns(&mut self) {
        self.numeric_columns = self.current_sheet().numeric_column_indices();
        if self.numeric_columns.is_empty() {
            self.active_numeric_column = 0;
        } else if self.active_numeric_column >= self.numeric_columns.len() {
            self.active_numeric_column = self.numeric_columns.len() - 1;
        }
    }

    fn selected_numeric_column(&self) -> Option<usize> {
        self.numeric_columns.get(self.active_numeric_column).copied()
    }

    fn next_kpi(&mut self) {
        if self.catalog.is_empty() {
            return;
        }
        self.selected_kpi = (self.selected_kpi + 1).min(self.catalog.len() - 1);
    }

    fn previous_kpi(&mut self) {
        self.selected_kpi = self.selected_kpi.saturating_sub(1);
    }

    fn toggle_selected_kpi(&mut self) {
        if let Some(enabled) = self.enabled_kpis.get_mut(self.selected_kpi) {
            *enabled = !*enabled;
            self.recompute_kpis();
        }
    }

    fn next_numeric_column(&mut self) {
        if self.numeric_columns.is_empty() {
            return;
        }
        self.active_numeric_column = (self.active_numeric_column + 1).min(self.numeric_columns.len() - 1);
        self.recompute_kpis();
    }

    fn previous_numeric_column(&mut self) {
        if self.numeric_columns.is_empty() {
            return;
        }
        self.active_numeric_column = self.active_numeric_column.saturating_sub(1);
        self.recompute_kpis();
    }

    fn recompute_kpis(&mut self) {
        self.results = compute_selected(
            self.current_sheet(),
            self.selected_numeric_column(),
            &self.catalog,
            &self.enabled_kpis,
        );
    }

    fn open_report(&mut self) {
        if self.csv_source.is_some() {
            self.view_mode = ViewMode::Loading;
            self.pending_view_after_load = Some(ViewMode::Report);
            self.status = "Carregando a tabela completa do CSV...".to_string();
            return;
        }

        self.recompute_kpis();
        self.view_mode = ViewMode::Report;
        self.status = format!(
            "Relatorio KPI gerado para {} / {}. Pressione z para voltar",
            self.sheet_name(),
            self.numeric_column_name()
        );
    }

    fn open_selection(&mut self) {
        self.view_mode = ViewMode::Selection;
        self.status = if self.has_csv_source() {
            format!(
                "Prévia CSV mantida com {} linhas. Pressione R para gerar o relatorio completo",
                self.csv_preview_rows
            )
        } else {
            "Selecionando KPIs. Pressione R para gerar o relatorio".to_string()
        };
    }

    fn try_import_file(&mut self) {
        let Some(path) = pick_excel_file() else {
            self.status = "Importação cancelada".to_string();
            return;
        };

        match import_selected_file(&path) {
            Ok(outcome) => {
                self.workbook = outcome.workbook;
                self.active_sheet = 0;
                self.csv_source = outcome.csv_source;
                self.csv_preview_rows = outcome.csv_preview_rows;
                self.pending_view_after_load = None;
                self.refresh_numeric_columns();
                self.view_mode = ViewMode::Selection;
                if self.csv_source.is_some() {
                    self.results.clear();
                    self.status = format!(
                        "Prévia CSV carregada com {} linhas. Pressione R para carregar a tabela completa",
                        self.csv_preview_rows
                    );
                } else {
                    self.recompute_kpis();
                    self.status = format!("Arquivo importado com sucesso: {}", path.display());
                }
            }
            Err(err) => {
                self.status = format!("Falha ao importar {}: {err}", path.display());
            }
        }
    }

    fn handle_events(&mut self) -> io::Result<()> {
        match event::read()? {
            // it's important to check that the event is a key press event as
            // crossterm also emits key release and repeat events on Windows.
            Event::Key(key_event) if key_event.kind == KeyEventKind::Press => {
                self.handle_key_event(key_event)
            }
            _ => {}
        };
        Ok(())
    }

    pub(crate) fn status(&self) -> &str {
        &self.status
    }

    pub(crate) fn should_exit(&self) -> bool {
        self.exit
    }

    pub(crate) fn is_report_view(&self) -> bool {
        self.view_mode == ViewMode::Report
    }

    pub(crate) fn is_loading_view(&self) -> bool {
        self.view_mode == ViewMode::Loading
    }

    pub(crate) fn has_csv_source(&self) -> bool {
        self.csv_source.is_some()
    }

    pub(crate) fn csv_preview_rows(&self) -> usize {
        self.csv_preview_rows
    }

    pub(crate) fn workbook_name(&self) -> &str {
        &self.workbook.file_name
    }

    pub(crate) fn sheet_name(&self) -> &str {
        &self.current_sheet().name
    }

    pub(crate) fn row_count(&self) -> usize {
        self.current_sheet().row_count()
    }

    pub(crate) fn column_count(&self) -> usize {
        self.current_sheet().column_count()
    }

    pub(crate) fn preview_table_lines(&self, limit: usize) -> Vec<String> {
        let sheet = self.current_sheet();
        let column_count = sheet.column_count();
        if column_count == 0 {
            return vec!["Sem colunas detectadas".to_string()];
        }

        let preview_rows = sheet.rows.iter().take(limit).collect::<Vec<_>>();
        let mut widths = vec![0usize; column_count];
        let index_width = preview_rows.len().saturating_sub(1).max(1).to_string().len().max(2);
        widths[0] = widths[0].max(index_width);

        for column in 0..column_count {
            widths[column] = widths[column].max(sheet.column_name(column).len());
        }

        for row in &preview_rows {
            for column in 0..column_count {
                if let Some(cell) = row.get(column) {
                    widths[column] = widths[column].max(cell.len());
                }
            }
        }

        let mut lines = Vec::new();
            lines.push(format_dataframe_row("idx", &sheet.headers, &widths, index_width));

        for (row_index, row) in preview_rows.iter().enumerate() {
                lines.push(format_dataframe_row(&row_index.to_string(), row, &widths, index_width));
        }

        lines
    }

    pub(crate) fn numeric_column_name(&self) -> String {
        self.selected_numeric_column()
            .map(|index| self.current_sheet().column_name(index))
            .unwrap_or_else(|| "N/A".to_string())
    }

    pub(crate) fn selected_kpi_index(&self) -> usize {
        self.selected_kpi
    }

    pub(crate) fn kpi_catalog(&self) -> &[KpiDefinition] {
        &self.catalog
    }

    pub(crate) fn kpi_enabled(&self, index: usize) -> bool {
        self.enabled_kpis.get(index).copied().unwrap_or(false)
    }

    pub(crate) fn kpi_results(&self) -> &[KpiResult] {
        &self.results
    }

    fn finish_pending_csv_load(&mut self) {
        let Some(path) = self.csv_source.clone() else {
            self.view_mode = self.pending_view_after_load.take().unwrap_or(ViewMode::Selection);
            return;
        };

        match import_csv_auto(&path) {
            Ok(workbook) => {
                let report_sheet = workbook
                    .sheets
                    .get(self.active_sheet)
                    .cloned()
                    .unwrap_or_else(|| workbook.sheets[0].clone());
                self.results = compute_selected(
                    &report_sheet,
                    self.selected_numeric_column(),
                    &self.catalog,
                    &self.enabled_kpis,
                );
                self.view_mode = self.pending_view_after_load.take().unwrap_or(ViewMode::Report);
                self.status = format!(
                    "Tabela completa carregada: {}. Relatorio pronto (prévia mantida em memória)",
                    path.display()
                );
            }
            Err(err) => {
                self.view_mode = ViewMode::Selection;
                self.pending_view_after_load = None;
                self.status = format!("Falha ao carregar a tabela completa {}: {err}", path.display());
            }
        }
    }
}

struct ImportOutcome {
    workbook: ImportedWorkbook,
    csv_source: Option<PathBuf>,
    csv_preview_rows: usize,
}

fn pick_excel_file() -> Option<PathBuf> {
    zenity_selection(&[
        "--file-selection",
        "--title=Selecione uma planilha Excel ou CSV",
        "--file-filter=Planilhas compatíveis | *.xlsx *.xls *.xlsm *.csv",
    ])
}

fn import_selected_file(path: &PathBuf) -> Result<ImportOutcome, String> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase());

    match extension.as_deref() {
        Some("csv") => {
            let (workbook, truncated) = import_csv_preview(path, 1000)?;
            Ok(ImportOutcome {
                workbook,
                csv_source: if truncated { Some(path.clone()) } else { None },
                csv_preview_rows: if truncated { 1000 } else { 0 },
            })
        }
        Some("xlsx") | Some("xls") | Some("xlsm") => Ok(ImportOutcome {
            workbook: import_workbook(path)?,
            csv_source: None,
            csv_preview_rows: 0,
        }),
        _ => match import_workbook(path) {
            Ok(workbook) => Ok(ImportOutcome {
                workbook,
                csv_source: None,
                csv_preview_rows: 0,
            }),
            Err(_) => {
                let (workbook, truncated) = import_csv_preview(path, 1000)?;
                Ok(ImportOutcome {
                    workbook,
                    csv_source: if truncated { Some(path.clone()) } else { None },
                    csv_preview_rows: if truncated { 1000 } else { 0 },
                })
            }
        },
    }
}

fn zenity_selection(args: &[&str]) -> Option<PathBuf> {
    let output = Command::new("zenity").args(args).output().ok()?;

    if !output.status.success() {
        return None;
    }

    let selected = String::from_utf8(output.stdout).ok()?;
    let selected = selected.trim();
    if selected.is_empty() {
        None
    } else {
        Some(PathBuf::from(selected))
    }
}

fn format_dataframe_row(index: &str, row: &[String], widths: &[usize], index_width: usize) -> String {
    let mut parts = Vec::with_capacity(widths.len() + 1);
    let _ = index_width;
    parts.push(index.to_string());

    for column in 0..widths.len() {
        let cell = row.get(column).map(|value| value.as_str()).unwrap_or("");
        parts.push(format!("{cell:<width$}", width = widths[column]));
    }

    parts.join(" | ")
}

#[cfg(test)]
mod tests;

fn main() -> io::Result<()> {
    ratatui::run(|terminal| App::default().run(terminal))
}