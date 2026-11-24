use crate::dispatch::models::{
    CommandFacts, Dispatch, DispatchFailure, ExportSummary, ExporterLoss, Route,
};

/// Facts of `command` and `subcommand` with every flag off and every required value present.
pub(super) fn facts(command: &str, subcommand: Option<&str>) -> CommandFacts {
    CommandFacts {
        command: Some(command.to_owned()),
        subcommand: subcommand.map(str::to_owned),
        from_target: true,
        contract_ready: true,
        ..CommandFacts::default()
    }
}

/// The dispatch that runs `route` with no pending failure.
pub(super) fn runs(route: &'static Route) -> Dispatch {
    Dispatch::Run(route, None)
}

/// A user failure with `message`, `code` and no help.
pub(super) fn failure(message: &str, code: &'static str) -> DispatchFailure {
    DispatchFailure {
        message: message.to_owned(),
        code,
        help: None,
    }
}

/// An export summary of `(name, accepted, delivered, failed, dropped)` sinks.
pub(super) fn export_summary(exporters: &[(&str, u64, u64, u64, u64)]) -> ExportSummary {
    ExportSummary {
        teardown_failures: Vec::new(),
        accepted: exporters.iter().map(|exporter| exporter.1).sum(),
        dropped: exporters.iter().map(|exporter| exporter.4).sum(),
        failed: exporters.iter().map(|exporter| exporter.3).sum(),
        per_exporter: exporters
            .iter()
            .map(|exporter| ExporterLoss {
                exporter_name: exporter.0.to_owned(),
                dropped: exporter.4,
                failed: exporter.3,
            })
            .collect(),
    }
}
