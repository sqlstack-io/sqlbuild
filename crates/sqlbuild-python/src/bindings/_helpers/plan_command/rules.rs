//! `sqb rules`: list, show, install skills for, or run a selection of compiler rules.

use pyo3::prelude::{
    Bound, PyAny, PyAnyMethods, PyModule, PyModuleMethods, PyResult, Python, pyfunction,
};
use pyo3::types::{PyDict, PyDictMethods, PyTuple};
use pyo3::wrap_pyfunction;
use sqlbuild_rules::lint_text::main::counts_toward_exit::counts_toward_exit;
use sqlbuild_rules::rule_catalogue::main::render_rule_list::render_rule_list;
use sqlbuild_rules::rule_catalogue::models::RuleListing;

use crate::bindings::_helpers::boundary::python_calls::imported;
use crate::bindings::_helpers::boundary::python_calls::keywords;
use crate::bindings::_helpers::command_runtime::adapters::resolve_cli_adapter;
use crate::bindings::_helpers::compile_driver::discovery::discover_project_inputs;
use crate::bindings::_helpers::compile_driver::project::build_project_graph;
use crate::bindings::_helpers::contracts::spec_resolution::resolve_effective_adapter_name;
use crate::bindings::_helpers::planner_selection::selectors::select_project_graph;
use crate::bindings::_helpers::planner_selection::unit_tests::reject_unit_test_selectors;
use crate::bindings::_helpers::records::construct::new_record;
use crate::bindings::_helpers::records::fields::field as native_field;
use crate::bindings::_helpers::records::fields::replaced_keywords;
use crate::bindings::_helpers::records::ids::RecordId;
use crate::bindings::_helpers::rules::catalogue::build_rule_catalogue;
use crate::bindings::_helpers::rules::guidance::{format_rule, install_rules_skills};
use crate::bindings::_helpers::rules::results::{
    format_rules_result, format_skipped_type_proof_rules,
};
use crate::bindings::_helpers::rules::rule_run::run_rules;
use crate::bindings::_helpers::rules::rules_config::load_rules_config;

const LIST_ACTION: &str = "list";
const SHOW_ACTION: &str = "show";
const SKILLS_ACTION: &str = "skills";
const GENERIC_DIALECT: &str = "generic";
const USAGE_EXIT_CODE: i32 = 2;
const RESULT_FIELDS: [&str; 5] = [
    "findings",
    "evaluated_models",
    "unevaluated_resources",
    "cache_hits",
    "cache_misses",
];

/// `print(text)`, or `print(text, file=sys.stderr)`.
fn print_line(py: Python<'_>, text: &str, stderr: bool) -> PyResult<()> {
    let options: Bound<'_, PyDict> = PyDict::new(py);
    if stderr {
        options.set_item("file", py.import("sys")?.getattr("stderr")?)?;
    }
    imported(py, "builtins", "print")?.call((text,), Some(&options))?;
    Ok(())
}

fn rule_codes(catalogue: &Bound<'_, PyAny>) -> PyResult<Vec<String>> {
    let mut codes: Vec<String> = Vec::new();
    for rule in catalogue.try_iter()? {
        codes.push(native_field(&rule?, "code")?.extract()?);
    }
    Ok(codes)
}

fn run_skills(
    request: &Bound<'_, PyAny>,
    config: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    let check: bool = native_field(request, "skills_check")?.is_truthy()?;
    let fresh: bool = install_rules_skills(config, project_dir, check)?;
    if !check {
        print_line(py, "Installed Rules skills", false)?;
        return Ok(0);
    }
    print_line(
        py,
        if fresh {
            "Rules skills are fresh"
        } else {
            "Rules skills are stale"
        },
        false,
    )?;
    Ok(if fresh { 0 } else { 1 })
}

fn rule_listing(rule: &Bound<'_, PyAny>) -> PyResult<RuleListing> {
    let subject: Bound<'_, PyAny> = native_field(rule, "subject")?;
    Ok(RuleListing {
        code: native_field(rule, "code")?.str()?.extract()?,
        family: native_field(rule, "family")?.extract()?,
        message: native_field(rule, "message")?.str()?.extract()?,
        custom: native_field(rule, "custom")?.is_truthy()?,
        subject: if subject.is_none() {
            None
        } else {
            Some(native_field(&subject, "value")?.extract()?)
        },
    })
}

fn list_rules(catalogue: &Bound<'_, PyAny>, json_output: bool) -> PyResult<i32> {
    let mut rules: Vec<RuleListing> = Vec::new();
    for rule in catalogue.try_iter()? {
        rules.push(rule_listing(&rule?)?);
    }
    print_line(
        catalogue.py(),
        &render_rule_list(&rules, json_output),
        false,
    )?;
    Ok(0)
}

fn show_rule(
    selector: &str,
    catalogue: &Bound<'_, PyAny>,
    config: &Bound<'_, PyAny>,
) -> PyResult<i32> {
    let py: Python<'_> = catalogue.py();
    for rule in catalogue.try_iter()? {
        let rule: Bound<'_, PyAny> = rule?;
        if native_field(&rule, "code")?.extract::<String>()? == selector {
            let text: String = format_rule(&rule, Some(config.clone()))?;
            print_line(py, &text, false)?;
            return Ok(0);
        }
    }
    print_line(py, &format!("Unknown rule: {selector}"), false)?;
    Ok(USAGE_EXIT_CODE)
}

/// The discovered project, its adapter, graph and selected keys a rule selection runs over.
struct RuleInputs<'py> {
    discovered: Bound<'py, PyAny>,
    adapter: Bound<'py, PyAny>,
    graph: Bound<'py, PyAny>,
    selected: Bound<'py, PyAny>,
}

fn rule_inputs<'py>(
    request: &Bound<'py, PyAny>,
    project_dir: &Bound<'py, PyAny>,
) -> PyResult<RuleInputs<'py>> {
    let py: Python<'py> = request.py();
    let discovered: Bound<'py, PyAny> = discover_project_inputs(
        py,
        Some(keywords(py, &[("project_dir", project_dir.clone())])?),
    )?;
    let select: Bound<'py, PyAny> = native_field(request, "select")?;
    let exclude: Bound<'py, PyAny> = native_field(request, "exclude")?;
    reject_unit_test_selectors(&select, &exclude, &discovered)?;
    let adapter_name: Bound<'py, PyAny> = resolve_effective_adapter_name(
        &native_field(&discovered, "project_config")?,
        &native_field(&discovered, "local_config")?,
    )?;
    let adapter: Bound<'py, PyAny> = resolve_cli_adapter(&adapter_name, project_dir)?;
    let graph: Bound<'py, PyAny> = build_project_graph(
        py,
        Some(keywords(
            py,
            &[
                ("discovered_inputs", discovered.clone()),
                ("adapter", adapter.clone()),
            ],
        )?),
    )?;
    let selected: Bound<'py, PyAny> = select_project_graph(&graph, &select, &exclude)?.into_any();
    Ok(RuleInputs {
        discovered,
        adapter,
        graph,
        selected,
    })
}

fn run_rule_selection(
    request: &Bound<'_, PyAny>,
    (selector, catalogue): (&str, &Bound<'_, PyAny>),
    config: &Bound<'_, PyAny>,
    project_dir: &Bound<'_, PyAny>,
) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    if !rule_codes(catalogue)?
        .iter()
        .any(|code| code.starts_with(selector))
    {
        print_line(py, &format!("Unknown rule or family: {selector}"), false)?;
        return Ok(USAGE_EXIT_CODE);
    }
    print_line(
        py,
        &format!("Evaluating rule selection {selector}..."),
        true,
    )?;
    let RuleInputs {
        discovered,
        adapter,
        graph,
        selected,
    } = rule_inputs(request, project_dir)?;
    let narrowed: bool = native_field(request, "select")?.is_truthy()?
        || native_field(request, "exclude")?.is_truthy()?;
    let replaced: Bound<'_, PyDict> = PyDict::new(py);
    replaced.set_item("select", PyTuple::new(py, [selector])?)?;
    replaced.set_item("ignore", PyTuple::empty(py))?;
    let dialect: Bound<'_, PyAny> = adapter.call_method0("sql_analysis_dialect")?;
    let result: Bound<'_, PyAny> = run_rules(
        py,
        Some(keywords(
            py,
            &[
                ("graph", graph),
                ("discovered_inputs", discovered),
                ("config", replaced_keywords(config, &replaced)?),
                ("project_dir", project_dir.clone()),
                (
                    "dialect",
                    if dialect.is_truthy()? {
                        dialect
                    } else {
                        pyo3::types::PyString::new(py, GENERIC_DIALECT).into_any()
                    },
                ),
                (
                    "selected_keys",
                    if narrowed {
                        selected
                    } else {
                        py.None().into_bound(py)
                    },
                ),
            ],
        )?),
    )?;
    if let Some(note) = format_skipped_type_proof_rules(
        native_field(&result, "skipped_type_proof_rules")?.extract()?,
    ) {
        print_line(py, &format!("note: {note}"), true)?;
    }
    let fields: Bound<'_, PyDict> = PyDict::new(py);
    for field in RESULT_FIELDS {
        fields.set_item(field, native_field(&result, field)?)?;
    }
    let text: String = format_rules_result(
        &new_record(py, RecordId::RulesResult, (), Some(&fields))?,
        native_field(request, "json_output")?.is_truthy()?,
    )?;
    print_line(py, &text, false)?;
    let mut findings: usize = 0;
    for finding in native_field(&result, "findings")?.try_iter()? {
        if counts_toward_exit(&native_field(&finding?, "code")?.extract::<String>()?) {
            findings += 1;
        }
    }
    if findings > 0 {
        print_line(
            py,
            &format!("Rule selection {selector} failed with {findings} finding(s)."),
            true,
        )?;
        return Ok(1);
    }
    print_line(py, &format!("Rule selection {selector} passed."), true)?;
    Ok(0)
}

/// List, inspect, execute, or generate guidance for compiler rules.
#[pyfunction]
#[pyo3(signature = (*, request))]
pub(crate) fn run_rules_command(request: &Bound<'_, PyAny>) -> PyResult<i32> {
    let py: Python<'_> = request.py();
    let requested_dir: Bound<'_, PyAny> = native_field(request, "project_dir")?;
    let project_dir: Bound<'_, PyAny> = if requested_dir.is_none() {
        imported(py, "pathlib", "Path")?.call_method0("cwd")?
    } else {
        requested_dir
    }
    .call_method0("resolve")?;
    let config: Bound<'_, PyAny> = load_rules_config(py, project_dir.extract()?)?.into_bound(py);
    let action: String = native_field(request, "action")?.extract()?;
    if action == SKILLS_ACTION {
        return run_skills(request, &config, &project_dir);
    }
    let catalogue: Bound<'_, PyAny> = build_rule_catalogue(&config, &project_dir, true)?.into_any();
    if action == LIST_ACTION {
        return list_rules(
            &catalogue,
            native_field(request, "json_output")?.is_truthy()?,
        );
    }
    let selector: Option<String> = native_field(request, "rule_selector")?.extract()?;
    let Some(selector) = selector else {
        print_line(py, "A rule code or family prefix is required", false)?;
        return Ok(USAGE_EXIT_CODE);
    };
    if action == SHOW_ACTION {
        return show_rule(&selector, &catalogue, &config);
    }
    run_rule_selection(request, (&selector, &catalogue), &config, &project_dir)
}

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(run_rules_command, module)?)
}
