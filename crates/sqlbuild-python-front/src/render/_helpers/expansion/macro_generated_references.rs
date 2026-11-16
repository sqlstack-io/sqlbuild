//! Diagnostics for reference calls a macro returned: P012 for rejected calls, P006 when enforced.

use pyo3::FromPyObject;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyDict, PyTuple};
use sqlbuild_analysis::compiled_project::main::text_position::text_position;
use sqlbuild_analysis::compiled_project::models::TextPosition;
use sqlbuild_sqltext::sql_references::main::extract_sql_references::extract_sql_references;
use sqlbuild_sqltext::sql_references::models::{
    InvalidReferenceCall, ReferenceExtraction, ReferenceScan,
};
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

use crate::sqltext::models::LexicalSyntaxInput;
use sqlbuild_python_base::compile_context::main::diagnostic_collector::report_compile_diagnostic::report_compile_diagnostic;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const REFERENCE_CALL_SYNTAX_CODE: &str = "P012";
const MACRO_GENERATED_REFERENCE_CODE: &str = "P006";
/// `(SqlReferenceKind value, SqlResourceRefKind value, typed reference function)`.
const EXPLICIT_REFERENCES: [(&str, &str, &str); 3] = [
    ("ref", "model", "__ref"),
    ("source", "source", "__source"),
    ("seed", "seed", "__seed"),
];
/// Consumer resource kinds that are compiled resource types of the same value.
const RESOURCE_TYPES: [&str; 3] = ["model", "source", "seed"];
const MODEL_KIND: &str = "model";

/// What the diagnostics are built and reported with.
#[derive(FromPyObject)]
#[pyo3(from_item_all)]
pub(crate) struct GeneratedReferenceClasses<'py> {
    compiler_diagnostic: Bound<'py, PyAny>,
    source_location: Bound<'py, PyAny>,
    resource_identity: Bound<'py, PyAny>,
    generic_syntax: LexicalSyntaxInput,
    /// `explicit_references_help(allowed="macro-generated references")`.
    generated_help: String,
    /// `function(` of every reference kind.
    all_markers: Vec<String>,
}

impl GeneratedReferenceClasses<'_> {
    /// `function(` of every reference kind.
    pub(crate) fn markers(&self) -> impl Iterator<Item = &str> {
        self.all_markers.iter().map(String::as_str)
    }
}

/// The expansion a macro's result was returned to: its file, consumer and enforcement.
pub(crate) struct GeneratedOrigin<'a, 'py> {
    pub(crate) file_path: &'a Bound<'py, PyAny>,
    pub(crate) consumer: &'a Bound<'py, PyAny>,
    pub(crate) enforce_explicit: bool,
}

/// `report_macro_generated_references`.
pub(crate) fn report_generated_references(
    classes: &GeneratedReferenceClasses<'_>,
    origin: &GeneratedOrigin<'_, '_>,
    loaded_macro: &Bound<'_, PyAny>,
    result: &str,
) -> PyResult<()> {
    if !classes.markers().any(|marker| result.contains(marker)) {
        return Ok(());
    }
    let syntax: LexicalSyntax = classes.generic_syntax.clone().into();
    let ReferenceExtraction::Extracted(scan) = extract_sql_references(result, &syntax) else {
        return Ok(());
    };
    let reporter: Reporter<'_, '_> = Reporter {
        classes,
        origin,
        loaded_macro,
        macro_name: native_field(loaded_macro, "name")?.extract()?,
        resource: resource_kind(classes, origin.consumer)?,
    };
    for invalid_call in &scan.invalid_calls {
        reporter.rejected_call(invalid_call)?;
    }
    if origin.enforce_explicit
        && EXPLICIT_REFERENCES
            .iter()
            .any(|(_, _, function)| result.contains(&format!("{function}(")))
    {
        reporter.generated(&scan)?;
    }
    Ok(())
}

/// The consumer's resource kind when it is a `ResourceIdentity`.
fn resource_kind(
    classes: &GeneratedReferenceClasses<'_>,
    consumer: &Bound<'_, PyAny>,
) -> PyResult<Option<(String, String)>> {
    if consumer.is_none() || !consumer.is_instance(&classes.resource_identity)? {
        return Ok(None);
    }
    Ok(Some((
        native_field(&native_field(consumer, "kind")?, "value")?.extract()?,
        native_field(consumer, "name")?.extract()?,
    )))
}

struct Reporter<'a, 'py> {
    classes: &'a GeneratedReferenceClasses<'py>,
    origin: &'a GeneratedOrigin<'a, 'py>,
    loaded_macro: &'a Bound<'py, PyAny>,
    macro_name: String,
    resource: Option<(String, String)>,
}

impl<'py> Reporter<'_, 'py> {
    fn py(&self) -> Python<'py> {
        self.loaded_macro.py()
    }

    /// `"the model"` for a model consumer, else `"the calling SQL"`.
    fn location_label(&self) -> &'static str {
        match &self.resource {
            Some((kind, _)) if kind == MODEL_KIND => "the model",
            _ => "the calling SQL",
        }
    }

    /// `report_macro_reference_call_syntax` for one rejected call.
    fn rejected_call(&self, invalid_call: &InvalidReferenceCall) -> PyResult<()> {
        let py: Python<'py> = self.py();
        let name: &str = &self.macro_name;
        let corrected: &str = &invalid_call.corrected_call;
        let pass_in_help: String = if EXPLICIT_REFERENCES
            .iter()
            .any(|(kind, _, _)| *kind == invalid_call.kind)
        {
            format!(
                "write {corrected} in {} and pass it in: @py.{name}({corrected})\n  = help: ",
                self.location_label()
            )
        } else {
            String::new()
        };
        let fields: Bound<'py, PyDict> = self.fields(
            REFERENCE_CALL_SYNTAX_CODE,
            &format!("{}, returned by macro {name}()", invalid_call.message),
            &format!("{pass_in_help}{}", invalid_call.help),
        )?;
        fields.set_item("location", self.source_location(&invalid_call.call)?)?;
        let owner: String = self.origin.file_path.call_method0("as_posix")?.extract()?;
        let key: Bound<'py, PyTuple> =
            PyTuple::new(py, [REFERENCE_CALL_SYNTAX_CODE, &owner, &invalid_call.call])?;
        self.report(key, &fields)
    }

    /// `reject_macro_generated_references`: P006 for each model, source or seed it generated.
    fn generated(&self, scan: &ReferenceScan) -> PyResult<()> {
        let py: Python<'py> = self.py();
        let consumer_label: String = if self.origin.consumer.is_none() {
            format!("'{}'", self.origin.file_path.str()?)
        } else {
            format!(
                "{}:{}",
                self.origin
                    .consumer
                    .getattr("kind")?
                    .getattr("value")?
                    .extract::<String>()?,
                self.origin.consumer.getattr("name")?.extract::<String>()?
            )
        };
        let mut generated: Vec<(&str, &str, &str)> = Vec::new();
        for reference in &scan.references {
            let Some((_, kind, function)) = EXPLICIT_REFERENCES
                .iter()
                .find(|(reference_kind, _, _)| *reference_kind == reference.kind)
            else {
                continue;
            };
            let entry: (&str, &str, &str) = (kind, reference.name.as_str(), function);
            if !generated.contains(&entry) {
                generated.push(entry);
            }
        }
        let name: &str = &self.macro_name;
        for (kind, reference, function) in generated {
            let fields: Bound<'py, PyDict> = self.fields(
                MACRO_GENERATED_REFERENCE_CODE,
                &format!("{consumer_label} depends on {kind}:{reference} through macro {name}()"),
                &format!(
                    "write the reference in {}, or pass it in: @py.{name}({function}(\"{reference}\"))\n  = help: {}",
                    self.location_label(),
                    self.classes.generated_help
                ),
            )?;
            let key: Bound<'py, PyTuple> = PyTuple::new(
                py,
                [
                    MACRO_GENERATED_REFERENCE_CODE,
                    &consumer_label,
                    name,
                    kind,
                    reference,
                ],
            )?;
            self.report(key, &fields)?;
        }
        Ok(())
    }

    /// A compile-phase error at the macro file, attributed to the consumer resource.
    fn fields(&self, code: &str, message: &str, help: &str) -> PyResult<Bound<'py, PyDict>> {
        let fields: Bound<'py, PyDict> = PyDict::new(self.py());
        fields.set_item("phase", "compile")?;
        fields.set_item("severity", "error")?;
        fields.set_item("code", code)?;
        fields.set_item("message", message)?;
        match &self.resource {
            Some((kind, name)) if RESOURCE_TYPES.contains(&kind.as_str()) => {
                fields.set_item("resource_type", kind)?;
                fields.set_item("resource_name", name)?;
            }
            _ => {}
        }
        fields.set_item("path", self.loaded_macro.getattr("relative_path")?)?;
        fields.set_item("help", help)?;
        Ok(fields)
    }

    /// `_macro_source_location`: the call's first occurrence in the macro's source, if any.
    fn source_location(&self, call: &str) -> PyResult<Bound<'py, PyAny>> {
        let source: String = self.loaded_macro.getattr("raw_source")?.extract()?;
        let Some(byte_start) = source.find(call) else {
            return Ok(self.py().None().into_bound(self.py()));
        };
        let start: usize = source[..byte_start].chars().count();
        let begin: TextPosition = text_position(&source, start);
        let end: TextPosition = text_position(&source, start + call.chars().count());
        let location: Bound<'py, PyDict> = PyDict::new(self.py());
        location.set_item("path", self.loaded_macro.getattr("relative_path")?)?;
        location.set_item("line", begin.line)?;
        location.set_item("column", begin.column)?;
        location.set_item("end_line", end.line)?;
        location.set_item("end_column", end.column)?;
        self.classes.source_location.call((), Some(&location))
    }

    fn report(&self, key: Bound<'py, PyTuple>, fields: &Bound<'py, PyDict>) -> PyResult<()> {
        report_compile_diagnostic(
            key.as_any(),
            &self.classes.compiler_diagnostic.call((), Some(fields))?,
        )
    }
}
