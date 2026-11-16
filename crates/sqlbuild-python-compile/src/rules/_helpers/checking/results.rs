//! Rules results rendered for the CLI, and SQLCLEAN044 help naming files that repeat a literal.

use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};

use sqlbuild_rules::rule_results::models::RuleFinding;

use sqlbuild_python_base::records::main::fields::field::field;
use sqlbuild_python_base::records::main::fields::path_posix::path_posix;

pub(crate) fn rule_finding(finding: &Bound<'_, PyAny>) -> PyResult<RuleFinding> {
    Ok(RuleFinding {
        code: field(finding, "code")?.extract()?,
        path: path_posix(&field(finding, "path")?)?,
        line: field(finding, "line")?.extract()?,
        column: field(finding, "column")?.extract()?,
        message: field(finding, "message")?.extract()?,
        remediation: field(finding, "remediation")?.extract()?,
        unevaluated: field(finding, "unevaluated")?.is_truthy()?,
        affected_rules: field(finding, "affected_rules")?.extract()?,
        fixable: field(finding, "fixable")?.is_truthy()?,
    })
}

pub(crate) fn rule_findings(findings: &Bound<'_, PyAny>) -> PyResult<Vec<RuleFinding>> {
    findings
        .try_iter()?
        .map(|finding| rule_finding(&finding?))
        .collect()
}
