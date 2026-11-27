//! The native project assembly request built from compile inputs, and its resource facts.

use pyo3::IntoPyObject;
use pyo3::exceptions::{PyException, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use pyo3::types::{PyBool, PyBoolMethods, PyInt, PyString, PyStringMethods, PyTuple};
use sqlbuild_analysis::assembly::analysis_session::types::Pairs;
use sqlbuild_analysis::assembly::project::errors::AssemblyError;
use sqlbuild_analysis::assembly::project::main::environment_template_names::environment_template_names;
use sqlbuild_analysis::assembly::project::models::{
    AuditFacts, FunctionFacts, InputRead, ModelFacts, Namespace, OpaqueRender, OpaqueVariable,
    ProjectRequest, ProjectResources, Reference, ResourceFailure, ResourceSlot, SeedDefaults,
    SeedFacts, SourceFacts, SyntaxCheck, TargetNamespace, Variable,
};
use sqlbuild_core::panics::main::native_failure::native_failure;
use sqlbuild_model_config::templates::models::Scalar;
use sqlbuild_python_base::positional_record;
use sqlbuild_python_base::records::main::fields::tag_texts::tag_texts;

use crate::project_assembly::models::NativeProjectResources;
use crate::project_assembly::models::NativeResourceFailure;
use sqlbuild_project::models::ReferenceRow;
use sqlbuild_python_base::boundary::main::compiler_error::compiler_error;
use sqlbuild_python_base::boundary::main::imported::imported;
use sqlbuild_python_base::boundary::main::keywords::keywords;
use sqlbuild_python_base::boundary::main::native_compiler_error::native_compiler_error;
use sqlbuild_python_base::records::main::construct::error_class::error_class;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_base::records::main::fields::mapping_get::mapping_get;
use sqlbuild_python_base::records::main::fields::mapping_items::mapping_items;
use sqlbuild_python_base::records::types::ErrorId;
use sqlbuild_python_front::authored_values::main::project_var_text::project_var_text;
use sqlbuild_python_front::model_config::main::config::record_reads::record_reads;
use sqlbuild_python_project::compiled_views::main::model_inputs::input_reference_rows::input_reference_rows;

const OS_MODULE: &str = "os";
pub(crate) const PROJECT_ASSEMBLY_CONTEXT: &str = "native project assembly";
const GENERIC_DIALECT: &str = "generic";
const RENDER_LABEL: &str = "\u{0}";
const SOURCE_SLOT: &str = "source";
const SEED_SLOT: &str = "seed";
const AUDIT_SLOT: &str = "audit";
const ENVIRONMENT_READ: &str = "env";
const CONTEXT_READ: &str = "ctx";
const NATIVE_FAILURE_PREFIX: &str = "NativeCompilerError: ";

/// `(kind, name, package)`.
type ResourceReferenceRow = (String, String, Option<String>);
/// `(references, attached (kind, name))`.
type AuditRow = (Vec<ResourceReferenceRow>, Option<(String, String)>);

positional_record! {
    extract;
    /// `(name, managed, database, schema)`.
    struct SourceRow {
        name: String,
        managed: bool,
        database: Option<String>,
        schema: Option<String>,
    }
}

positional_record! {
    extract;
    /// `(kind, name, tags, references)`.
    struct FunctionRow {
        kind: String,
        name: String,
        tags: Vec<String>,
        references: Vec<ResourceReferenceRow>,
    }
}

positional_record! {
    into;
    /// `(database, schema, logical database, logical schema)` of one seed.
    struct NamespaceRow {
        database: Option<String>,
        schema: Option<String>,
        logical_database: Option<String>,
        logical_schema: Option<String>,
    }
}

/// The request's named variables and the Python exceptions opaque ones raise, by index.
type RequestVariables<'py> = (Vec<(String, Variable)>, Vec<Bound<'py, PyAny>>);

/// How one opaque variable renders as text, or the Python exception rendering it raises.
enum Render<'py> {
    Text(String),
    Rejected(String),
    Raises(Bound<'py, PyAny>),
}

/// The plain scalar `value` crosses as: `None`, exact `bool`, `str` or 64-bit `int`.
fn plain_scalar(value: &Bound<'_, PyAny>) -> PyResult<Option<Scalar>> {
    if value.is_none() {
        return Ok(Some(Scalar::Null));
    }
    if let Ok(flag) = value.cast_exact::<PyBool>() {
        return Ok(Some(Scalar::Bool(flag.is_true())));
    }
    if let Ok(text) = value.cast_exact::<PyString>() {
        return Ok(Some(Scalar::Text(text.to_str()?.to_owned())));
    }
    if value.is_exact_instance_of::<PyInt>() {
        return Ok(match value.extract::<i64>() {
            Ok(number) => Some(Scalar::Text(number.to_string())),
            Err(_) => None,
        });
    }
    Ok(None)
}

/// `render_project_var_text` under a label no message starts with otherwise.
fn opaque_render<'py>(value: &Bound<'py, PyAny>) -> PyResult<Render<'py>> {
    let py: Python<'py> = value.py();
    match project_var_text(value, RENDER_LABEL) {
        Ok(text) => Ok(Render::Text(text)),
        Err(error) if !error.is_instance_of::<PyException>(py) => Err(error),
        Err(error) => {
            let raised: Bound<'py, PyAny> = error.value(py).clone().into_any();
            if error.is_instance_of::<PyValueError>(py) {
                let message: String = raised.str()?.extract()?;
                if let Some(rest) = message.strip_prefix(RENDER_LABEL) {
                    return Ok(Render::Rejected(rest.to_owned()));
                }
            }
            Ok(Render::Raises(raised))
        }
    }
}

/// Each effective var as the native request reads it, and the exceptions opaque ones raise.
fn variables<'py>(effective_vars: &Bound<'py, PyAny>) -> PyResult<RequestVariables<'py>> {
    let mut rows: Vec<(String, Variable)> = Vec::new();
    let mut held: Vec<Bound<'py, PyAny>> = Vec::new();
    for item in mapping_items(effective_vars)?.try_iter()? {
        let (name, value): (String, Bound<'py, PyAny>) = item?.extract()?;
        if let Some(scalar) = plain_scalar(&value)? {
            rows.push((name, Variable::Scalar(scalar)));
            continue;
        }
        let render: OpaqueRender = match opaque_render(&value)? {
            Render::Text(text) => OpaqueRender::Text(text),
            Render::Rejected(message) => OpaqueRender::Rejected(message),
            Render::Raises(error) => {
                held.push(error);
                OpaqueRender::Raises(held.len() - 1)
            }
        };
        let text: String = value.str()?.extract::<String>()?;
        rows.push((name, Variable::Opaque(OpaqueVariable { text, render })));
    }
    Ok((rows, held))
}

fn optional_text(value: &Bound<'_, PyAny>, attribute: &str) -> PyResult<Option<String>> {
    native_field(value, attribute)?.extract()
}

/// Python's own `os.environ` lookup of every name an `ENV:` template may read.
fn environment_rows(
    py: Python<'_>,
    values: &[Option<String>],
) -> PyResult<Vec<(String, Option<String>)>> {
    let environment: Bound<'_, PyAny> = imported(py, OS_MODULE, "environ")?;
    environment_template_names(values)
        .into_iter()
        .map(|name| {
            let value: Option<String> = mapping_get(&environment, &name)?.extract()?;
            Ok((name, value))
        })
        .collect()
}

fn references(rows: Vec<ResourceReferenceRow>) -> Vec<Reference> {
    rows.into_iter()
        .map(|(kind, name, package)| Reference {
            kind,
            name,
            package,
        })
        .collect()
}

fn model_facts(model_input: &Bound<'_, PyAny>, checks: &Bound<'_, PyAny>) -> PyResult<ModelFacts> {
    let model_file: Bound<'_, PyAny> = native_field(model_input, "model_file")?;
    let reference_rows: Vec<ResourceReferenceRow> = input_reference_rows(model_input)?
        .into_iter()
        .map(
            |ReferenceRow {
                 kind,
                 name,
                 package,
                 ..
             }| (kind, name, package),
        )
        .collect();
    let mut syntax_checks: Vec<SyntaxCheck> = Vec::new();
    for check in checks.try_iter()? {
        let (sql, placeholders): (String, Bound<'_, PyAny>) = check?.extract()?;
        let placeholders: Pairs = if placeholders.is_truthy()? {
            mapping_items(&placeholders)?
                .try_iter()?
                .map(|pair| pair?.extract())
                .collect::<PyResult<_>>()?
        } else {
            Vec::new()
        };
        syntax_checks.push(SyntaxCheck { sql, placeholders });
    }
    Ok(ModelFacts {
        name: native_field(&native_field(&model_file, "file_path")?, "stem")?.extract()?,
        directory: native_field(&native_field(&model_file, "relative_path")?, "parent")?
            .str()?
            .to_string(),
        tags: tag_texts(
            &native_field(&native_field(model_input, "config")?, "values")?
                .call_method1("get", ("tags",))?,
        )?,
        references: references(reference_rows),
        syntax_checks,
    })
}

/// Every item of `inputs.<attribute>`.
fn items<'py>(inputs: &Bound<'py, PyAny>, attribute: &str) -> PyResult<Vec<Bound<'py, PyAny>>> {
    native_field(inputs, attribute)?.try_iter()?.collect()
}

/// The native assembly request for compile inputs, and the exceptions opaque vars raise.
pub(crate) fn project_request<'py>(
    inputs: &Bound<'py, PyAny>,
    dialect: Option<String>,
    syntax_checks: &Bound<'py, PyAny>,
) -> PyResult<(ProjectRequest, Vec<Bound<'py, PyAny>>)> {
    let py: Python<'py> = inputs.py();
    let (variables, held) = variables(&native_field(inputs, "effective_vars")?)?;
    let target: Bound<'py, PyAny> = native_field(inputs, "effective_target")?;
    let target: Option<TargetNamespace> = if target.is_none() {
        None
    } else {
        Some(TargetNamespace {
            database: optional_text(&target, "database")?,
            schema: optional_text(&target, "schema")?,
            loader_schema: optional_text(&target, "loader_schema")?,
        })
    };
    let defaults: Bound<'py, PyAny> =
        native_field(&native_field(inputs, "project_config")?, "defaults")?;
    let defaults = SeedDefaults {
        database: optional_text(&defaults, "database")?,
        schema: optional_text(&defaults, "schema")?,
        seed_database: optional_text(&defaults, "seed_database")?,
        seed_schema: optional_text(&defaults, "seed_schema")?,
    };
    let seed_inputs: Vec<Bound<'py, PyAny>> = items(inputs, "seed_inputs")?;
    let mut seeds: Vec<SeedFacts> = Vec::with_capacity(seed_inputs.len());
    for seed_input in &seed_inputs {
        let entry: Bound<'py, PyAny> = native_field(seed_input, "schema_entry")?;
        seeds.push(SeedFacts {
            name: native_field(&entry, "name")?.extract()?,
            tags: native_field(&entry, "tags")?
                .try_iter()?
                .map(|tag| tag?.extract())
                .collect::<PyResult<_>>()?,
            database: optional_text(&entry, "database")?,
            schema: optional_text(&entry, "schema")?,
        });
    }
    let mut template_values: Vec<Option<String>> = Vec::new();
    if let Some(target) = &target {
        template_values.extend([
            target.database.clone(),
            target.schema.clone(),
            target.loader_schema.clone(),
        ]);
    }
    template_values.extend([
        defaults.database.clone(),
        defaults.schema.clone(),
        defaults.seed_database.clone(),
        defaults.seed_schema.clone(),
    ]);
    for seed in &seeds {
        template_values.extend([seed.database.clone(), seed.schema.clone()]);
    }
    let model_inputs: Vec<Bound<'py, PyAny>> = items(inputs, "model_inputs")?;
    let checks: Vec<Bound<'py, PyAny>> = syntax_checks.try_iter()?.collect::<PyResult<_>>()?;
    if checks.len() != model_inputs.len() {
        return Err(PyValueError::new_err(
            "zip() arguments have different lengths: model inputs and syntax checks",
        ));
    }
    let models: Vec<ModelFacts> = model_inputs
        .iter()
        .zip(&checks)
        .map(|(model_input, checks)| model_facts(model_input, checks))
        .collect::<PyResult<_>>()?;
    let rows = |attribute: &str| -> PyResult<Vec<Bound<'py, PyAny>>> {
        items(inputs, attribute)?
            .into_iter()
            .map(|input| native_field(&input, "assembly_row"))
            .collect()
    };
    let sources: Vec<SourceFacts> = rows("source_inputs")?
        .into_iter()
        .map(|row| {
            let SourceRow {
                name,
                managed,
                database,
                schema,
            }: SourceRow = row.extract()?;
            Ok(SourceFacts {
                name,
                managed,
                database,
                schema,
            })
        })
        .collect::<PyResult<_>>()?;
    let functions: Vec<FunctionFacts> = rows("sql_function_inputs")?
        .into_iter()
        .map(|row| {
            let FunctionRow {
                kind,
                name,
                tags,
                references: reference_rows,
            }: FunctionRow = row.extract()?;
            Ok(FunctionFacts {
                kind,
                name,
                tags,
                references: references(reference_rows),
            })
        })
        .collect::<PyResult<_>>()?;
    let audits: Vec<AuditFacts> = rows("audit_inputs")?
        .into_iter()
        .map(|row| {
            let (reference_rows, attached): AuditRow = row.extract()?;
            Ok(AuditFacts {
                references: references(reference_rows),
                attached,
            })
        })
        .collect::<PyResult<_>>()?;
    let request = ProjectRequest {
        dialect: dialect
            .filter(|dialect| !dialect.is_empty())
            .unwrap_or_else(|| GENERIC_DIALECT.to_owned()),
        target,
        defaults,
        variables,
        environment: environment_rows(py, &template_values)?,
        models,
        sources,
        seeds,
        functions,
        audits,
    };
    Ok((request, held))
}

/// The exception Python's assembly raises at the failing resource.
fn failure_error<'py>(
    py: Python<'py>,
    error: AssemblyError,
    held: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    match error {
        AssemblyError::CompileInput { message, help } => error_class(py, ErrorId::CompileInput)?
            .call(
                (message,),
                Some(&keywords(
                    py,
                    &[("help", help.into_pyobject(py)?.into_any())],
                )?),
            ),
        AssemblyError::Value(message) => {
            Ok(PyValueError::new_err(message).value(py).clone().into_any())
        }
        AssemblyError::Raise(index) => match held.get(index) {
            Some(error) => Ok(error.clone()),
            None => Ok(native_compiler_error(format!(
                "{NATIVE_FAILURE_PREFIX}{PROJECT_ASSEMBLY_CONTEXT}: unknown failure kind 'held'"
            ))
            .value(py)
            .clone()
            .into_any()),
        },
        AssemblyError::Internal(reason) => Err(compiler_error(native_failure(
            PROJECT_ASSEMBLY_CONTEXT,
            &reason,
        ))),
    }
}

fn failure_object<'py>(
    py: Python<'py>,
    failure: ResourceFailure,
    held: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    let slot: &str = match failure.slot {
        ResourceSlot::Source => SOURCE_SLOT,
        ResourceSlot::Seed => SEED_SLOT,
        ResourceSlot::Audit => AUDIT_SLOT,
    };
    Ok(Bound::new(
        py,
        NativeResourceFailure {
            slot: PyString::new(py, slot).into_any().unbind(),
            index: failure.index.into_pyobject(py)?.into_any().unbind(),
            error: failure_error(py, failure.error, held)?.unbind(),
        },
    )?
    .into_any())
}

fn namespace_row(namespace: Namespace) -> NamespaceRow {
    NamespaceRow {
        database: namespace.database,
        schema: namespace.schema,
        logical_database: namespace.logical_database,
        logical_schema: namespace.logical_schema,
    }
}

/// `NativeProjectResources` for the native result, replaying its template reads first.
pub(crate) fn resources_object<'py>(
    py: Python<'py>,
    resources: ProjectResources,
    held: &[Bound<'py, PyAny>],
) -> PyResult<Bound<'py, PyAny>> {
    record_reads(
        py,
        resources.reads.iter().map(|read| match read {
            InputRead::Environment(name) => (ENVIRONMENT_READ, name.as_str()),
            InputRead::Context(name) => (CONTEXT_READ, name.as_str()),
        }),
    )?;
    let tuple = |values: Bound<'py, PyAny>| -> PyResult<Bound<'py, PyAny>> {
        Ok(PyTuple::new(py, values.try_iter()?.collect::<PyResult<Vec<_>>>()?)?.into_any())
    };
    let failure: Bound<'py, PyAny> = match resources.failure {
        Some(failure) => failure_object(py, failure, held)?,
        None => py.None().into_bound(py),
    };
    let seeds: Vec<_> = resources.seeds.into_iter().map(namespace_row).collect();
    Ok(Bound::new(
        py,
        NativeProjectResources {
            model_deps: tuple(resources.model_deps.into_pyobject(py)?.into_any())?.unbind(),
            model_syntax_valid: tuple(resources.model_syntax_valid.into_pyobject(py)?.into_any())?
                .unbind(),
            source_namespaces: tuple(resources.sources.into_pyobject(py)?.into_any())?.unbind(),
            seed_namespaces: tuple(seeds.into_pyobject(py)?.into_any())?.unbind(),
            function_deps: tuple(resources.function_deps.into_pyobject(py)?.into_any())?.unbind(),
            audit_scope_deps: tuple(resources.audit_deps.into_pyobject(py)?.into_any())?.unbind(),
            failure: failure.unbind(),
        },
    )?
    .into_any())
}
