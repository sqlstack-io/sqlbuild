use crate::debug_report::models::{
    ConnectionValue, DebugLine, DebugProvider, DebugReport, DebugStatus,
};

pub(super) fn sample_report() -> DebugReport {
    DebugReport {
        local_files: vec!["/repo/warehouse.db".into()],
        missing_secrets: 0,
        runtime: vec![DebugLine::plain("sqlbuild version", "0.2.1")],
        configuration: vec![DebugLine::checked(
            "project file",
            "/repo/sqlbuild_project.toml",
            (DebugStatus::Ok, Some("found and valid")),
        )],
        providers: vec![
            DebugLine::checked("providers", "1", (DebugStatus::Ok, Some("discovered"))),
            DebugLine::checked(
                "marker_provider",
                "providers/marker.py:MarkerProvider",
                (DebugStatus::Ok, Some("valid settings")),
            ),
        ],
        connection: vec![
            DebugLine::plain("token", "****"),
            DebugLine::checked(
                "connection test",
                "",
                (DebugStatus::Error, Some("authentication failed")),
            ),
            DebugLine::checked(
                "query test",
                "",
                (DebugStatus::Skip, Some("connection failed")),
            ),
        ],
    }
}

pub(super) fn text(value: &str) -> ConnectionValue {
    ConnectionValue::Text(value.to_owned())
}

pub(super) fn line_summary(line: &DebugLine) -> (&str, &str, Option<&str>) {
    (&line.label, &line.message, line.status_message.as_deref())
}

pub(super) fn provider(name: &str, secrets: &[(&str, bool)]) -> DebugProvider {
    DebugProvider {
        name: name.to_owned(),
        relative_path: format!("providers/{name}/provider.py"),
        class_name: "Provider".to_owned(),
        secrets: secrets
            .iter()
            .map(|(variable, set)| ((*variable).to_owned(), *set))
            .collect(),
    }
}
