//! Motor de computação de tabelas dinâmicas (pivot tables).
//!
//! Este módulo fornece estruturas e funções para criar tabelas dinâmicas
//! a partir de dados importados de planilhas, suportando múltiplas
//! funções de agregação.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::analytics::excel::ImportedSheet;

// ---------------------------------------------------------------------------
// PivotAggregation
// ---------------------------------------------------------------------------

/// Tipos de agregação disponíveis para tabelas dinâmicas.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PivotAggregation {
    /// Soma dos valores.
    Sum,
    /// Contagem de valores.
    Count,
    /// Média aritmética dos valores.
    Average,
    /// Valor mínimo.
    Min,
    /// Valor máximo.
    Max,
    /// Mediana dos valores.
    Median,
    /// Desvio padrão populacional.
    StdDev,
    /// Contagem de valores distintos.
    CountDistinct,
}

/// Todas as variantes de [`PivotAggregation`] em ordem de declaração.
const ALL_AGGREGATIONS: &[PivotAggregation] = &[
    PivotAggregation::Sum,
    PivotAggregation::Count,
    PivotAggregation::Average,
    PivotAggregation::Min,
    PivotAggregation::Max,
    PivotAggregation::Median,
    PivotAggregation::StdDev,
    PivotAggregation::CountDistinct,
];

impl PivotAggregation {
    /// Retorna uma fatia contendo todas as variantes de agregação.
    pub fn all() -> &'static [PivotAggregation] {
        ALL_AGGREGATIONS
    }

    /// Avança para a próxima variante, ciclando de volta ao início.
    pub fn next(&self) -> PivotAggregation {
        let all = Self::all();
        let pos = all.iter().position(|a| a == self).unwrap_or(0);
        let next_pos = (pos + 1) % all.len();
        all[next_pos]
    }
}

impl fmt::Display for PivotAggregation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            PivotAggregation::Sum => "Soma",
            PivotAggregation::Count => "Contagem",
            PivotAggregation::Average => "Média",
            PivotAggregation::Min => "Mínimo",
            PivotAggregation::Max => "Máximo",
            PivotAggregation::Median => "Mediana",
            PivotAggregation::StdDev => "Desvio Padrão",
            PivotAggregation::CountDistinct => "Distintos",
        };
        write!(f, "{}", label)
    }
}

// ---------------------------------------------------------------------------
// PivotConfig
// ---------------------------------------------------------------------------

/// Configuração que define como uma tabela dinâmica deve ser construída.
#[derive(Clone, Debug)]
pub struct PivotConfig {
    /// Índice da coluna usada como rótulos das linhas.
    pub row_field: usize,
    /// Índice da coluna usada como cabeçalhos das colunas.
    pub column_field: usize,
    /// Índice da coluna cujos valores serão agregados.
    pub value_field: usize,
    /// Função de agregação a ser aplicada.
    pub aggregation: PivotAggregation,
}

// ---------------------------------------------------------------------------
// PivotTable
// ---------------------------------------------------------------------------

/// Resultado da computação de uma tabela dinâmica.
///
/// Contém os rótulos de linha e coluna, a matriz de células agregadas,
/// totais por linha, totais por coluna e o total geral.
#[derive(Clone, Debug)]
pub struct PivotTable {
    /// Valores únicos do campo de linha, ordenados alfabeticamente.
    pub row_labels: Vec<String>,
    /// Valores únicos do campo de coluna, ordenados alfabeticamente.
    pub column_labels: Vec<String>,
    /// Matriz de valores agregados indexada como `[linha][coluna]`.
    /// `None` indica que não há dados para aquela combinação.
    pub cells: Vec<Vec<Option<f64>>>,
    /// Total agregado para cada linha.
    pub row_totals: Vec<f64>,
    /// Total agregado para cada coluna.
    pub column_totals: Vec<f64>,
    /// Total geral de todos os valores.
    pub grand_total: f64,
}

impl PivotTable {
    /// Retorna o número de linhas da tabela dinâmica.
    pub fn row_count(&self) -> usize {
        self.row_labels.len()
    }

    /// Retorna o número de colunas da tabela dinâmica.
    pub fn column_count(&self) -> usize {
        self.column_labels.len()
    }

    /// Retorna o valor agregado na posição `(row, col)`, ou `None` se a
    /// posição estiver fora dos limites ou não houver dados.
    pub fn cell_value(&self, row: usize, col: usize) -> Option<f64> {
        self.cells.get(row).and_then(|r| r.get(col)).copied().flatten()
    }

    /// Retorna o valor formatado da célula na posição `(row, col)`.
    ///
    /// Retorna `"—"` quando não há valor disponível.
    pub fn formatted_cell(&self, row: usize, col: usize) -> String {
        match self.cell_value(row, col) {
            Some(v) => format_pivot_value(v),
            None => "—".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Funções de agregação
// ---------------------------------------------------------------------------

/// Despacha a agregação para a função correspondente ao tipo solicitado.
///
/// Retorna `None` quando a agregação exige pelo menos um valor e a
/// fatia está vazia (`Average`, `Min`, `Max`, `Median`, `StdDev`).
pub fn aggregate(values: &[f64], aggregation: PivotAggregation) -> Option<f64> {
    match aggregation {
        PivotAggregation::Sum => Some(aggregate_sum(values)),
        PivotAggregation::Count => Some(aggregate_count(values)),
        PivotAggregation::Average => aggregate_average(values),
        PivotAggregation::Min => aggregate_min(values),
        PivotAggregation::Max => aggregate_max(values),
        PivotAggregation::Median => aggregate_median(values),
        PivotAggregation::StdDev => aggregate_stddev(values),
        PivotAggregation::CountDistinct => Some(aggregate_count_distinct(values)),
    }
}

/// Retorna a soma de todos os valores. Retorna `0.0` se a fatia estiver vazia.
pub fn aggregate_sum(values: &[f64]) -> f64 {
    values.iter().sum()
}

/// Retorna a contagem de valores como `f64`.
pub fn aggregate_count(values: &[f64]) -> f64 {
    values.len() as f64
}

/// Retorna a média aritmética dos valores, ou `None` se a fatia estiver vazia.
pub fn aggregate_average(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    Some(aggregate_sum(values) / values.len() as f64)
}

/// Retorna o valor mínimo, ou `None` se a fatia estiver vazia.
pub fn aggregate_min(values: &[f64]) -> Option<f64> {
    values
        .iter()
        .copied()
        .reduce(f64::min)
}

/// Retorna o valor máximo, ou `None` se a fatia estiver vazia.
pub fn aggregate_max(values: &[f64]) -> Option<f64> {
    values
        .iter()
        .copied()
        .reduce(f64::max)
}

/// Retorna a mediana dos valores, ou `None` se a fatia estiver vazia.
///
/// Ordena uma cópia dos valores e seleciona o elemento central.
/// Para quantidades pares, retorna a média dos dois elementos centrais.
pub fn aggregate_median(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let len = sorted.len();
    if len % 2 == 1 {
        Some(sorted[len / 2])
    } else {
        Some((sorted[len / 2 - 1] + sorted[len / 2]) / 2.0)
    }
}

/// Retorna o desvio padrão populacional, ou `None` se a fatia estiver vazia.
///
/// Utiliza a fórmula: `sqrt(Σ(xi - μ)² / N)`
pub fn aggregate_stddev(values: &[f64]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mean = aggregate_sum(values) / values.len() as f64;
    let variance = values
        .iter()
        .map(|v| {
            let diff = v - mean;
            diff * diff
        })
        .sum::<f64>()
        / values.len() as f64;
    Some(variance.sqrt())
}

/// Retorna a contagem de valores distintos.
///
/// Utiliza uma comparação com epsilon (`1e-10`) para lidar com
/// imprecisões de ponto flutuante.
pub fn aggregate_count_distinct(values: &[f64]) -> f64 {
    const EPSILON: f64 = 1e-10;

    if values.is_empty() {
        return 0.0;
    }

    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    let mut count: usize = 1;
    for i in 1..sorted.len() {
        if (sorted[i] - sorted[i - 1]).abs() > EPSILON {
            count += 1;
        }
    }
    count as f64
}

// ---------------------------------------------------------------------------
// Formatação
// ---------------------------------------------------------------------------

/// Formata um valor numérico para exibição.
///
/// Inteiros são exibidos sem casas decimais; demais valores com duas
/// casas decimais.
pub fn format_pivot_value(value: f64) -> String {
    if value.fract().abs() < 1e-9 {
        format!("{}", value as i64)
    } else {
        format!("{:.2}", value)
    }
}

// ---------------------------------------------------------------------------
// Computação da tabela dinâmica
// ---------------------------------------------------------------------------

/// Computa uma tabela dinâmica a partir de uma planilha importada e uma
/// configuração.
///
/// Itera sobre todas as linhas da planilha, agrupando os valores por
/// combinação de rótulo de linha e rótulo de coluna, e então aplica a
/// função de agregação configurada a cada grupo.
///
/// Valores não numéricos no campo de valor são silenciosamente ignorados.
pub fn compute_pivot(sheet: &ImportedSheet, config: &PivotConfig) -> PivotTable {
    // Mapa: (rótulo_linha, rótulo_coluna) -> vetor de valores
    let mut groups: BTreeMap<(String, String), Vec<f64>> = BTreeMap::new();

    // Conjuntos para rótulos únicos (BTreeSet garante ordenação)
    let mut row_label_set: BTreeSet<String> = BTreeSet::new();
    let mut col_label_set: BTreeSet<String> = BTreeSet::new();

    // Coleta de todos os valores válidos para o cálculo de totais
    let mut all_values: Vec<f64> = Vec::new();

    let col_count = sheet.column_count();

    for row in &sheet.rows {
        // Garante que os índices de campo estão dentro dos limites da linha
        if config.row_field >= col_count
            || config.column_field >= col_count
            || config.value_field >= col_count
        {
            break;
        }

        // Extrai os valores das colunas relevantes, pulando linhas curtas
        let row_label = match row.get(config.row_field) {
            Some(v) => v.clone(),
            None => continue,
        };
        let col_label = match row.get(config.column_field) {
            Some(v) => v.clone(),
            None => continue,
        };
        let raw_value = match row.get(config.value_field) {
            Some(v) => v,
            None => continue,
        };

        // Tenta converter o valor para f64; pula se não for numérico
        let value: f64 = match raw_value.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        row_label_set.insert(row_label.clone());
        col_label_set.insert(col_label.clone());
        all_values.push(value);

        groups
            .entry((row_label, col_label))
            .or_default()
            .push(value);
    }

    let row_labels: Vec<String> = row_label_set.into_iter().collect();
    let column_labels: Vec<String> = col_label_set.into_iter().collect();

    // Mapas de índice para acesso rápido
    let row_index: BTreeMap<&str, usize> = row_labels
        .iter()
        .enumerate()
        .map(|(i, l)| (l.as_str(), i))
        .collect();
    let col_index: BTreeMap<&str, usize> = column_labels
        .iter()
        .enumerate()
        .map(|(i, l)| (l.as_str(), i))
        .collect();

    let n_rows = row_labels.len();
    let n_cols = column_labels.len();

    // Inicializa a matriz de células
    let mut cells: Vec<Vec<Option<f64>>> = vec![vec![None; n_cols]; n_rows];

    // Acumula valores por linha e coluna para cálculo de totais
    let mut row_values: Vec<Vec<f64>> = vec![Vec::new(); n_rows];
    let mut col_values: Vec<Vec<f64>> = vec![Vec::new(); n_cols];

    for ((rl, cl), values) in &groups {
        let ri = match row_index.get(rl.as_str()) {
            Some(&i) => i,
            None => continue,
        };
        let ci = match col_index.get(cl.as_str()) {
            Some(&i) => i,
            None => continue,
        };

        let agg = aggregate(values, config.aggregation);
        cells[ri][ci] = agg;

        // Para os totais, coletamos os valores brutos do grupo
        row_values[ri].extend_from_slice(values);
        col_values[ci].extend_from_slice(values);
    }

    // Calcula totais por linha
    let row_totals: Vec<f64> = row_values
        .iter()
        .map(|vals| aggregate(vals, config.aggregation).unwrap_or(0.0))
        .collect();

    // Calcula totais por coluna
    let column_totals: Vec<f64> = col_values
        .iter()
        .map(|vals| aggregate(vals, config.aggregation).unwrap_or(0.0))
        .collect();

    // Calcula total geral
    let grand_total = aggregate(&all_values, config.aggregation).unwrap_or(0.0);

    PivotTable {
        row_labels,
        column_labels,
        cells,
        row_totals,
        column_totals,
        grand_total,
    }
}
