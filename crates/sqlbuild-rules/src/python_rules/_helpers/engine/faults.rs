//! Fensu findings as `SQBRPYTHON` faults at SQLBuild positions.

use fensu_native::NativeFaultRow;

use crate::models::Fault;
use crate::python_rules::_helpers::engine::programs::Programs;
use crate::python_rules::constants::{CATALOGUE_REMEDIATION_CODES, PACKAGE_ROOT, PYTHON_RULES};
use crate::python_rules::models::{PythonFiles, PythonRuleSpec, PythonTarget};

/// `row` under its `SQBRPYTHON` code on its file, or none when a `python/` rule reports outside `python/`.
pub(crate) fn python_fault(
    row: NativeFaultRow,
    target: &PythonTarget,
    programs: &Programs<'_>,
) -> Result<Option<Fault>, String> {
    let rule: &PythonRuleSpec = PYTHON_RULES
        .iter()
        .find(|rule| rule.fensu_code == Some(row.code.as_str()))
        .ok_or_else(|| {
            format!(
                "Fensu reported rule {} that no SQBRPYTHON rule runs",
                row.code
            )
        })?;
    let path: String = row.path.unwrap_or_else(|| target.repository_path.clone());
    if rule.applies == PythonFiles::Package && !path.starts_with(&format!("{PACKAGE_ROOT}/")) {
        return Ok(None);
    }
    let line: u32 = row.line.max(1);
    let column: u64 = programs.index_of(&path).map_or(1, |index| {
        character_column(&programs.sources[index].text, line, row.column)
    });
    Ok(Some(Fault {
        unevaluated: false,
        code: rule.code.to_owned(),
        path,
        line: u64::from(line),
        column,
        message: row.message.unwrap_or_else(|| rule.message.to_owned()),
        remediation: row
            .remediation
            .filter(|_| !CATALOGUE_REMEDIATION_CODES.contains(&row.code.as_str()))
            .unwrap_or_else(|| rule.remediation.to_owned()),
    }))
}

/// The 1-based character column of 0-based UTF-8 byte offset `column` on 1-based `line`.
pub(crate) fn character_column(text: &str, line: u32, column: u32) -> u64 {
    let Some(start): Option<usize> = line_start(text, line) else {
        return 1;
    };
    let rest: &str = &text[start..];
    let end: usize = rest.find(['\n', '\r']).unwrap_or(rest.len());
    let mut offset: usize = (column as usize).min(end);
    while !rest.is_char_boundary(offset) {
        offset -= 1;
    }
    rest[..offset].chars().count() as u64 + 1
}

/// The byte offset where 1-based `line` starts, ending lines at `\n`, `\r\n` or `\r` as Python does.
fn line_start(text: &str, line: u32) -> Option<usize> {
    let mut remaining: u32 = line.checked_sub(1)?;
    let bytes: &[u8] = text.as_bytes();
    let mut index: usize = 0;
    while remaining > 0 {
        let next: usize = index
            + bytes[index..]
                .iter()
                .position(|byte| matches!(byte, b'\n' | b'\r'))?;
        index = if bytes[next] == b'\r' && bytes.get(next + 1) == Some(&b'\n') {
            next + 2
        } else {
            next + 1
        };
        remaining -= 1;
    }
    Some(index)
}

/// A fault of the catalogue rule `code` on `path` at `(line, column)`, its message followed by `detail`.
pub(crate) fn catalogue_fault(
    code: &str,
    path: &str,
    (line, column): (u64, u64),
    detail: &str,
) -> Option<Fault> {
    let rule = PYTHON_RULES.iter().find(|rule| rule.code == code)?;
    Some(Fault {
        unevaluated: false,
        code: code.to_owned(),
        path: path.to_owned(),
        line,
        column,
        message: format!("{}: {detail}", rule.message),
        remediation: rule.remediation.to_owned(),
    })
}
