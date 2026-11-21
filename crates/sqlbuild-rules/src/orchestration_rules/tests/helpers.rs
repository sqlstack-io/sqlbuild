use serde_json::json;

use crate::engine::_helpers::memo::evaluate_rows;
use crate::models::{EvaluateRequest, RowsRequest};

/// A job declaration selecting `select`.
pub(super) fn job(name: &str, select: &str) -> String {
    format!("name = \"{name}\"\ndescription = \"d\"\nselect = \"{select}\"\n")
}

/// A sensor module whose `when` is `condition`.
pub(super) fn sensor(name: &str, condition: &str) -> String {
    format!(
        "from sqlbuild.orchestration import node_event, sensor, source_new_data\n\nSELECTION = \"a\"\n@sensor(description=\"d\", when={condition})\ndef {name}_sensor(ctx):\n    return None\n"
    )
}

/// The job the sensors run.
pub(super) fn export_job() -> (&'static str, String) {
    (
        "orchestration/jobs/orders/export.toml",
        job("orders_export_job", "orders"),
    )
}

/// The named-selector findings for a project of `files`, as path, line, column and message.
pub(super) fn limit_findings(
    files: &[(&str, String)],
    config: serde_json::Value,
) -> Result<Vec<(String, u64, u64, String)>, String> {
    let directory = tempfile::tempdir().map_err(|error| error.to_string())?;
    for (path, contents) in files {
        let file = directory.path().join(path);
        std::fs::create_dir_all(file.parent().expect("parent"))
            .map_err(|error| error.to_string())?;
        std::fs::write(&file, contents).map_err(|error| error.to_string())?;
    }
    let mut config = config;
    config["cache"] = json!({ "enabled": false });
    let request = EvaluateRequest {
        project_dir: directory.path().to_string_lossy().into_owned(),
        config: serde_json::from_value(config).map_err(|error| error.to_string())?,
        ..EvaluateRequest::default()
    };
    let evaluation = evaluate_rows(RowsRequest {
        request,
        memo: None,
        retain: false,
    })
    .map_err(|error| format!("{error:?}"))?;
    Ok(evaluation
        .faults
        .into_iter()
        .filter(|fault| fault.code == "SQBRORCHESTRATION101")
        .map(|fault| (fault.path, fault.line, fault.column, fault.message))
        .collect())
}

/// Expected findings as owned path, line, column and message tuples.
pub(super) fn owned_findings(items: &[(&str, u64, u64, &str)]) -> Vec<(String, u64, u64, String)> {
    items
        .iter()
        .map(|(path, line, column, message)| {
            ((*path).to_owned(), *line, *column, (*message).to_owned())
        })
        .collect()
}
