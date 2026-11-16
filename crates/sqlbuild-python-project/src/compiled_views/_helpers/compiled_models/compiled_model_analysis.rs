//! A compiled model's analysis, kept from the native session and projected on demand.

use crate::compiled_views::models::AnalysisProjections;
use sqlbuild_python_base::positional_record;
use std::borrow::Cow;
use std::collections::HashMap;

use pyo3::exceptions::PyValueError;
use pyo3::prelude::{Bound, Py, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyDictMethods, PyString, PyTuple};
use pyo3::{IntoPyObject, IntoPyObjectExt};
use sqlbuild_analysis::assembly::analysis_session::main::resolved_cleaned_sql::resolved_cleaned_sql;
use sqlbuild_analysis::assembly::analysis_session::models::{
    ColumnFact, ContractProof, LineageFacts, LineageRow, ModelOutcome, PivotOutcome,
};
use sqlbuild_analysis::binding_locations::main::locate_binding_diagnostics::locate_binding_diagnostics;
use sqlbuild_analysis::binding_locations::models::{
    AuthoredSpan, BindingSource, LocatedDiagnostic, RelatedSpot,
};
use sqlbuild_analysis::semantic_validation::models::{DiagnosticRow, Expansion};
use sqlbuild_analysis::sql_macros::constants::{ARGUMENT_CODE, SUPERSEDED_ANALYSIS_CODE};
use sqlbuild_analysis::sql_macros::main::marker_free_text::marker_free_text;
use sqlbuild_analysis::sql_macros::main::typed_analysis_pass::typed_analysis_pass;
use sqlbuild_analysis::sql_macros::models::SqlMacroFinding;

use sqlbuild_project::models as project_models;
use sqlbuild_project::types::InferredRow;

use crate::analysis_session::models::NativeModelAnalysisSession;
use crate::compiled_views::models::CompiledViewClasses;
use crate::compiled_views::models::PyProjectHost;
use crate::compiled_views::types::ModelInputRecord;
use crate::lineage::main::confidence_member::confidence_member;
use crate::lineage::main::transform_kind_member::transform_kind_member;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_front::discovery::models::NativeModelFile;
use sqlbuild_python_front::render::main::calls::span_expansion::span_expansion;

const COMPILE_PHASE: &str = "compile";
const ERROR_SEVERITY: &str = "error";
const PARAMETER_LABEL: &str = "parameter declared here";
const MODEL_RESOURCE: &str = "model";

positional_record! {
    extract;
    /// `(output proven, fixed columns, (family, inferred type), inputs, failure, bare pivot)`.
    pub(crate) struct ProofArgument {
        output_proven: bool,
        fixed: Vec<InferredRow>,
        families: Vec<(String, Option<String>)>,
        input_relations: Vec<String>,
        failure_reason: Option<String>,
        bare_dynamic_pivot: bool,
    }
}

/// The session analysis a model was assembled from: the session and its request position.
pub(crate) type SessionArgument = (Py<NativeModelAnalysisSession>, usize);

/// What one model's analysis left for its compiled model.
pub(crate) type AnalysisRecord = project_models::AnalysisRecord<PyProjectHost>;

fn inferred_row(column: &ColumnFact) -> InferredRow {
    (
        column.name.clone(),
        column.data_type.clone(),
        column.nullability.clone(),
    )
}

fn proof_fact(argument: ProofArgument) -> ContractProof {
    let ProofArgument {
        output_proven,
        fixed,
        families,
        input_relations,
        failure_reason,
        bare_dynamic_pivot,
    } = argument;
    ContractProof {
        output_proven,
        fixed_columns: fixed
            .into_iter()
            .map(|(name, data_type, nullability)| ColumnFact {
                name,
                data_type,
                nullability,
            })
            .collect(),
        families,
        input_relations,
        failure_reason,
        bare_dynamic_pivot,
    }
}

/// The model's analysis: the session's outcome, a failed analysis's fallback columns, the proof.
pub(crate) fn analysis_record(
    py: Python<'_>,
    input: &ModelInputRecord,
    analysis: Option<SessionArgument>,
    (fallback, standalone_proof): (Option<Vec<InferredRow>>, Option<ProofArgument>),
) -> PyResult<AnalysisRecord> {
    let mut record: AnalysisRecord = AnalysisRecord {
        inferred: None,
        lineage: None,
        has_star: false,
        star_resolved: false,
        binding: Vec::new(),
        binding_validated: false,
        proof: standalone_proof.map(proof_fact),
        host: AnalysisProjections::default(),
    };
    if let Some((session, index)) = analysis {
        let session = session.borrow(py);
        let (outcome, contract): &(ModelOutcome, PivotOutcome) = session.outcome(index)?;
        let analysed = &outcome.analysis;
        if analysed.analysis_succeeded {
            record.inferred = analysed
                .columns
                .as_ref()
                .map(|columns| columns.iter().map(inferred_row).collect());
            record.lineage = Some(analysed.lineage.clone());
            record.has_star = analysed.has_star;
            record.star_resolved = analysed.star_resolved;
        } else {
            record.inferred = fallback;
        }
        if let PivotOutcome::Proof(proof) = contract {
            record.proof = Some(proof.clone());
        }
        record.binding =
            with_sql_macro_findings(located(input, outcome, session.dialect())?, outcome);
        record.binding_validated = analysed.binding_validated;
    }
    if let Some(proof) = record.proof.as_ref().filter(|proof| proof.output_proven) {
        record.inferred = Some(proof.fixed_columns.iter().map(inferred_row).collect());
        record.has_star = true;
    }
    Ok(record)
}

/// The model's binding diagnostics at their authored locations.
fn located(
    input: &ModelInputRecord,
    outcome: &ModelOutcome,
    dialect: &str,
) -> PyResult<Vec<LocatedDiagnostic>> {
    let rows: &[DiagnosticRow] = &outcome.analysis.binding_diagnostics;
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let cleaned: Cow<'_, str> = resolved_cleaned_sql(outcome).map_err(compiler_error)?;
    let model_file: &NativeModelFile = input.host.model_file.get();
    let authored: &str = model_file.contents_text();
    let query: &str = model_file.query_text();
    let expansion = &input.expansion;
    let mut typed_text: Option<String> = None;
    let typed: Option<(String, Vec<Expansion>)> = if *expansion.expanded_sql == *input.query_sql {
        typed_analysis_pass(&expansion.expanded_sql, &input.typed_calls, dialect)
    } else {
        None
    };
    let mut passes: Vec<Vec<Expansion>> = vec![
        expansion
            .declaration_spans
            .iter()
            .copied()
            .map(span_expansion)
            .collect(),
        expansion
            .macro_spans
            .iter()
            .copied()
            .map(span_expansion)
            .collect(),
    ];
    let expanded: &str = match typed {
        Some((typed_sql, pass)) => {
            passes.push(pass);
            typed_text.insert(typed_sql)
        }
        None => &expansion.expanded_sql,
    };
    let source: BindingSource<'_> = BindingSource {
        authored,
        query,
        cleaned: &cleaned,
        expansion: expansion.authored.then_some((expanded, passes.as_slice())),
    };
    let mut located: Vec<LocatedDiagnostic> =
        locate_binding_diagnostics(rows, &source).map_err(PyValueError::new_err)?;
    for diagnostic in &mut located {
        diagnostic.message = marker_free_text(&diagnostic.message).into_owned();
        if let Some(help) = diagnostic.help.as_mut() {
            *help = marker_free_text(help).into_owned();
        }
    }
    Ok(located)
}

/// The SQL macro findings, then the binding diagnostics without unknown columns inside a call.
fn with_sql_macro_findings(
    binding: Vec<LocatedDiagnostic>,
    outcome: &ModelOutcome,
) -> Vec<LocatedDiagnostic> {
    let findings: &[SqlMacroFinding] = &outcome.sql_macro_findings;
    if findings.is_empty() {
        return binding;
    }
    let inside_call = |diagnostic: &LocatedDiagnostic| {
        let (Some(line), Some(column)) = (diagnostic.line, diagnostic.column) else {
            return false;
        };
        findings.iter().any(|finding| {
            (finding.call.line, finding.call.column) <= (line, column)
                && (line, column) <= finding.call_end
        })
    };
    let mut kept: Vec<LocatedDiagnostic> = findings.iter().map(finding_diagnostic).collect();
    kept.extend(binding.into_iter().filter(|diagnostic| {
        diagnostic.code != SUPERSEDED_ANALYSIS_CODE || !inside_call(diagnostic)
    }));
    kept
}

/// One SQL macro finding as a located compile error at the call, noting the parameter.
fn finding_diagnostic(finding: &SqlMacroFinding) -> LocatedDiagnostic {
    LocatedDiagnostic {
        code: ARGUMENT_CODE.to_owned(),
        message: finding.message.clone(),
        severity: ERROR_SEVERITY.to_owned(),
        line: Some(finding.call.line),
        column: Some(finding.call.column),
        location: Some(AuthoredSpan {
            line: finding.call.line,
            column: finding.call.column,
            end_line: Some(finding.call_end.0),
            end_column: Some(finding.call_end.1),
        }),
        help: finding.help.clone(),
        related: Some(RelatedSpot {
            label: PARAMETER_LABEL.to_owned(),
            path: finding.declared_at.path.clone(),
            line: finding.declared_at.line,
            column: finding.declared_at.column,
        }),
    }
}

/// The Python objects a model analysis record projects, each built once.
pub(crate) trait AnalysisViews {
    /// The session's lineage as `CompactLineageFacts` or plain facts, or `None`.
    fn lineage_object(
        &self,
        py: Python<'_>,
        classes: &CompiledViewClasses<'_>,
    ) -> PyResult<Py<PyAny>>;

    /// The model's `CompilerDiagnostic` tuple, built once.
    fn binding_tuple(
        &self,
        py: Python<'_>,
        classes: &CompiledViewClasses<'_>,
        model: (&str, &Bound<'_, PyAny>),
    ) -> PyResult<Py<PyTuple>>;

    /// The model's `DynamicColumnContractProof`, or `None`.
    fn proof_object(
        &self,
        py: Python<'_>,
        classes: &CompiledViewClasses<'_>,
    ) -> PyResult<Py<PyAny>>;
}

impl AnalysisViews for AnalysisRecord {
    /// The session's lineage as `CompactLineageFacts` or plain facts, or `None`.
    fn lineage_object(
        &self,
        py: Python<'_>,
        classes: &CompiledViewClasses<'_>,
    ) -> PyResult<Py<PyAny>> {
        self.host
            .lineage
            .get_or_try_init(py, || match &self.lineage {
                None => Ok(py.None()),
                Some(LineageFacts::Native(rows)) => compact_lineage(classes, rows),
                Some(LineageFacts::NativeFacts(rows)) => lineage_facts(classes, rows),
            })
            .map(|lineage| lineage.clone_ref(py))
    }

    /// The model's `CompilerDiagnostic` tuple, built once.
    fn binding_tuple(
        &self,
        py: Python<'_>,
        classes: &CompiledViewClasses<'_>,
        model: (&str, &Bound<'_, PyAny>),
    ) -> PyResult<Py<PyTuple>> {
        self.host
            .binding
            .get_or_try_init(py, || {
                let mut diagnostics: Vec<Py<PyAny>> = Vec::with_capacity(self.binding.len());
                for diagnostic in &self.binding {
                    diagnostics.push(compiler_diagnostic(classes, diagnostic, model)?);
                }
                Ok::<_, pyo3::PyErr>(PyTuple::new(py, diagnostics)?.unbind())
            })
            .map(|diagnostics| diagnostics.clone_ref(py))
    }

    /// The model's `DynamicColumnContractProof`, or `None`.
    fn proof_object(
        &self,
        py: Python<'_>,
        classes: &CompiledViewClasses<'_>,
    ) -> PyResult<Py<PyAny>> {
        self.host
            .proof
            .get_or_try_init(py, || match &self.proof {
                None => Ok(py.None()),
                Some(proof) => contract_proof(classes, proof),
            })
            .map(|proof| proof.clone_ref(py))
    }
}

/// `compact_lineage`: rows over a per-model string pool, sources pooled before each output.
fn compact_lineage(classes: &CompiledViewClasses<'_>, rows: &[LineageRow]) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = classes.compact_lineage_facts.py();
    let mut pool: StringPool<'_> = StringPool::default();
    let mut compact: Vec<Bound<'_, PyTuple>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut sources: Vec<(usize, usize, usize)> = Vec::with_capacity(row.sources.len());
        for (resource_type, resource_name, column_name) in &row.sources {
            let kind: usize = pool.index(resource_type);
            let name: usize = pool.index(resource_name);
            sources.push((kind, name, pool.index(column_name)));
        }
        let output: usize = pool.index(&row.output_column);
        compact.push(
            (
                output,
                row.transform_code,
                row.confidence_code,
                PyTuple::new(py, sources)?,
            )
                .into_pyobject(py)?,
        );
    }
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    fields.set_item("string_pool", PyTuple::new(py, pool.ordered)?)?;
    fields.set_item("rows", PyTuple::new(py, compact)?)?;
    Ok(classes
        .compact_lineage_facts
        .call((), Some(&fields))?
        .unbind())
}

/// Python's `pool.setdefault(text, len(pool))`, remembering first-seen order.
#[derive(Default)]
struct StringPool<'a> {
    indexes: HashMap<&'a str, usize>,
    ordered: Vec<&'a str>,
}

impl<'a> StringPool<'a> {
    fn index(&mut self, text: &'a str) -> usize {
        if let Some(index) = self.indexes.get(text) {
            return *index;
        }
        self.ordered.push(text);
        self.indexes.insert(text, self.ordered.len() - 1);
        self.ordered.len() - 1
    }
}

/// `lineage_facts`: plain `CompiledLineageColumnFact`s of a re-analysis.
fn lineage_facts(classes: &CompiledViewClasses<'_>, rows: &[LineageRow]) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = classes.lineage_column_fact.py();
    let mut facts: Vec<Py<PyAny>> = Vec::with_capacity(rows.len());
    for row in rows {
        let mut sources: Vec<Py<PyAny>> = Vec::with_capacity(row.sources.len());
        for (resource_type, resource_name, column_name) in &row.sources {
            let fields: Bound<'_, PyDict> = PyDict::new(py);
            fields.set_item("resource_type", resource_type)?;
            fields.set_item("resource_name", resource_name)?;
            fields.set_item("column_name", column_name)?;
            sources.push(
                classes
                    .lineage_source_fact
                    .call((), Some(&fields))?
                    .unbind(),
            );
        }
        let fields: Bound<'_, PyDict> = PyDict::new(py);
        fields.set_item("output_column", &row.output_column)?;
        fields.set_item("upstream_columns", PyTuple::new(py, sources)?)?;
        fields.set_item(
            "transform_kind",
            transform_kind_member(py, usize::from(row.transform_code))?,
        )?;
        fields.set_item(
            "confidence",
            confidence_member(py, usize::from(row.confidence_code))?,
        )?;
        facts.push(
            classes
                .lineage_column_fact
                .call((), Some(&fields))?
                .unbind(),
        );
    }
    Ok(PyTuple::new(py, facts)?.into_any().unbind())
}

/// One located binding diagnostic as the model's `CompilerDiagnostic`.
fn compiler_diagnostic(
    classes: &CompiledViewClasses<'_>,
    diagnostic: &LocatedDiagnostic,
    (name, path): (&str, &Bound<'_, PyAny>),
) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = path.py();
    let location: Option<Bound<'_, PyAny>> = match &diagnostic.location {
        None => None,
        Some(span) => {
            let fields: Bound<'_, PyDict> = PyDict::new(py);
            fields.set_item("path", path)?;
            fields.set_item("line", span.line)?;
            fields.set_item("column", span.column)?;
            fields.set_item("end_line", span.end_line)?;
            fields.set_item("end_column", span.end_column)?;
            Some(classes.source_location.call((), Some(&fields))?)
        }
    };
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    fields.set_item("phase", COMPILE_PHASE)?;
    fields.set_item("severity", &diagnostic.severity)?;
    fields.set_item("code", &diagnostic.code)?;
    fields.set_item("message", &diagnostic.message)?;
    fields.set_item("resource_type", MODEL_RESOURCE)?;
    fields.set_item("resource_name", name)?;
    fields.set_item("path", path)?;
    fields.set_item("line", diagnostic.line)?;
    fields.set_item("column", diagnostic.column)?;
    fields.set_item("location", location)?;
    if let Some(help) = &diagnostic.help {
        fields.set_item("help", help)?;
    }
    if let Some(RelatedSpot {
        label,
        path: related_path,
        line,
        column,
    }) = &diagnostic.related
    {
        let place: Bound<'_, PyAny> = new_record_with(
            py,
            RecordId::SourceLocation,
            &[
                (
                    "path",
                    std::path::PathBuf::from(related_path).into_bound_py_any(py)?,
                ),
                ("line", line.into_bound_py_any(py)?),
                ("column", column.into_bound_py_any(py)?),
            ],
        )?;
        let related: Bound<'_, PyAny> = new_record_with(
            py,
            RecordId::RelatedLocation,
            &[
                ("label", PyString::new(py, label).into_any()),
                ("location", place),
            ],
        )?;
        fields.set_item("related_locations", PyTuple::new(py, [related])?)?;
    }
    Ok(classes
        .compiler_diagnostic
        .call((), Some(&fields))?
        .unbind())
}

/// `contract_proof`: the session's dynamic pivot proof as Python's.
fn contract_proof(classes: &CompiledViewClasses<'_>, proof: &ContractProof) -> PyResult<Py<PyAny>> {
    let py: Python<'_> = classes.contract_proof.py();
    let mut fixed: Vec<Py<PyAny>> = Vec::with_capacity(proof.fixed_columns.len());
    for column in &proof.fixed_columns {
        let fields: Bound<'_, PyDict> = PyDict::new(py);
        fields.set_item("name", &column.name)?;
        fields.set_item("type", &column.data_type)?;
        fields.set_item(
            "nullability",
            classes.nullability.call1((&column.nullability,))?,
        )?;
        fixed.push(classes.inferred_column.call((), Some(&fields))?.unbind());
    }
    let mut families: Vec<Py<PyAny>> = Vec::with_capacity(proof.families.len());
    for (name, inferred_type) in &proof.families {
        let fields: Bound<'_, PyDict> = PyDict::new(py);
        fields.set_item("name", name)?;
        fields.set_item("inferred_type", inferred_type)?;
        families.push(classes.family_proof.call((), Some(&fields))?.unbind());
    }
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    fields.set_item("output_proven", proof.output_proven)?;
    fields.set_item("fixed_columns", PyTuple::new(py, fixed)?)?;
    fields.set_item("families", PyTuple::new(py, families)?)?;
    fields.set_item("input_relations", PyTuple::new(py, &proof.input_relations)?)?;
    fields.set_item("failure_reason", &proof.failure_reason)?;
    fields.set_item("bare_dynamic_pivot", proof.bare_dynamic_pivot)?;
    Ok(classes.contract_proof.call((), Some(&fields))?.unbind())
}
