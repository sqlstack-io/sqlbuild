//! `format_sql_bodies`: SQLBuild-owned safe formatting of every SQL body in files.

use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use crate::lint_text::_helpers::format_bodies::{
    PendingBody, RequestTable, prepare_file, splice_file,
};
use crate::lint_text::errors::LintTextError;
use crate::lint_text::models::{FormatFault, FormatFile, FormattedBodies, PreparedBody};
use sqlclean::sql_lint::main::format_requests::format_requests;
use sqlclean::sql_lint::models::FormatResponse;

/// Format SQL bodies, reporting every declined body as a fault and leaving it unchanged.
pub fn format_sql_bodies(
    files: &[FormatFile],
    dialect: &str,
    line_width: usize,
) -> Result<FormattedBodies, LintTextError> {
    let pending: Vec<Result<Vec<PendingBody>, LintTextError>> = files
        .par_iter()
        .map(|file| prepare_file(file, dialect))
        .collect();
    let mut table: RequestTable = RequestTable::default();
    let mut prepared: Vec<Vec<PreparedBody>> = Vec::with_capacity(files.len());
    for file_bodies in pending {
        let mut bodies: Vec<PreparedBody> = Vec::new();
        for (mut body, sql, widths) in file_bodies? {
            body.request = table.insert(sql, widths, (dialect, line_width));
            bodies.push(body);
        }
        prepared.push(bodies);
    }
    let responses: Vec<FormatResponse> = format_requests(table.requests);
    let spliced: Vec<(Option<String>, Vec<FormatFault>)> = files
        .par_iter()
        .zip(prepared.par_iter())
        .enumerate()
        .map(|(index, (file, bodies))| splice_file(index, &file.contents, bodies, &responses))
        .collect();
    let mut result: FormattedBodies = FormattedBodies::default();
    for (index, (formatted, faults)) in spliced.into_iter().enumerate() {
        result.formatted.extend(formatted.map(|text| (index, text)));
        result.faults.extend(faults);
    }
    Ok(result)
}
