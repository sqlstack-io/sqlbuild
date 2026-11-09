//! Rejected `sql_analysis false` opt-outs, and native-route analysis skipped on a parser gap.

use std::path::Path;

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyErr, PyResult, Python};
use pyo3::types::{PyBool, PyBoolMethods, PyDict, PyDictMethods, PyList, PyString, PyTuple};
use sqlbuild_analysis::analysis_route::main::aligned_offset::aligned_offset;
use sqlbuild_analysis::analysis_route::main::native_gap::native_gap;
use sqlbuild_analysis::analysis_route::main::skipped_analysis::skipped_analysis;
use sqlbuild_analysis::analysis_route::main::unavailable_analysis::unavailable_analysis;
use sqlbuild_analysis::analysis_route::models::{AnalysisRoute, GapPosition, GapSite, LocatedGap};
use sqlbuild_analysis::analysis_route::types::NativeDialect;
use sqlbuild_analysis::assembly::project::models::{SyntaxCheck, SyntaxMode};
use sqlbuild_analysis::compiled_project::main::authored_model_offset::authored_model_offset;
use sqlbuild_analysis::compiled_project::main::opt_out_position::opt_out_position;
use sqlbuild_analysis::compiled_project::main::text_position::text_position;
use sqlbuild_analysis::compiled_project::models::ModelLoopSettings;
use sqlbuild_analysis::compiled_project::models::{SplicedSpan, TextPosition};
use sqlbuild_attachments::cursor_intrinsics::main::replace_intrinsics::replace_intrinsics;

use crate::bindings::_helpers::input_loops::model_loop::{
    LoopContext, NativeModelLoop, compile_error,
};
use crate::bindings::_helpers::input_loops::model_loop_inputs::{HOOK_KEYS, present};
use crate::bindings::_helpers::project_assembly::resources::{syntax_error_message, syntax_valid};
use crate::bindings::_helpers::project_assembly::syntax_validation::sql_entry_label;
use crate::bindings::_helpers::records::construct::{new_helped_error, new_record_with};
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::ids::{ErrorId, RecordId};

const OPT_OUT_KEYS: [&str; 2] = ["sql_analysis", "sql_validation"];
const CURSOR_TYPE_KEY: &str = "cursor_type";
const PLACEHOLDERS_KEY: &str = "placeholders";
const INTEGER_CURSOR: &str = "integer";
const INTEGER_LITERAL: &str = "CAST(0 AS BIGINT)";
const TIMESTAMP_LITERAL: &str = "CAST('2000-01-01 00:00:00' AS TIMESTAMP)";
const INTRINSIC_CONTEXT: &str = "Model SQL";
const GENERIC_DIALECT: &str = "generic";
const STATEMENT_ATTRIBUTE: &str = "statement";
const HEADER_KEYS_ATTRIBUTE: &str = "model_header_keys";
const PATH_DEFAULT_ATTRIBUTE: &str = "matched_path_default";
const PATHLIB_MODULE: &str = "pathlib";
const PATH_CLASS: &str = "Path";
const PATH_FIELD: &str = "path";
const LINE_FIELD: &str = "line";
const COLUMN_FIELD: &str = "column";
const HELP_ARGUMENT: &str = "help";
const MODEL_RESOURCE: &str = "model";
const RELATIVE_PATH_ATTRIBUTE: &str = "relative_path";

/// Where the opt-out was authored, or a check the attachment still answers.
pub(crate) enum OptOut<'py> {
    Accepted,
    Rejected(Bound<'py, PyAny>),
}

/// The model the opt-out is checked for.
pub(crate) struct OptOutModel<'a, 'py> {
    pub(crate) name: &'a str,
    pub(crate) file_path: &'a str,
    pub(crate) config: &'a Bound<'py, PyAny>,
    pub(crate) values: &'a Bound<'py, PyDict>,
    pub(crate) query_sql: &'a str,
    pub(crate) relative_path: &'a str,
    pub(crate) contents: &'a str,
    /// How `query_sql` maps back to the model file; none when interpolation rewrote the query.
    pub(crate) authored: Option<AuthoredQuery<'a>>,
}

/// The query as the model file holds it, as macros expanded it, and the passes between them.
#[derive(Clone, Copy)]
pub(crate) struct AuthoredQuery<'a> {
    pub(crate) query: &'a str,
    pub(crate) macro_sql: &'a str,
    /// The declaration and macro expansion passes, in that order.
    pub(crate) passes: &'a [Vec<SplicedSpan>],
}

/// One statement a native parser must read, with the site P026 names it by.
struct SiteCheck {
    check: SyntaxCheck,
    /// The hook, such as `post_hooks[0] sql("record_refresh")`; none for the model's query.
    label: Option<String>,
    path: String,
}

/// A hook value being walked: its key, its file and the label its statements are named by.
struct HookSite<'a> {
    hook_name: &'a str,
    path: String,
    label: Option<String>,
}

/// Whether the opt-out is rejected, and where it was authored.
pub(crate) fn rejected_opt_out<'py>(
    model_loop: &NativeModelLoop,
    context: &LoopContext<'_, 'py>,
    model: &OptOutModel<'_, 'py>,
) -> PyResult<OptOut<'py>> {
    let settings = model_loop.settings;
    if !settings.require_sql_analysis
        || !settings.sql_analysis
        || settings.no_sql_validation
        || !opted_out(model.values)?
    {
        return Ok(OptOut::Accepted);
    }
    let checks: Vec<SiteCheck> = model_checks(context, model)?;
    let readable: bool = match model_loop.route {
        AnalysisRoute::Legacy => {
            let statements: Vec<SyntaxCheck> =
                checks.iter().map(|site| site.check.clone()).collect();
            syntax_valid(model.config.py(), GENERIC_DIALECT, &statements)?
        }
        AnalysisRoute::Native(dialect) => first_gap(dialect, context, &checks)?.is_none(),
        AnalysisRoute::Unavailable => false,
    };
    if readable {
        Ok(OptOut::Rejected(location(model_loop, context, model)?))
    } else {
        Ok(OptOut::Accepted)
    }
}

/// Whether a native route analyses the model: P026 warns or raises when its parser cannot read it, and P001 raises when the parser reports invalid syntax.
pub(crate) fn skip_unreadable(
    settings: ModelLoopSettings,
    dialect: &dyn NativeDialect,
    context: &LoopContext<'_, '_>,
    model: &OptOutModel<'_, '_>,
) -> PyResult<bool> {
    let checks: Vec<SiteCheck> = model_checks(context, model)?;
    let Some((check, gap)) = first_gap(dialect, context, &checks)? else {
        return Ok(true);
    };
    let site: GapSite<'_> = GapSite {
        label: check.label.as_deref(),
        path: &check.path,
        position: gap_position(model, check, &gap),
    };
    let (code, message) = skipped_analysis(model.name, dialect.name(), &gap, &site);
    if gap.syntax {
        return Err(new_helped_error(
            model.config.py(),
            ErrorId::CompileInput,
            (message, code),
            Some(&context.classes.syntax_help),
        ));
    }
    report_skip(settings, context, model, (code, message))?;
    Ok(false)
}

/// A hook's gap in its statement; the query's in the model file, when its text maps back there.
fn gap_position(model: &OptOutModel<'_, '_>, check: &SiteCheck, gap: &LocatedGap) -> GapPosition {
    let Some(offset) = gap.source_offset else {
        return GapPosition::Analysed;
    };
    if check.label.is_some() {
        return GapPosition::Statement(text_position(&check.check.sql, offset));
    }
    model
        .authored
        .and_then(|authored| {
            let query: usize = aligned_offset(&check.check.sql, offset, model.query_sql)?;
            let expanded: usize = aligned_offset(model.query_sql, query, authored.macro_sql)?;
            authored_model_offset(
                model.contents,
                authored.query,
                (authored.macro_sql, expanded),
                authored.passes,
            )
        })
        .map_or(GapPosition::Analysed, |file| {
            GapPosition::File(text_position(model.contents, file))
        })
}

/// P026 for a model whose dialect no native parser reads; it warns, or raises, and is not analysed.
pub(crate) fn skip_unavailable(
    settings: ModelLoopSettings,
    dialect: Option<&str>,
    context: &LoopContext<'_, '_>,
    model: &OptOutModel<'_, '_>,
) -> PyResult<()> {
    report_skip(
        settings,
        context,
        model,
        unavailable_analysis(model.name, dialect),
    )
}

/// Reports P026 as a warning, or raises it under `require_sql_analysis`.
fn report_skip(
    settings: ModelLoopSettings,
    context: &LoopContext<'_, '_>,
    model: &OptOutModel<'_, '_>,
    (code, message): (&str, String),
) -> PyResult<()> {
    let py: Python<'_> = model.config.py();
    let help: &str = &context.classes.syntax_help;
    if settings.require_sql_analysis {
        return Err(new_helped_error(
            py,
            ErrorId::CompileInput,
            (message, code),
            Some(help),
        ));
    }
    let diagnostic: Bound<'_, PyAny> = new_record_with(
        py,
        RecordId::CompilerDiagnostic,
        &[
            ("phase", PyString::new(py, "compile").into_any()),
            ("severity", PyString::new(py, "warning").into_any()),
            ("code", PyString::new(py, code).into_any()),
            ("message", PyString::new(py, &message).into_any()),
            (
                "resource_type",
                PyString::new(py, MODEL_RESOURCE).into_any(),
            ),
            ("resource_name", PyString::new(py, model.name).into_any()),
            (
                "path",
                context.classes.path_class.call1((model.relative_path,))?,
            ),
            ("help", PyString::new(py, help).into_any()),
        ],
    )?;
    let arguments: Bound<'_, PyDict> = PyDict::new(py);
    arguments.set_item("key", PyTuple::new(py, [code, model.name])?)?;
    arguments.set_item("diagnostic", diagnostic)?;
    context
        .classes
        .report_diagnostic
        .call((), Some(&arguments))?;
    Ok(())
}

/// The model's analysis SQL and each hook statement, as filled to run, with its placeholders.
fn model_checks(
    context: &LoopContext<'_, '_>,
    model: &OptOutModel<'_, '_>,
) -> PyResult<Vec<SiteCheck>> {
    let mut statements: Vec<(Option<String>, String, String)> = vec![(
        None,
        model.file_path.to_owned(),
        analysis_sql(context, model)?,
    )];
    for key in HOOK_KEYS {
        if let Some(hooks) = model.values.get_item(key)? {
            let site: HookSite<'_> = HookSite {
                hook_name: key,
                path: model.file_path.to_owned(),
                label: None,
            };
            statements.extend(hook_statements(context, &hooks, site)?);
        }
    }
    let placeholders: Vec<(String, String)> = placeholders(model.values)?;
    Ok(statements
        .into_iter()
        .map(|(label, path, sql)| SiteCheck {
            check: SyntaxCheck {
                sql,
                placeholders: placeholders.clone(),
            },
            label,
            path,
        })
        .collect())
}

/// The native parser's first syntax error across `checks`, else its first gap, with its check.
fn first_gap<'a>(
    dialect: &dyn NativeDialect,
    context: &LoopContext<'_, '_>,
    checks: &'a [SiteCheck],
) -> PyResult<Option<(&'a SiteCheck, LocatedGap)>> {
    let mut first: Option<(&'a SiteCheck, LocatedGap)> = None;
    for site in checks {
        if let Some(gap) = native_gap(dialect, &site.check)
            .map_err(|message| compile_error(context.classes, &message, None))?
        {
            if gap.syntax {
                return Ok(Some((site, gap)));
            }
            first = first.or(Some((site, gap)));
        }
    }
    Ok(first)
}

/// `validate_sql_syntax`: raise the model's SQL syntax error before its references are scanned.
pub(crate) fn reject_unparseable_query(
    context: &LoopContext<'_, '_>,
    model: &OptOutModel<'_, '_>,
) -> PyResult<()> {
    let check: SyntaxCheck = SyntaxCheck {
        sql: analysis_sql(context, model)?,
        placeholders: placeholders(model.values)?,
    };
    let Some(error) = syntax_error_message(
        model.config.py(),
        &check,
        GENERIC_DIALECT,
        SyntaxMode::Validate,
    )?
    else {
        return Ok(());
    };
    let message: String = format!(
        "SQL syntax error in model '{}' ({}): {error}",
        model.name, model.file_path
    );
    let keywords: Bound<'_, PyDict> = PyDict::new(model.config.py());
    keywords.set_item(HELP_ARGUMENT, &context.classes.syntax_help)?;
    Err(PyErr::from_value(
        context
            .classes
            .compile_error
            .call((message,), Some(&keywords))?,
    ))
}

/// `cursor_intrinsics_analysis_sql`: the query with each cursor intrinsic as a typed literal.
fn analysis_sql(context: &LoopContext<'_, '_>, model: &OptOutModel<'_, '_>) -> PyResult<String> {
    let literal: &str = match model.values.get_item(CURSOR_TYPE_KEY)? {
        Some(value) if value.eq(INTEGER_CURSOR)? => INTEGER_LITERAL,
        _ => TIMESTAMP_LITERAL,
    };
    let (analysis_sql, _) =
        replace_intrinsics(model.query_sql, INTRINSIC_CONTEXT, |_| literal.to_owned())
            .map_err(|message| compile_error(context.classes, &message, None))?;
    Ok(analysis_sql)
}

/// Whether either opt-out key is exactly `False`.
fn opted_out(values: &Bound<'_, PyDict>) -> PyResult<bool> {
    for key in OPT_OUT_KEYS {
        if let Some(value) = values.get_item(key)?
            && value.downcast::<PyBool>().is_ok_and(|flag| !flag.is_true())
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// `hook_sql_statements` as `(label, file, SQL)`, labelled as `validate_hook_sql_syntax` names them.
fn hook_statements(
    context: &LoopContext<'_, '_>,
    value: &Bound<'_, PyAny>,
    site: HookSite<'_>,
) -> PyResult<Vec<(Option<String>, String, String)>> {
    let sql_hook_entry: &Bound<'_, PyAny> = &context.classes.sql_hook_entry;
    if let Ok(text) = value.downcast::<PyString>() {
        let label: String = site.label.unwrap_or_else(|| site.hook_name.to_owned());
        return Ok(vec![(Some(label), site.path, text.extract()?)]);
    }
    if value.is_instance(sql_hook_entry)? {
        let label: String = match site.label {
            Some(label) => label,
            None => sql_entry_label(site.hook_name, value)?,
        };
        let relative_path: Bound<'_, PyAny> = native_field(value, RELATIVE_PATH_ATTRIBUTE)?;
        let path: String = if relative_path.is_truthy()? {
            relative_path.str()?.to_string()
        } else {
            site.path
        };
        let inner: HookSite<'_> = HookSite {
            hook_name: site.hook_name,
            path,
            label: Some(label),
        };
        return hook_statements(context, &native_field(value, STATEMENT_ATTRIBUTE)?, inner);
    }
    let mut statements: Vec<(Option<String>, String, String)> = Vec::new();
    if value.is_instance_of::<PyList>() || value.is_instance_of::<PyTuple>() {
        for (index, item) in value.try_iter()?.enumerate() {
            let item: Bound<'_, PyAny> = item?;
            let prefix: String = format!("{}[{index}]", site.hook_name);
            let label: Option<String> = if item.is_instance(sql_hook_entry)? {
                Some(sql_entry_label(&prefix, &item)?)
            } else if item.is_instance_of::<PyString>() {
                Some(format!("{prefix} inline_sql(\"...\")"))
            } else {
                None
            };
            let inner: HookSite<'_> = HookSite {
                hook_name: site.hook_name,
                path: site.path.clone(),
                label,
            };
            statements.extend(hook_statements(context, &item, inner)?);
        }
    }
    Ok(statements)
}

/// The model's `placeholders` mapping as `str` pairs, when it is a dictionary.
fn placeholders(values: &Bound<'_, PyDict>) -> PyResult<Vec<(String, String)>> {
    let Some(raw) = present(values, PLACEHOLDERS_KEY)? else {
        return Ok(Vec::new());
    };
    let Ok(raw) = raw.downcast::<PyDict>() else {
        return Ok(Vec::new());
    };
    raw.iter()
        .map(|(key, value)| Ok((key.str()?.to_string(), value.str()?.to_string())))
        .collect()
}

/// `_opt_out_location`: the model header when it sets the key, else its path default entry.
fn location<'py>(
    model_loop: &NativeModelLoop,
    context: &LoopContext<'_, 'py>,
    model: &OptOutModel<'_, 'py>,
) -> PyResult<Bound<'py, PyAny>> {
    let py: Python<'py> = model.config.py();
    let header_keys: Vec<String> = native_field(model.config, HEADER_KEYS_ATTRIBUTE)?.extract()?;
    let (path, position): (&str, TextPosition) = if header_keys
        .iter()
        .any(|key| OPT_OUT_KEYS.contains(&key.as_str()))
    {
        (model.relative_path, opt_out_position(model.contents, 0))
    } else {
        let path_key: Option<String> =
            native_field(model.config, PATH_DEFAULT_ATTRIBUTE)?.extract()?;
        let config_path: &Path = Path::new(&model_loop.project_config_path);
        let config_text: String = if config_path.is_file() {
            std::fs::read_to_string(config_path)?
        } else {
            String::new()
        };
        let start: usize = path_key
            .filter(|key| !key.is_empty())
            .and_then(|key| config_text.find(&key))
            .map_or(0, |byte| config_text[..byte].chars().count());
        let file_name: &str = model_loop
            .project_config_path
            .rsplit('/')
            .next()
            .unwrap_or_default();
        (file_name, opt_out_position(&config_text, start))
    };
    let path_class: Bound<'py, PyAny> = py.import(PATHLIB_MODULE)?.getattr(PATH_CLASS)?;
    let fields: Bound<'py, PyDict> = PyDict::new(py);
    fields.set_item(PATH_FIELD, path_class.call1((path,))?)?;
    fields.set_item(LINE_FIELD, position.line)?;
    fields.set_item(COLUMN_FIELD, position.column)?;
    context.contracts.source_location.call((), Some(&fields))
}
