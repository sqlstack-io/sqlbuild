//! A location load is a full compile: the configured rules run after it, as `sqb compile` runs them.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult, Python};
use sqlbuild_orchestration::platform::constants::{
    MAX_REPORTED_RULE_FINDINGS, MAX_RULE_FINDING_CHARS,
};
use sqlbuild_orchestration::platform::models::{LocationRuleFailure, LocationRuleFinding};

use crate::bindings::_helpers::compile_driver::plan_pipeline::configured_rule_failures;
use crate::bindings::_helpers::query_command::project::ProjectAdapter;
use sqlbuild_python_base::boundary::models::KeywordArguments;
use sqlbuild_python_base::records::main::construct::new_record_with::new_record_with;
use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::types::RecordId;
use sqlbuild_python_compile::compile_driver::main::setup::project_graph::project_graph;
use sqlbuild_python_compile::rules::models::EarlySqlLint;
use sqlbuild_python_output::recall::main::write_stderr::write_stderr;

/// The configured rules' findings when they fail the compiled project, printed as `sqb compile` would.
pub(super) fn load_rules<'py>(
    py: Python<'py>,
    arguments: &KeywordArguments<'py>,
    (project, compiled): (&ProjectAdapter<'py>, &Bound<'py, PyAny>),
) -> PyResult<Option<LocationRuleFailure>> {
    let options = new_record_with(
        py,
        RecordId::CompilePipelineOptions,
        &[("selected_target", arguments.value("selected_target")?)],
    )?;
    let expansion = Bound::new(py, EarlySqlLint::new(false, false))?;
    let compiled_project = field(compiled, "project")?;
    let graph = project_graph(&compiled_project)?;
    write_stderr(py, "Evaluating configured rules...\n")?;
    let Some((summary, rendered, notice)) = configured_rule_failures(
        (&project.discovered, &project.adapter, expansion.as_any()),
        (&options, &py.None().into_bound(py)),
        (&compiled_project, &graph),
    )?
    else {
        write_stderr(py, "Configured rules passed.\n")?;
        return Ok(None);
    };
    let mut findings: Vec<LocationRuleFinding> = Vec::new();
    let mut finding_count: u64 = 0;
    for finding in field(&summary, "findings")?.try_iter()? {
        let finding = finding?;
        finding_count += 1;
        if findings.len() < MAX_REPORTED_RULE_FINDINGS {
            let path = field(&finding, "path")?;
            findings.push(LocationRuleFinding {
                code: bounded(field(&finding, "code")?.extract()?),
                message: bounded(field(&finding, "message")?.extract()?),
                path: (!path.is_none())
                    .then(|| path.str().map(|path| bounded(path.to_string())))
                    .transpose()?,
                line: field(&finding, "line")?.extract()?,
            });
        }
    }
    let noun = if finding_count == 1 {
        "rule finding"
    } else {
        "rule findings"
    };
    let notice = notice
        .map(|notice| format!("\n{notice}"))
        .unwrap_or_default();
    write_stderr(
        py,
        &format!("Compile failed: {finding_count} {noun}\n{rendered}{notice}\n"),
    )?;
    Ok(Some(LocationRuleFailure {
        finding_count,
        findings,
    }))
}

/// `text` cut to the reported bound.
fn bounded(text: String) -> String {
    text.chars().take(MAX_RULE_FINDING_CHARS).collect()
}
