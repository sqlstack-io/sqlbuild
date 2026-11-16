//! A format run's Python violations and Rule fix reports as plain report data.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::format_command::models::{LintFinding, LintReport, RuleFixLine};
use sqlbuild_core::text::main::python_splitlines::python_splitlines;

use crate::bindings::_helpers::format_command::run::FormatOutcome;
use sqlbuild_python_base::records::main::fields::field::field as native_field;

const FAULT_SEVERITY: &str = "fault";
const APPLIED_STATUS: &str = "applied";
const REFUSED_STATUS: &str = "refused";
const DECLINED_REASON: &str = "Final layout formatting failed; original file retained";

fn text(value: &Bound<'_, PyAny>) -> PyResult<String> {
    value.str()?.extract()
}

fn display_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

fn source_line(
    contents: &HashMap<PathBuf, String>,
    path: &Bound<'_, PyAny>,
    line: i64,
) -> PyResult<Option<String>> {
    let Some(source) = contents.get(&path.extract::<PathBuf>()?) else {
        return Ok(None);
    };
    if line < 1 {
        return Ok(None);
    }
    let lines: Vec<&str> = python_splitlines(source);
    let index: usize = usize::try_from(line - 1).unwrap_or(usize::MAX);
    Ok(lines.get(index).map(|line| (*line).to_owned()))
}

fn finding(
    violation: &Bound<'_, PyAny>,
    contents: &HashMap<PathBuf, String>,
    root: &Path,
) -> PyResult<LintFinding> {
    let path: Bound<'_, PyAny> = native_field(violation, "file_path")?;
    let line: i64 = native_field(violation, "line")?.extract()?;
    let remediation: Bound<'_, PyAny> = native_field(violation, "remediation")?;
    Ok(LintFinding {
        file: text(&path)?,
        display_path: display_path(&path.extract::<PathBuf>()?, root),
        line,
        column: native_field(violation, "column")?.extract()?,
        end_line: native_field(violation, "end_line")?.extract()?,
        end_column: native_field(violation, "end_column")?.extract()?,
        code: native_field(violation, "code")?.extract()?,
        message: native_field(violation, "message")?.extract()?,
        fault: text(&native_field(violation, "severity")?)? == FAULT_SEVERITY,
        engine: native_field(violation, "engine")?.extract()?,
        remediation: if remediation.is_none() {
            None
        } else {
            Some(remediation.extract()?)
        },
        source_line: source_line(contents, &path, line)?,
    })
}

fn rule_fix(report: &Bound<'_, PyAny>, declined: &BTreeSet<PathBuf>) -> PyResult<RuleFixLine> {
    let path: Bound<'_, PyAny> = native_field(report, "file_path")?;
    let status: String = native_field(report, "status")?.extract()?;
    let reason: String = native_field(report, "reason")?.extract()?;
    let refused: bool = status == APPLIED_STATUS && declined.contains(&path.extract::<PathBuf>()?);
    Ok(RuleFixLine {
        file: text(&path)?,
        code: native_field(report, "code")?.extract()?,
        line: native_field(report, "line")?.extract()?,
        status: if refused {
            REFUSED_STATUS.to_owned()
        } else {
            status
        },
        reason: if refused {
            DECLINED_REASON.to_owned()
        } else {
            reason
        },
    })
}

/// The report of `outcome`, violations sorted by file then position, paths shown from `root`.
pub(crate) fn lint_report(outcome: &FormatOutcome<'_>, root: &Path) -> PyResult<LintReport> {
    let mut findings: Vec<LintFinding> = outcome
        .violations
        .iter()
        .map(|violation| finding(violation, &outcome.final_contents, root))
        .collect::<PyResult<_>>()?;
    findings.sort_by(|left, right| {
        (&left.file, left.line, left.column).cmp(&(&right.file, right.line, right.column))
    });
    Ok(LintReport {
        files_checked: outcome.files_checked,
        formatted_files: outcome
            .changes
            .iter()
            .map(|change| text(&change.path))
            .collect::<PyResult<_>>()?,
        rule_fixes: outcome
            .rule_fixes
            .iter()
            .map(|report| rule_fix(report, &outcome.declined))
            .collect::<PyResult<_>>()?,
        findings,
    })
}
