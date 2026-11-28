//! Column-by-column schema comparison state.

use std::collections::HashSet;

use crate::planner_changes::models::{
    BackfillFacts, SchemaChangeKind, SchemaColumnSource, SchemaColumns, SchemaFindingRow,
};

/// Whether schema findings replay the policy: only alongside a definition, config or declared-column change.
pub(crate) fn schema_replays(facts: BackfillFacts) -> bool {
    facts.schema_changed
        && (facts.query_changed
            || facts.functions_changed
            || facts.config_changed
            || facts.declared_columns_changed)
}

/// Expected columns with optional types, and warehouse columns with dictionary semantics.
pub(crate) struct FoldedColumns {
    pub(crate) yml: Vec<(String, Option<String>)>,
    pub(crate) inferred: Option<Vec<(String, Option<String>)>>,
    pub(crate) warehouse: Vec<(String, String)>,
}

/// Lower-case names when the dialect folds them; later duplicate warehouse names replace earlier types in place.
pub(crate) fn folded_columns(columns: SchemaColumns, case_folding: bool) -> FoldedColumns {
    let fold = |name: String| {
        if case_folding {
            name.to_lowercase()
        } else {
            name
        }
    };
    let yml: Vec<(String, Option<String>)> = columns
        .yml
        .into_iter()
        .map(|(name, kind)| (fold(name), Some(kind)))
        .collect();
    let inferred: Option<Vec<(String, Option<String>)>> = columns.inferred.map(|items| {
        let mut folded: Vec<(String, Option<String>)> = Vec::with_capacity(items.len());
        for (name, kind) in items {
            folded.push((fold(name), kind));
        }
        folded
    });
    let mut warehouse: Vec<(String, String)> = Vec::new();
    for (name, kind) in columns.warehouse {
        let name: String = fold(name);
        match warehouse.iter_mut().find(|(existing, _)| *existing == name) {
            Some(entry) => entry.1 = kind,
            None => warehouse.push((name, kind)),
        }
    }
    FoldedColumns {
        yml,
        inferred,
        warehouse,
    }
}

/// Findings collected so far and the expected names already compared.
pub(crate) struct Comparer<'a, 'b, E> {
    pub(crate) warehouse: &'a [(String, String)],
    pub(crate) seen: HashSet<String>,
    pub(crate) findings: Vec<SchemaFindingRow>,
    /// Whether an expected column type, then a warehouse-listed one, name the same type.
    pub(crate) types_equal: &'b mut dyn FnMut(&str, &str) -> Result<bool, E>,
}

impl<E> Comparer<'_, '_, E> {
    /// Added and retyped columns; `skip_seen` skips names an earlier source already covered; then marks them seen.
    pub(crate) fn compare(
        &mut self,
        expected: &[(String, Option<String>)],
        source: SchemaColumnSource,
        skip_seen: bool,
    ) -> Result<(), E> {
        for (name, kind) in expected {
            if skip_seen && self.seen.contains(name) {
                continue;
            }
            match self.warehouse.iter().find(|(existing, _)| existing == name) {
                None => self.findings.push(finding(
                    (SchemaChangeKind::ColumnAdded, name, source),
                    kind.clone(),
                    None,
                )),
                Some((_, actual)) => {
                    if let Some(expected_type) = kind
                        && !(self.types_equal)(expected_type, actual)?
                    {
                        self.findings.push(finding(
                            (SchemaChangeKind::ColumnTypeChanged, name, source),
                            Some(expected_type.clone()),
                            Some(actual.clone()),
                        ));
                    }
                }
            }
        }
        self.seen
            .extend(expected.iter().map(|(name, _)| name.clone()));
        Ok(())
    }

    /// Retyped warehouse columns that exactly one dynamic family accepts.
    pub(crate) fn compare_families(
        &mut self,
        single_family_type: &mut dyn FnMut(&str) -> Result<Option<String>, E>,
    ) -> Result<(), E> {
        for (name, kind) in self.warehouse {
            if self.seen.contains(name) {
                continue;
            }
            let Some(family_type) = single_family_type(name)? else {
                continue;
            };
            self.seen.insert(name.clone());
            if !(self.types_equal)(&family_type, kind)? {
                self.findings.push(finding(
                    (
                        SchemaChangeKind::ColumnTypeChanged,
                        name,
                        SchemaColumnSource::Yml,
                    ),
                    Some(family_type),
                    Some(kind.clone()),
                ));
            }
        }
        Ok(())
    }

    /// Warehouse columns no expected column or family covered.
    pub(crate) fn report_removed(&mut self, source: SchemaColumnSource) {
        for (name, kind) in self.warehouse {
            if !self.seen.contains(name) {
                self.findings.push(finding(
                    (SchemaChangeKind::ColumnRemoved, name, source),
                    None,
                    Some(kind.clone()),
                ));
            }
        }
    }
}

fn finding(
    (kind, name, source): (SchemaChangeKind, &str, SchemaColumnSource),
    expected_type: Option<String>,
    actual_type: Option<String>,
) -> SchemaFindingRow {
    SchemaFindingRow {
        kind,
        column_name: name.to_owned(),
        source,
        expected_type,
        actual_type,
    }
}
