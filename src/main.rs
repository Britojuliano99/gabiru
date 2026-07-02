use std::cell::Cell;
use std::io;
use std::path::PathBuf;
use std::process::Command;

use chrono::{Datelike, NaiveDate};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
    MouseEvent, MouseEventKind,
};
use ratatui::{layout::Rect, DefaultTerminal, Frame};

mod analytics;
mod ui;

use analytics::excel::{
    ImportedSheet, ImportedWorkbook, import_csv_auto,
    import_csv_preview_with_total_rows, import_workbook, sample_workbook,
};
use analytics::pivot::{PivotAggregation, PivotConfig, PivotTable, compute_pivot};
use analytics::kpi::{KpiDefinition, KpiResult, compute_selected, default_catalog};
use analytics::timeseries::{
    TimeSeriesData, TimeSeriesView, TrendResult, cumulative_sum, detect_date_column,
    extract_time_series, growth_rate, linear_trend, moving_average, period_summary,
    seasonal_decomposition,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewMode {
    Selection,
    PivotTable,
    TimeSeries,
    Report,
    Loading,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PivotFocus {
    Row,
    Column,
    Value,
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
    pivot_config: PivotConfig,
    pivot_result: Option<PivotTable>,
    pivot_active_field: PivotFocus,
    pivot_scroll_offset: usize,
    pivot_column_scroll: usize,
    ts_date_column: Option<usize>,
    ts_value_column: usize,
    ts_data: Option<TimeSeriesData>,
    ts_moving_avg_window: usize,
    ts_active_analysis: TimeSeriesView,
    ts_chart_data: Vec<(f64, f64)>,
    ts_trend: Option<TrendResult>,
    status: String,
    csv_source: Option<PathBuf>,
    pending_view_after_load: Option<ViewMode>,
    csv_preview_rows: usize,
    csv_total_rows: usize,
    preview_offset: usize,
    preview_column_offset: usize,
    total_preview_lines: usize,
    selection_area: Cell<Option<Rect>>,
    preview_area: Cell<Option<Rect>>,
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
            pivot_config: PivotConfig {
                row_field: 0,
                column_field: 1,
                value_field: 1,
                aggregation: PivotAggregation::Sum,
            },
            pivot_result: None,
            pivot_active_field: PivotFocus::Row,
            pivot_scroll_offset: 0,
            pivot_column_scroll: 0,
            ts_date_column: None,
            ts_value_column: 0,
            ts_data: None,
            ts_moving_avg_window: 3,
            ts_active_analysis: TimeSeriesView::Overview,
            ts_chart_data: Vec::new(),
            ts_trend: None,
            status: "Usando dados de amostra. Pressione I para importar Excel ou CSV".to_string(),
            csv_source: None,
            pending_view_after_load: None,
            csv_preview_rows: 0,
            csv_total_rows: 0,
            preview_offset: 0,
            preview_column_offset: 0,
            total_preview_lines: 0,
            selection_area: Cell::new(None),
            preview_area: Cell::new(None),
        };
        app.refresh_numeric_columns();
        app.recompute_kpis();
        app.total_preview_lines = app.row_count();
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
            KeyCode::Char('1') => self.open_selection(),
            KeyCode::Char('2') => self.open_pivot_table(),
            KeyCode::Char('3') => self.open_time_series(),
            KeyCode::Char('4') => self.open_report(),
            KeyCode::Char('r') => self.open_report(),
            KeyCode::Char('z') => self.open_selection(),
            KeyCode::Char('i') => self.try_import_file(),
            KeyCode::Char('p') if self.is_selection_view() => self.down_preview_offset(),
            KeyCode::Char('o') if self.is_selection_view() => self.up_preview_offset(),
            KeyCode::Char('d') if self.is_selection_view() => {
                self.down_preview_column_offset()
            }
            KeyCode::Char('a') if self.is_selection_view() => {
                self.up_preview_column_offset()
            }
            KeyCode::Up if self.is_selection_view() => self.previous_kpi(),
            KeyCode::Down if self.is_selection_view() => self.next_kpi(),
            KeyCode::Char(' ') | KeyCode::Enter if self.is_selection_view() => {
                self.toggle_selected_kpi()
            }
            KeyCode::Left if self.is_selection_view() => self.previous_numeric_column(),
            KeyCode::Right if self.is_selection_view() => self.next_numeric_column(),
            KeyCode::Tab if self.is_pivot_view() => self.next_pivot_focus(),
            KeyCode::BackTab if self.is_pivot_view() => self.previous_pivot_focus(),
            KeyCode::Left if self.is_pivot_view() => self.move_active_pivot_field(-1),
            KeyCode::Right if self.is_pivot_view() => self.move_active_pivot_field(1),
            KeyCode::Up if self.is_pivot_view() => self.scroll_pivot_up(),
            KeyCode::Down if self.is_pivot_view() => self.scroll_pivot_down(),
            KeyCode::Char(' ') if self.is_pivot_view() => self.cycle_pivot_aggregation(),
            KeyCode::Enter if self.is_pivot_view() => self.recompute_pivot(),
            KeyCode::Tab if self.is_timeseries_view() => self.next_timeseries_date_column(),
            KeyCode::Up if self.is_timeseries_view() => self.previous_timeseries_value_column(),
            KeyCode::Down if self.is_timeseries_view() => self.next_timeseries_value_column(),
            KeyCode::Left if self.is_timeseries_view() => self.previous_timeseries_view(),
            KeyCode::Right if self.is_timeseries_view() => self.next_timeseries_view(),
            KeyCode::Char('+') | KeyCode::Char('=') if self.is_timeseries_view() => {
                self.increase_timeseries_window()
            }
            KeyCode::Char('-') if self.is_timeseries_view() => self.decrease_timeseries_window(),
            KeyCode::Enter if self.is_timeseries_view() => self.recompute_timeseries(),
            _ => {}
        }
    }

    fn handle_mouse_event(&mut self, mouse_event: MouseEvent) {
        if self.view_mode != ViewMode::Selection {
            return;
        }

        let x = mouse_event.column;
        let y = mouse_event.row;

        let Some(preview_area) = self.preview_area.get() else {
            if let Some(selection_area) = self.selection_area.get() {
                if self.point_in_rect(selection_area, x, y) {
                    self.handle_selection_mouse(mouse_event, selection_area);
                }
            }
            return;
        };

        if self.point_in_rect(preview_area, x, y) {
            self.handle_preview_mouse(mouse_event);
            return;
        }

        if let Some(selection_area) = self.selection_area.get() {
            if self.point_in_rect(selection_area, x, y) {
                self.handle_selection_mouse(mouse_event, selection_area);
            }
        }
    }

    fn handle_preview_mouse(&mut self, mouse_event: MouseEvent) {
        match mouse_event.kind {
            MouseEventKind::ScrollDown => self.down_preview_offset(),
            MouseEventKind::ScrollUp => self.up_preview_offset(),
            MouseEventKind::ScrollRight => self.down_preview_column_offset(),
            MouseEventKind::ScrollLeft => self.up_preview_column_offset(),
            _ => {}
        }
    }

    fn handle_selection_mouse(&mut self, mouse_event: MouseEvent, selection_area: Rect) {
        match mouse_event.kind {
            MouseEventKind::ScrollDown => self.next_kpi(),
            MouseEventKind::ScrollUp => self.previous_kpi(),
            MouseEventKind::Down(crossterm::event::MouseButton::Left) => {
                self.toggle_kpi_at(selection_area, mouse_event.row);
            }
            _ => {}
        }
    }

    fn point_in_rect(&self, rect: Rect, x: u16, y: u16) -> bool {
        x >= rect.x && x < rect.x + rect.width && y >= rect.y && y < rect.y + rect.height
    }

    fn toggle_kpi_at(&mut self, selection_area: Rect, row: u16) {
        let content_row = row.saturating_sub(selection_area.y + 1) as usize;
        if content_row == 0 {
            return;
        }

        let index = content_row - 1;
        if index < self.catalog.len() {
            self.selected_kpi = index;
            self.toggle_selected_kpi();
        }
    }

    fn exit(&mut self) {
        self.exit = true;
    }

    fn current_sheet(&self) -> &ImportedSheet {
        &self.workbook.sheets[self.active_sheet]
    }

    fn down_preview_offset(&mut self) {
        if self.preview_offset + 1 < self.total_preview_lines {
            self.preview_offset += 1;
        }
    }

    fn up_preview_offset(&mut self) {
        if self.preview_offset > 0 {
            self.preview_offset -= 1;
        }
    }

    fn down_preview_column_offset(&mut self) {
        if self.preview_column_offset + 1 < self.current_sheet().column_count() {
            self.preview_column_offset += 1;
        }
    }

    fn up_preview_column_offset(&mut self) {
        if self.preview_column_offset > 0 {
            self.preview_column_offset -= 1;
        }
    }

    fn refresh_numeric_columns(&mut self) {
        self.numeric_columns = self.current_sheet().numeric_column_indices();
        self.total_preview_lines = self.row_count();
        if self.preview_offset >= self.total_preview_lines {
            self.preview_offset = self.total_preview_lines.saturating_sub(1);
        }
        let total_columns = self.current_sheet().column_count();
        if self.preview_column_offset >= total_columns {
            self.preview_column_offset = total_columns.saturating_sub(1);
        }
        if self.numeric_columns.is_empty() {
            self.active_numeric_column = 0;
        } else if self.active_numeric_column >= self.numeric_columns.len() {
            self.active_numeric_column = self.numeric_columns.len() - 1;
        }

        self.refresh_analysis_state();
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

    fn refresh_analysis_state(&mut self) {
        self.refresh_pivot_state();
        self.refresh_timeseries_state();
    }

    fn refresh_pivot_state(&mut self) {
        let sheet = self.current_sheet().clone();
        let column_count = sheet.column_count();
        if column_count == 0 {
            self.pivot_result = None;
            return;
        }

        let row_field = self.pivot_config.row_field.min(column_count - 1);
        let column_field = self.pivot_config.column_field.min(column_count - 1);
        let numeric_columns = sheet.numeric_column_indices();
        let mut value_field = self
            .pivot_config
            .value_field
            .min(column_count - 1);

        if !numeric_columns.contains(&value_field) {
            value_field = numeric_columns
                .iter()
                .copied()
                .find(|&index| index != row_field && index != column_field)
                .unwrap_or(value_field);
        }

        self.pivot_config.row_field = row_field;
        self.pivot_config.column_field = column_field;
        self.pivot_config.value_field = value_field;

        self.pivot_result = Some(compute_pivot(&sheet, &self.pivot_config));
    }

    fn refresh_timeseries_state(&mut self) {
        let sheet = self.current_sheet().clone();
        let column_count = sheet.column_count();
        if column_count == 0 {
            self.ts_date_column = None;
            self.ts_value_column = 0;
            self.ts_data = None;
            self.ts_chart_data.clear();
            self.ts_trend = None;
            return;
        }

        let detected_date_column = self.ts_date_column.or_else(|| detect_date_column(&sheet));
        self.ts_date_column = detected_date_column;

        let numeric_columns = sheet.numeric_column_indices();
        if self.ts_value_column >= column_count || !numeric_columns.contains(&self.ts_value_column) {
            self.ts_value_column = numeric_columns
                .iter()
                .copied()
                .find(|&index| Some(index) != detected_date_column)
                .unwrap_or_else(|| if column_count > 1 { 1 } else { 0 });
        }

        let Some(date_column) = self.ts_date_column else {
            self.ts_data = None;
            self.ts_chart_data.clear();
            self.ts_trend = None;
            return;
        };

        if date_column >= column_count || self.ts_value_column >= column_count || date_column == self.ts_value_column {
            self.ts_data = None;
            self.ts_chart_data.clear();
            self.ts_trend = None;
            return;
        }

        match extract_time_series(&sheet, date_column, self.ts_value_column) {
            Ok(data) => {
                self.ts_chart_data = time_series_to_chart_points(&data);
                self.ts_trend = Some(linear_trend(&data));
                self.ts_data = Some(data);
            }
            Err(_) => {
                self.ts_data = None;
                self.ts_chart_data.clear();
                self.ts_trend = None;
            }
        }
    }

    fn open_pivot_table(&mut self) {
        self.recompute_pivot();
        self.view_mode = ViewMode::PivotTable;
        self.status = format!(
            "Tabela pivô pronta para {}. Pressione Enter para recalcular.",
            self.sheet_name()
        );
    }

    fn open_time_series(&mut self) {
        self.recompute_timeseries();
        self.view_mode = ViewMode::TimeSeries;
        self.status = format!(
            "Série temporal pronta para {}. Pressione Enter para recalcular.",
            self.sheet_name()
        );
    }

    fn recompute_pivot(&mut self) {
        self.refresh_pivot_state();
    }

    fn recompute_timeseries(&mut self) {
        self.refresh_timeseries_state();
    }

    fn next_pivot_focus(&mut self) {
        self.pivot_active_field = match self.pivot_active_field {
            PivotFocus::Row => PivotFocus::Column,
            PivotFocus::Column => PivotFocus::Value,
            PivotFocus::Value => PivotFocus::Row,
        };
    }

    fn previous_pivot_focus(&mut self) {
        self.pivot_active_field = match self.pivot_active_field {
            PivotFocus::Row => PivotFocus::Value,
            PivotFocus::Column => PivotFocus::Row,
            PivotFocus::Value => PivotFocus::Column,
        };
    }

    fn move_active_pivot_field(&mut self, delta: isize) {
        let column_count = self.column_count();
        if column_count == 0 {
            return;
        }

        let adjust = |value: &mut usize| {
            if delta < 0 {
                *value = value.saturating_sub(1);
            } else if *value + 1 < column_count {
                *value += 1;
            }
        };

        match self.pivot_active_field {
            PivotFocus::Row => adjust(&mut self.pivot_config.row_field),
            PivotFocus::Column => adjust(&mut self.pivot_config.column_field),
            PivotFocus::Value => adjust(&mut self.pivot_config.value_field),
        }

        self.refresh_pivot_state();
    }

    fn scroll_pivot_up(&mut self) {
        self.pivot_scroll_offset = self.pivot_scroll_offset.saturating_sub(1);
    }

    fn scroll_pivot_down(&mut self) {
        if let Some(pivot) = &self.pivot_result {
            if self.pivot_scroll_offset + 1 < pivot.row_count() {
                self.pivot_scroll_offset += 1;
            }
        }
    }

    fn cycle_pivot_aggregation(&mut self) {
        self.pivot_config.aggregation = self.pivot_config.aggregation.next();
        self.refresh_pivot_state();
    }

    fn next_timeseries_view(&mut self) {
        self.ts_active_analysis = self.ts_active_analysis.next();
    }

    fn previous_timeseries_view(&mut self) {
        self.ts_active_analysis = self.ts_active_analysis.prev();
    }

    fn next_timeseries_date_column(&mut self) {
        let column_count = self.column_count();
        if column_count == 0 {
            return;
        }

        self.ts_date_column = Some(match self.ts_date_column {
            Some(index) => (index + 1) % column_count,
            None => 0,
        });
        self.refresh_timeseries_state();
    }

    fn previous_timeseries_value_column(&mut self) {
        if self.column_count() == 0 {
            return;
        }

        if self.ts_value_column > 0 {
            self.ts_value_column -= 1;
        }
        self.refresh_timeseries_state();
    }

    fn next_timeseries_value_column(&mut self) {
        let column_count = self.column_count();
        if self.ts_value_column + 1 < column_count {
            self.ts_value_column += 1;
        }
        self.refresh_timeseries_state();
    }

    fn increase_timeseries_window(&mut self) {
        let max_window = self.ts_data.as_ref().map(|data| data.len().max(1)).unwrap_or(1);
        self.ts_moving_avg_window = (self.ts_moving_avg_window.saturating_add(1)).min(max_window);
    }

    fn decrease_timeseries_window(&mut self) {
        self.ts_moving_avg_window = self.ts_moving_avg_window.saturating_sub(1).max(1);
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
                self.csv_total_rows = outcome.csv_total_rows;
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
            Event::Mouse(mouse_event) => self.handle_mouse_event(mouse_event),
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

    pub(crate) fn is_selection_view(&self) -> bool {
        self.view_mode == ViewMode::Selection
    }

    pub(crate) fn is_pivot_view(&self) -> bool {
        self.view_mode == ViewMode::PivotTable
    }

    pub(crate) fn is_timeseries_view(&self) -> bool {
        self.view_mode == ViewMode::TimeSeries
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

    pub(crate) fn csv_total_rows(&self) -> usize {
        self.csv_total_rows
    }

    pub(crate) fn display_row_count(&self) -> usize {
        if self.has_csv_source() && self.csv_total_rows > 0 {
            self.csv_total_rows
        } else {
            self.row_count()
        }
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

    pub(crate) fn preview_offset(&self) -> usize {
        self.preview_offset
    }

    pub(crate) fn total_preview_lines(&self) -> usize {
        self.total_preview_lines
    }

    pub(crate) fn set_selection_area(&self, area: Option<Rect>) {
        self.selection_area.set(area);
    }

    pub(crate) fn set_preview_area(&self, area: Option<Rect>) {
        self.preview_area.set(area);
    }

    pub(crate) fn preview_column_offset(&self) -> usize {
        self.preview_column_offset
    }

    pub(crate) fn preview_table_lines(&self, limit: usize) -> Vec<String> {
        self.preview_table_window_lines(limit, self.preview_column_offset, usize::MAX)
    }

    pub(crate) fn preview_table_window_lines(
        &self,
        limit: usize,
        column_offset: usize,
        available_width: usize,
    ) -> Vec<String> {
        let sheet = self.current_sheet();
        let column_count = sheet.column_count();
        if column_count == 0 {
            return vec!["Sem colunas detectadas".to_string()];
        }

        let preview_rows = sheet
            .rows
            .iter()
            .skip(self.preview_offset)
            .take(limit)
            .collect::<Vec<_>>();
        let index_width = sheet.rows.len().saturating_sub(1).max(1).to_string().len().max(2);

        let column_offset = column_offset.min(column_count.saturating_sub(1));
        let mut visible_columns = Vec::new();
        let mut widths = Vec::new();
        let mut used_width = index_width;

        for column in column_offset..column_count {
            let mut column_width = sheet.column_name(column).len();
            for row in &preview_rows {
                if let Some(cell) = row.get(column) {
                    column_width = column_width.max(cell.len());
                }
            }

            let next_width = used_width + column_width + 3;
            if !visible_columns.is_empty() && next_width > available_width {
                break;
            }

            visible_columns.push(column);
            widths.push(column_width);
            used_width = next_width;
        }

        if visible_columns.is_empty() {
            let column = column_offset;
            let mut column_width = sheet.column_name(column).len();
            for row in &preview_rows {
                if let Some(cell) = row.get(column) {
                    column_width = column_width.max(cell.len());
                }
            }
            visible_columns.push(column);
            widths.push(column_width);
        }

        let mut lines = Vec::new();
        let header_row = visible_columns
            .iter()
            .map(|&column| sheet.column_name(column))
            .collect::<Vec<_>>();
        lines.push(format_dataframe_row("idx", &header_row, &widths, index_width));

        for (row_index, row) in preview_rows.iter().enumerate() {
            let actual_row_index = self.preview_offset + row_index;
            let visible_row = visible_columns
                .iter()
                .map(|&column| row.get(column).cloned().unwrap_or_default())
                .collect::<Vec<_>>();
            lines.push(format_dataframe_row(&actual_row_index.to_string(), &visible_row, &widths, index_width));
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

    pub(crate) fn pivot_row_field_name(&self) -> String {
        self.current_sheet().column_name(self.pivot_config.row_field)
    }

    pub(crate) fn pivot_col_field_name(&self) -> String {
        self.current_sheet().column_name(self.pivot_config.column_field)
    }

    pub(crate) fn pivot_value_field_name(&self) -> String {
        self.current_sheet().column_name(self.pivot_config.value_field)
    }

    pub(crate) fn pivot_aggregation_name(&self) -> String {
        self.pivot_config.aggregation.to_string()
    }

    pub(crate) fn pivot_result(&self) -> Option<&PivotTable> {
        self.pivot_result.as_ref()
    }

    pub(crate) fn pivot_scroll_offset(&self) -> usize {
        self.pivot_scroll_offset
    }

    pub(crate) fn pivot_column_scroll(&self) -> usize {
        self.pivot_column_scroll
    }

    pub(crate) fn ts_has_data(&self) -> bool {
        self.ts_data.as_ref().is_some_and(|data| !data.is_empty())
    }

    pub(crate) fn ts_active_view_name(&self) -> String {
        self.ts_active_analysis.to_string()
    }

    pub(crate) fn ts_date_column_name(&self) -> String {
        self.ts_date_column
            .map(|index| self.current_sheet().column_name(index))
            .unwrap_or_else(|| "Auto".to_string())
    }

    pub(crate) fn ts_value_column_name(&self) -> String {
        self.current_sheet().column_name(self.ts_value_column)
    }

    pub(crate) fn ts_moving_avg_window(&self) -> usize {
        self.ts_moving_avg_window
    }

    pub(crate) fn ts_chart_points(&self) -> Vec<(f64, f64)> {
        if !self.ts_has_data() {
            return Vec::new();
        }

        let Some(data) = &self.ts_data else {
            return Vec::new();
        };

        match self.ts_active_analysis {
            TimeSeriesView::Overview | TimeSeriesView::Trend | TimeSeriesView::MovingAverage => {
                self.ts_chart_data.clone()
            }
            TimeSeriesView::CumulativeSum => {
                time_series_to_chart_points_from_points(&cumulative_sum(data))
            }
            TimeSeriesView::GrowthRate => time_series_growth_chart_points(data),
            TimeSeriesView::Seasonal => self.ts_chart_data.clone(),
            TimeSeriesView::PeriodSummary => Vec::new(),
        }
    }

    pub(crate) fn ts_overlay_points(&self) -> Vec<(f64, f64)> {
        let Some(data) = &self.ts_data else {
            return Vec::new();
        };

        match self.ts_active_analysis {
            TimeSeriesView::MovingAverage => time_series_to_chart_points_from_points(&moving_average(data, self.ts_moving_avg_window.max(1))),
            TimeSeriesView::Trend => self.ts_trend.as_ref().map_or_else(Vec::new, |trend| {
                if data.points.is_empty() {
                    return Vec::new();
                }

                let last = (data.points.last().unwrap().date - data.points.first().unwrap().date).num_days() as f64;
                vec![(0.0, trend.slope * 0.0 + trend.intercept), (last, trend.slope * last + trend.intercept)]
            }),
            TimeSeriesView::Seasonal => {
                let seasonal = seasonal_decomposition(data, self.ts_moving_avg_window.max(2));
                data.points
                    .iter()
                    .zip(seasonal.trend.iter())
                    .filter_map(|(point, trend)| trend.map(|value| (point.date.num_days_from_ce() as f64, value)))
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    pub(crate) fn ts_chart_x_label_start(&self) -> String {
        self.ts_data
            .as_ref()
            .and_then(|data| data.points.first().map(|point| point.date.to_string()))
            .unwrap_or_default()
    }

    pub(crate) fn ts_chart_x_label_end(&self) -> String {
        self.ts_data
            .as_ref()
            .and_then(|data| data.points.last().map(|point| point.date.to_string()))
            .unwrap_or_default()
    }

    pub(crate) fn ts_result_lines(&self) -> Vec<String> {
        let Some(data) = &self.ts_data else {
            return Vec::new();
        };

        let mut lines = Vec::new();
        lines.push(format!("Análise: {}", self.ts_active_analysis));
        lines.push(format!("Pontos válidos: {}", data.len()));
        lines.push(format!("Data: {}", self.ts_date_column_name()));
        lines.push(format!("Valor: {}", self.ts_value_column_name()));

        match self.ts_active_analysis {
            TimeSeriesView::Overview => {
                let values = data.values();
                let min = values.iter().copied().fold(f64::INFINITY, f64::min);
                let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let mean = values.iter().sum::<f64>() / values.len() as f64;
                lines.push("───".to_string());
                lines.push(format!("Mínimo: {:.2}", min));
                lines.push(format!("Máximo: {:.2}", max));
                lines.push(format!("Média: {:.2}", mean));
            }
            TimeSeriesView::MovingAverage => {
                let values = moving_average(data, self.ts_moving_avg_window.max(1));
                lines.push("───".to_string());
                lines.push(format!("Janela: {}", self.ts_moving_avg_window));
                lines.push(format!("Pontos calculados: {}", values.len()));
            }
            TimeSeriesView::GrowthRate => {
                let values = growth_rate(data);
                lines.push("───".to_string());
                lines.push(format!("Pontos calculados: {}", values.len()));
            }
            TimeSeriesView::Trend => {
                if let Some(trend) = &self.ts_trend {
                    lines.push("───".to_string());
                    lines.push(format!("Inclinação: {:.4}", trend.slope));
                    lines.push(format!("Intercepto: {:.4}", trend.intercept));
                    lines.push(format!("Direção: {}", trend.direction));
                    lines.push(format!("R²: {:.4}", trend.r_squared));
                }
            }
            TimeSeriesView::CumulativeSum => {
                let values = cumulative_sum(data);
                if let Some(last) = values.last() {
                    lines.push("───".to_string());
                    lines.push(format!("Total acumulado: {:.2}", last.value));
                }
            }
            TimeSeriesView::Seasonal => {
                let seasonal = seasonal_decomposition(data, self.ts_moving_avg_window.max(2));
                lines.push("───".to_string());
                lines.push(format!("Período: {}", seasonal.period));
                lines.push(format!("Tendência: {} valores", seasonal.trend.len()));
                lines.push(format!("Sazonal: {} valores", seasonal.seasonal.len()));
                lines.push(format!("Resíduo: {} valores", seasonal.residual.len()));
            }
            TimeSeriesView::PeriodSummary => {
                lines.push("───".to_string());
                for period in period_summary(data) {
                    lines.push(format!(
                        "{}: min {:.2}, max {:.2}, média {:.2}, n={}",
                        period.period_label, period.min, period.max, period.mean, period.count
                    ));
                }
            }
        }

        lines
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
    csv_total_rows: usize,
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
            let (workbook, truncated, total_rows) = import_csv_preview_with_total_rows(path, 1000)?;
            Ok(ImportOutcome {
                workbook,
                csv_source: if truncated { Some(path.clone()) } else { None },
                csv_preview_rows: if truncated { 1000 } else { 0 },
                csv_total_rows: total_rows,
            })
        }
        Some("xlsx") | Some("xls") | Some("xlsm") => Ok(ImportOutcome {
            workbook: import_workbook(path)?,
            csv_source: None,
            csv_preview_rows: 0,
            csv_total_rows: 0,
        }),
        _ => match import_workbook(path) {
            Ok(workbook) => Ok(ImportOutcome {
                workbook,
                csv_source: None,
                csv_preview_rows: 0,
                csv_total_rows: 0,
            }),
            Err(_) => {
                let (workbook, truncated, total_rows) = import_csv_preview_with_total_rows(path, 1000)?;
                Ok(ImportOutcome {
                    workbook,
                    csv_source: if truncated { Some(path.clone()) } else { None },
                    csv_preview_rows: if truncated { 1000 } else { 0 },
                    csv_total_rows: total_rows,
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
    let index_display_width = index_width.max(index.len());
    parts.push(format!("{index:>index_display_width$}"));

    for column in 0..widths.len() {
        let cell = row.get(column).map(|value| value.as_str()).unwrap_or("");
        parts.push(format!("{cell:<width$}", width = widths[column]));
    }

    parts.join(" | ")
}

fn time_series_to_chart_points(data: &TimeSeriesData) -> Vec<(f64, f64)> {
    time_series_to_chart_points_from_points(&data.points)
}

fn time_series_to_chart_points_from_points<T>(points: &[T]) -> Vec<(f64, f64)>
where
    T: TimeSeriesPointLike,
{
    if points.is_empty() {
        return Vec::new();
    }

    let base_date = points[0].date();
    points
        .iter()
        .map(|point| ((point.date() - base_date).num_days() as f64, point.value()))
        .collect()
}

fn time_series_growth_chart_points(data: &TimeSeriesData) -> Vec<(f64, f64)> {
    let growth = growth_rate(data);
    if growth.is_empty() {
        return Vec::new();
    }

    let base_date = growth[0].date;
    growth
        .into_iter()
        .map(|point| ((point.date - base_date).num_days() as f64, point.growth_pct.unwrap_or(0.0)))
        .collect()
}

trait TimeSeriesPointLike {
    fn date(&self) -> NaiveDate;
    fn value(&self) -> f64;
}

impl TimeSeriesPointLike for analytics::timeseries::TimeSeriesPoint {
    fn date(&self) -> NaiveDate {
        self.date
    }

    fn value(&self) -> f64 {
        self.value
    }
}

impl TimeSeriesPointLike for analytics::timeseries::GrowthPoint {
    fn date(&self) -> NaiveDate {
        self.date
    }

    fn value(&self) -> f64 {
        self.growth_pct.unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests;

fn main() -> io::Result<()> {
    let mut terminal = ratatui::try_init()?;

    if let Err(err) = crossterm::execute!(io::stdout(), EnableMouseCapture) {
        ratatui::restore();
        return Err(err);
    }

    struct Cleanup;

    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = crossterm::execute!(io::stdout(), DisableMouseCapture);
            ratatui::restore();
        }
    }

    let _cleanup = Cleanup;
    App::default().run(&mut terminal)
}