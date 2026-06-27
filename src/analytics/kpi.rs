use crate::analytics::excel::ImportedSheet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KpiKind {
    Global,
    NumericColumn,
}

#[derive(Clone, Debug)]
pub struct KpiDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub kind: KpiKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct KpiResult {
    pub label: String,
    pub value: String,
}

pub fn default_catalog() -> Vec<KpiDefinition> {
    vec![
        KpiDefinition {
            id: "total_rows",
            label: "Total de linhas",
            description: "Quantidade total de registros",
            kind: KpiKind::Global,
        },
        KpiDefinition {
            id: "missing_values",
            label: "Valores ausentes",
            description: "Total de celulas vazias",
            kind: KpiKind::Global,
        },
        KpiDefinition {
            id: "sum",
            label: "Soma",
            description: "Soma dos valores da coluna numerica",
            kind: KpiKind::NumericColumn,
        },
        KpiDefinition {
            id: "average",
            label: "Media",
            description: "Media da coluna numerica",
            kind: KpiKind::NumericColumn,
        },
        KpiDefinition {
            id: "min",
            label: "Minimo",
            description: "Menor valor da coluna numerica",
            kind: KpiKind::NumericColumn,
        },
        KpiDefinition {
            id: "max",
            label: "Maximo",
            description: "Maior valor da coluna numerica",
            kind: KpiKind::NumericColumn,
        },
    ]
}

pub fn compute_selected(
    sheet: &ImportedSheet,
    numeric_column: Option<usize>,
    catalog: &[KpiDefinition],
    enabled: &[bool],
) -> Vec<KpiResult> {
    let selected_column_values = numeric_column
        .map(|index| sheet.numeric_values(index))
        .unwrap_or_default();

    let mut results = Vec::new();
    for (kpi, enabled) in catalog.iter().zip(enabled.iter().copied()) {
        if !enabled {
            continue;
        }

        let value = match kpi.id {
            "total_rows" => format!("{}", sheet.row_count()),
            "missing_values" => format!("{}", sheet.missing_values_count()),
            "sum" => format_number(selected_column_values.iter().sum::<f64>()),
            "average" => {
                if selected_column_values.is_empty() {
                    "N/A".to_string()
                } else {
                    format_number(
                        selected_column_values.iter().sum::<f64>() / selected_column_values.len() as f64,
                    )
                }
            }
            "min" => selected_column_values
                .iter()
                .copied()
                .reduce(f64::min)
                .map(format_number)
                .unwrap_or_else(|| "N/A".to_string()),
            "max" => selected_column_values
                .iter()
                .copied()
                .reduce(f64::max)
                .map(format_number)
                .unwrap_or_else(|| "N/A".to_string()),
            _ => "N/A".to_string(),
        };

        results.push(KpiResult {
            label: kpi.label.to_string(),
            value,
        });
    }

    results
}

fn format_number(value: f64) -> String {
    if (value.fract() - 0.0).abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
    }
}
