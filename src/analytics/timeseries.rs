//! Motor de análise de séries temporais.
//!
//! Fornece funcionalidades de parsing de datas, extração de séries temporais
//! a partir de planilhas importadas e funções de análise estatística como
//! média móvel, taxa de crescimento, tendência linear, soma acumulada,
//! decomposição sazonal e resumo por período.

use std::fmt;

use chrono::{Datelike, NaiveDate};

use crate::analytics::excel::ImportedSheet;

// ---------------------------------------------------------------------------
// Formatos de data
// ---------------------------------------------------------------------------

/// Formato de data reconhecido pelo parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DateFormat {
    /// Ano-Mês-Dia (ex: 2024-01-15)
    YearMonthDay,
    /// Dia/Mês/Ano (ex: 15/01/2024)
    DayMonthYear,
    /// Mês/Dia/Ano (ex: 01/15/2024)
    MonthDayYear,
    /// ISO 8601 com horário (ex: 2024-01-15T10:30:00)
    Iso8601,
    /// Nome abreviado do mês (ex: Jan, Fev, Mar)
    MonthName,
}

/// Formatos suportados para parsing de datas via `chrono`.
///
/// Cada entrada é um par `(formato_chrono, variante_DateFormat)`.
pub const SUPPORTED_FORMATS: &[(&str, DateFormat)] = &[
    ("%Y-%m-%d", DateFormat::YearMonthDay),
    ("%d/%m/%Y", DateFormat::DayMonthYear),
    ("%m/%d/%Y", DateFormat::MonthDayYear),
    ("%Y-%m-%dT%H:%M:%S", DateFormat::Iso8601),
    ("%Y-%m-%d %H:%M:%S", DateFormat::Iso8601),
];

/// Abreviações de meses em português.
const MONTH_NAMES_PT: &[&str] = &[
    "Jan", "Fev", "Mar", "Abr", "Mai", "Jun",
    "Jul", "Ago", "Set", "Out", "Nov", "Dez",
];

/// Abreviações de meses em inglês.
const MONTH_NAMES_EN: &[&str] = &[
    "Jan", "Feb", "Mar", "Apr", "May", "Jun",
    "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Tenta parsear uma string como data usando todos os formatos suportados.
///
/// Além dos formatos de `chrono`, reconhece abreviações de meses em
/// português e inglês (ex: "Jan", "Fev", "Mar"), retornando `NaiveDate`
/// com ano=2024 e dia=1 como padrão.
pub fn parse_date_flexible(value: &str) -> Option<NaiveDate> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Tenta cada formato chrono
    for &(fmt, _) in SUPPORTED_FORMATS {
        if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(trimmed, fmt) {
            return Some(dt.date());
        }
        if let Ok(d) = NaiveDate::parse_from_str(trimmed, fmt) {
            return Some(d);
        }
    }

    // Tenta nome abreviado do mês (pt-BR)
    let lower = trimmed.to_lowercase();
    for (i, &name) in MONTH_NAMES_PT.iter().enumerate() {
        if lower == name.to_lowercase() {
            return NaiveDate::from_ymd_opt(2024, (i + 1) as u32, 1);
        }
    }

    // Tenta nome abreviado do mês (en)
    for (i, &name) in MONTH_NAMES_EN.iter().enumerate() {
        if lower == name.to_lowercase() {
            return NaiveDate::from_ymd_opt(2024, (i + 1) as u32, 1);
        }
    }

    None
}

/// Detecta a coluna que contém datas em uma planilha.
///
/// Retorna o índice da primeira coluna onde pelo menos 50% dos valores
/// não-vazios são reconhecidos como datas válidas.
pub fn detect_date_column(sheet: &ImportedSheet) -> Option<usize> {
    let col_count = sheet.column_count();

    for col in 0..col_count {
        let mut total = 0usize;
        let mut parsed = 0usize;

        for row in &sheet.rows {
            if let Some(cell) = row.get(col) {
                let trimmed = cell.trim();
                if trimmed.is_empty() {
                    continue;
                }
                total += 1;
                if parse_date_flexible(trimmed).is_some() {
                    parsed += 1;
                }
            }
        }

        if total > 0 && parsed * 2 >= total {
            return Some(col);
        }
    }

    None
}

/// Detecta qual formato de data funciona melhor para um conjunto de valores.
///
/// Retorna o formato que conseguiu parsear o maior número de valores
/// não-vazios, desde que pelo menos um valor tenha sido parseado.
pub fn detect_date_format(values: &[String]) -> Option<DateFormat> {
    let non_empty: Vec<&str> = values
        .iter()
        .map(|v| v.trim())
        .filter(|v| !v.is_empty())
        .collect();

    if non_empty.is_empty() {
        return None;
    }

    let mut best_format: Option<DateFormat> = None;
    let mut best_count = 0usize;

    // Testa cada formato chrono
    for &(fmt, date_format) in SUPPORTED_FORMATS {
        let count = non_empty
            .iter()
            .filter(|v| {
                chrono::NaiveDateTime::parse_from_str(v, fmt).is_ok()
                    || NaiveDate::parse_from_str(v, fmt).is_ok()
            })
            .count();

        if count > best_count {
            best_count = count;
            best_format = Some(date_format);
        }
    }

    // Testa nomes de meses
    let month_count = non_empty
        .iter()
        .filter(|v| {
            let lower = v.to_lowercase();
            MONTH_NAMES_PT
                .iter()
                .chain(MONTH_NAMES_EN.iter())
                .any(|name| lower == name.to_lowercase())
        })
        .count();

    if month_count > best_count {
        best_format = Some(DateFormat::MonthName);
    }

    best_format
}

// ---------------------------------------------------------------------------
// Estruturas de dados da série temporal
// ---------------------------------------------------------------------------

/// Um ponto individual em uma série temporal.
#[derive(Clone, Debug)]
pub struct TimeSeriesPoint {
    /// Data do ponto.
    pub date: NaiveDate,
    /// Valor numérico associado.
    pub value: f64,
}

/// Série temporal extraída de uma planilha.
///
/// Os pontos são mantidos ordenados por data.
#[derive(Clone, Debug)]
pub struct TimeSeriesData {
    /// Pontos da série, ordenados por data.
    pub points: Vec<TimeSeriesPoint>,
    /// Nome da coluna de datas.
    pub date_column_name: String,
    /// Nome da coluna de valores.
    pub value_column_name: String,
}

impl TimeSeriesData {
    /// Quantidade de pontos na série.
    pub fn len(&self) -> usize {
        self.points.len()
    }

    /// Verifica se a série está vazia.
    pub fn is_empty(&self) -> bool {
        self.points.is_empty()
    }

    /// Retorna todas as datas da série.
    pub fn dates(&self) -> Vec<NaiveDate> {
        self.points.iter().map(|p| p.date).collect()
    }

    /// Retorna todos os valores da série.
    pub fn values(&self) -> Vec<f64> {
        self.points.iter().map(|p| p.value).collect()
    }

    /// Retorna o intervalo de datas (primeira, última), ou `None` se vazio.
    pub fn date_range(&self) -> Option<(NaiveDate, NaiveDate)> {
        if self.points.is_empty() {
            return None;
        }
        Some((self.points.first().unwrap().date, self.points.last().unwrap().date))
    }
}

// ---------------------------------------------------------------------------
// Extração de série temporal
// ---------------------------------------------------------------------------

/// Extrai uma série temporal de uma planilha importada.
///
/// Parseia a coluna de datas e a coluna de valores, ignorando linhas onde
/// qualquer uma delas falhe. Retorna erro se houver menos de 2 pontos válidos.
pub fn extract_time_series(
    sheet: &ImportedSheet,
    date_col: usize,
    value_col: usize,
) -> Result<TimeSeriesData, String> {
    let mut points = Vec::new();

    for row in &sheet.rows {
        let date_str = match row.get(date_col) {
            Some(v) => v.trim(),
            None => continue,
        };
        let value_str = match row.get(value_col) {
            Some(v) => v.trim(),
            None => continue,
        };

        let date = match parse_date_flexible(date_str) {
            Some(d) => d,
            None => continue,
        };
        let value = match value_str.parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };

        points.push(TimeSeriesPoint { date, value });
    }

    if points.len() < 2 {
        return Err(format!(
            "Dados insuficientes: encontrados {} ponto(s), mínimo necessário é 2",
            points.len()
        ));
    }

    points.sort_by_key(|p| p.date);

    Ok(TimeSeriesData {
        points,
        date_column_name: sheet.column_name(date_col),
        value_column_name: sheet.column_name(value_col),
    })
}

// ---------------------------------------------------------------------------
// Funções de análise
// ---------------------------------------------------------------------------

/// Calcula a média móvel simples da série temporal.
///
/// Para cada ponto `i` onde `i >= window - 1`, calcula a média dos
/// últimos `window` valores. O ponto resultante mantém a data original.
///
/// Retorna vetor vazio se `window` for zero ou maior que o número de pontos.
pub fn moving_average(data: &TimeSeriesData, window: usize) -> Vec<TimeSeriesPoint> {
    if window == 0 || window > data.len() {
        return Vec::new();
    }

    let values = data.values();
    let mut result = Vec::with_capacity(data.len() - window + 1);

    let mut window_sum: f64 = values[..window].iter().sum();
    result.push(TimeSeriesPoint {
        date: data.points[window - 1].date,
        value: window_sum / window as f64,
    });

    for i in window..values.len() {
        window_sum += values[i] - values[i - window];
        result.push(TimeSeriesPoint {
            date: data.points[i].date,
            value: window_sum / window as f64,
        });
    }

    result
}

/// Ponto com taxa de crescimento período a período.
#[derive(Clone, Debug)]
pub struct GrowthPoint {
    /// Data do ponto.
    pub date: NaiveDate,
    /// Valor original.
    pub value: f64,
    /// Variação percentual em relação ao período anterior.
    /// `None` para o primeiro ponto.
    pub growth_pct: Option<f64>,
}

/// Calcula a taxa de crescimento período a período.
///
/// Para cada par consecutivo, calcula `(atual - anterior) / |anterior| * 100`.
/// O primeiro ponto não possui taxa de crescimento (`None`).
pub fn growth_rate(data: &TimeSeriesData) -> Vec<GrowthPoint> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(data.len());

    result.push(GrowthPoint {
        date: data.points[0].date,
        value: data.points[0].value,
        growth_pct: None,
    });

    for i in 1..data.len() {
        let prev = data.points[i - 1].value;
        let curr = data.points[i].value;

        let pct = if prev.abs() < f64::EPSILON {
            None
        } else {
            Some((curr - prev) / prev.abs() * 100.0)
        };

        result.push(GrowthPoint {
            date: data.points[i].date,
            value: curr,
            growth_pct: pct,
        });
    }

    result
}

/// Direção da tendência linear.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrendDirection {
    /// Tendência de alta.
    Up,
    /// Tendência de baixa.
    Down,
    /// Tendência estável.
    Flat,
}

impl fmt::Display for TrendDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrendDirection::Up => write!(f, "↑ Crescente"),
            TrendDirection::Down => write!(f, "↓ Decrescente"),
            TrendDirection::Flat => write!(f, "→ Estável"),
        }
    }
}

/// Resultado da regressão linear (tendência).
#[derive(Clone, Debug)]
pub struct TrendResult {
    /// Inclinação da reta de tendência.
    pub slope: f64,
    /// Intercepto (valor estimado no dia zero).
    pub intercept: f64,
    /// Direção da tendência.
    pub direction: TrendDirection,
    /// Coeficiente de determinação (R²).
    pub r_squared: f64,
}

/// Calcula a tendência linear da série usando regressão por mínimos quadrados.
///
/// Os valores de X representam dias desde a primeira data (como `f64`).
/// Retorna inclinação, intercepto, R² e direção.
pub fn linear_trend(data: &TimeSeriesData) -> TrendResult {
    let n = data.len();

    if n == 0 {
        return TrendResult {
            slope: 0.0,
            intercept: 0.0,
            direction: TrendDirection::Flat,
            r_squared: 0.0,
        };
    }

    let first_date = data.points[0].date;
    let xs: Vec<f64> = data
        .points
        .iter()
        .map(|p| (p.date - first_date).num_days() as f64)
        .collect();
    let ys = data.values();

    let n_f = n as f64;
    let sum_x: f64 = xs.iter().sum();
    let sum_y: f64 = ys.iter().sum();
    let sum_xy: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| x * y).sum();
    let sum_x2: f64 = xs.iter().map(|x| x * x).sum();

    let denom = n_f * sum_x2 - sum_x * sum_x;

    let (slope, intercept) = if denom.abs() < f64::EPSILON {
        (0.0, sum_y / n_f)
    } else {
        let slope = (n_f * sum_xy - sum_x * sum_y) / denom;
        let intercept = (sum_y - slope * sum_x) / n_f;
        (slope, intercept)
    };

    // Calcula R²
    let mean_y = sum_y / n_f;
    let ss_tot: f64 = ys.iter().map(|y| (y - mean_y).powi(2)).sum();
    let ss_res: f64 = xs
        .iter()
        .zip(ys.iter())
        .map(|(x, y)| {
            let predicted = slope * x + intercept;
            (y - predicted).powi(2)
        })
        .sum();

    let r_squared = if ss_tot.abs() < f64::EPSILON {
        // Todos os valores são iguais — modelo perfeito trivial
        1.0
    } else {
        1.0 - ss_res / ss_tot
    };

    let direction = if slope.abs() < 0.001 {
        TrendDirection::Flat
    } else if slope > 0.0 {
        TrendDirection::Up
    } else {
        TrendDirection::Down
    };

    TrendResult {
        slope,
        intercept,
        direction,
        r_squared,
    }
}

/// Calcula a soma acumulada (running total) da série temporal.
pub fn cumulative_sum(data: &TimeSeriesData) -> Vec<TimeSeriesPoint> {
    let mut acc = 0.0;
    data.points
        .iter()
        .map(|p| {
            acc += p.value;
            TimeSeriesPoint {
                date: p.date,
                value: acc,
            }
        })
        .collect()
}

/// Resultado da decomposição sazonal aditiva.
#[derive(Clone, Debug)]
pub struct SeasonalResult {
    /// Componente de tendência (média móvel centrada). `None` nas bordas.
    pub trend: Vec<Option<f64>>,
    /// Componente sazonal (repetido para cada posição no ciclo).
    pub seasonal: Vec<f64>,
    /// Componente residual. `None` onde a tendência não está definida.
    pub residual: Vec<Option<f64>>,
    /// Tamanho do período sazonal.
    pub period: usize,
}

/// Decomposição sazonal aditiva da série temporal.
///
/// 1. Tendência = média móvel centrada com janela = `period`
/// 2. Detrended = original − tendência
/// 3. Sazonal = média dos valores detrended por posição `mod period`
/// 4. Residual = original − tendência − sazonal
///
/// Retorna resultado vazio se `period` for zero ou maior que o número de pontos.
pub fn seasonal_decomposition(data: &TimeSeriesData, period: usize) -> SeasonalResult {
    let n = data.len();

    if period == 0 || period > n {
        return SeasonalResult {
            trend: vec![None; n],
            seasonal: vec![0.0; n],
            residual: vec![None; n],
            period,
        };
    }

    let values = data.values();

    // 1. Componente de tendência — média móvel centrada
    let mut trend: Vec<Option<f64>> = vec![None; n];
    let half = period / 2;

    for i in 0..n {
        let start = if period % 2 == 0 {
            // Para período par, centramos entre i-half e i+half-1,
            // mas usamos a média dos extremos para centralizar
            if i < half || i + half >= n {
                continue;
            }
            i - half
        } else {
            if i < half || i + half >= n {
                continue;
            }
            i - half
        };
        let end = start + period;
        if end > n {
            continue;
        }

        let sum: f64 = values[start..end].iter().sum();

        if period % 2 == 0 {
            // Para período par, média centrada = (sum + metade dos extremos) / period
            // Abordagem simplificada: média simples do window
            trend[i] = Some(sum / period as f64);
        } else {
            trend[i] = Some(sum / period as f64);
        }
    }

    // 2. Detrended = original - tendência
    let detrended: Vec<Option<f64>> = values
        .iter()
        .zip(trend.iter())
        .map(|(&v, t)| t.map(|tv| v - tv))
        .collect();

    // 3. Componente sazonal = média dos detrended por posição mod period
    let mut seasonal_sums = vec![0.0_f64; period];
    let mut seasonal_counts = vec![0usize; period];

    for (i, dt) in detrended.iter().enumerate() {
        if let Some(val) = dt {
            let pos = i % period;
            seasonal_sums[pos] += val;
            seasonal_counts[pos] += 1;
        }
    }

    let seasonal_pattern: Vec<f64> = seasonal_sums
        .iter()
        .zip(seasonal_counts.iter())
        .map(|(&sum, &count)| if count > 0 { sum / count as f64 } else { 0.0 })
        .collect();

    let seasonal: Vec<f64> = (0..n).map(|i| seasonal_pattern[i % period]).collect();

    // 4. Residual = original - tendência - sazonal
    let residual: Vec<Option<f64>> = values
        .iter()
        .zip(trend.iter())
        .zip(seasonal.iter())
        .map(|((&v, t), &s)| t.map(|tv| v - tv - s))
        .collect();

    SeasonalResult {
        trend,
        seasonal,
        residual,
        period,
    }
}

/// Estatísticas resumidas de um período (ano-mês).
#[derive(Clone, Debug)]
pub struct PeriodStats {
    /// Rótulo do período (ex: "2024-01").
    pub period_label: String,
    /// Valor mínimo no período.
    pub min: f64,
    /// Valor máximo no período.
    pub max: f64,
    /// Média dos valores no período.
    pub mean: f64,
    /// Quantidade de pontos no período.
    pub count: usize,
}

/// Agrupa os dados por ano-mês e calcula estatísticas resumidas.
///
/// Para cada período, computa mínimo, máximo, média e contagem.
pub fn period_summary(data: &TimeSeriesData) -> Vec<PeriodStats> {
    if data.is_empty() {
        return Vec::new();
    }

    // Agrupa valores por (ano, mês)
    let mut groups: Vec<((i32, u32), Vec<f64>)> = Vec::new();

    for p in &data.points {
        let key = (p.date.year_ce().1 as i32, p.date.month());
        match groups.last_mut() {
            Some((k, vals)) if *k == key => {
                vals.push(p.value);
            }
            _ => {
                groups.push((key, vec![p.value]));
            }
        }
    }

    groups
        .into_iter()
        .map(|((year, month), vals)| {
            let count = vals.len();
            let sum: f64 = vals.iter().sum();
            let min = vals.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

            PeriodStats {
                period_label: format!("{:04}-{:02}", year, month),
                min,
                max,
                mean: sum / count as f64,
                count,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Visualizações disponíveis
// ---------------------------------------------------------------------------

/// Modos de visualização para análise de séries temporais.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimeSeriesView {
    /// Visão geral dos dados.
    Overview,
    /// Média móvel.
    MovingAverage,
    /// Taxa de crescimento.
    GrowthRate,
    /// Tendência linear.
    Trend,
    /// Soma acumulada.
    CumulativeSum,
    /// Decomposição sazonal.
    Seasonal,
    /// Resumo por período.
    PeriodSummary,
}

impl TimeSeriesView {
    /// Retorna todas as variantes em ordem.
    pub fn all() -> &'static [TimeSeriesView] {
        &[
            TimeSeriesView::Overview,
            TimeSeriesView::MovingAverage,
            TimeSeriesView::GrowthRate,
            TimeSeriesView::Trend,
            TimeSeriesView::CumulativeSum,
            TimeSeriesView::Seasonal,
            TimeSeriesView::PeriodSummary,
        ]
    }

    /// Retorna a próxima visualização (circular).
    pub fn next(self) -> TimeSeriesView {
        let views = Self::all();
        let idx = views.iter().position(|&v| v == self).unwrap_or(0);
        views[(idx + 1) % views.len()]
    }

    /// Retorna a visualização anterior (circular).
    pub fn prev(self) -> TimeSeriesView {
        let views = Self::all();
        let idx = views.iter().position(|&v| v == self).unwrap_or(0);
        views[(idx + views.len() - 1) % views.len()]
    }
}

impl fmt::Display for TimeSeriesView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TimeSeriesView::Overview => write!(f, "Visão Geral"),
            TimeSeriesView::MovingAverage => write!(f, "Média Móvel"),
            TimeSeriesView::GrowthRate => write!(f, "Taxa de Crescimento"),
            TimeSeriesView::Trend => write!(f, "Tendência Linear"),
            TimeSeriesView::CumulativeSum => write!(f, "Soma Acumulada"),
            TimeSeriesView::Seasonal => write!(f, "Decomposição Sazonal"),
            TimeSeriesView::PeriodSummary => write!(f, "Resumo por Período"),
        }
    }
}
