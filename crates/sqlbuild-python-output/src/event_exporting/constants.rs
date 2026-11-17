//! Public constants of event exporting.

use crate::event_exporting::_helpers::getters::read_bound_event_exporter;
use crate::event_exporting::_helpers::getters::read_event_export_summary;
use crate::event_exporting::_helpers::getters::read_event_exporter_accounting;
use crate::event_exporting::_helpers::getters::read_event_exporter_command_scope;
use crate::event_exporting::_helpers::getters::read_event_exporter_counts;
use crate::event_exporting::_helpers::getters::read_event_exporter_dispatcher;
use crate::event_exporting::_helpers::getters::read_event_exporter_failure;
use crate::event_exporting::models::EventExporterDispatcher;
use sqlbuild_python_base::event_exporting::models::BoundEventExporter;
use sqlbuild_python_base::event_exporting::models::EventExportSummary;
use sqlbuild_python_base::event_exporting::models::EventExporterAccounting;
use sqlbuild_python_base::event_exporting::models::EventExporterCommandScope;
use sqlbuild_python_base::event_exporting::models::EventExporterCounts;
use sqlbuild_python_base::event_exporting::models::EventExporterFailure;
use sqlbuild_python_base::records::main::registry::class_of::class_of;
use sqlbuild_python_base::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<EventExporterDispatcher>,
        read: read_event_exporter_dispatcher,
    },
    GetterEntry {
        class: class_of::<BoundEventExporter>,
        read: read_bound_event_exporter,
    },
    GetterEntry {
        class: class_of::<EventExportSummary>,
        read: read_event_export_summary,
    },
    GetterEntry {
        class: class_of::<EventExporterAccounting>,
        read: read_event_exporter_accounting,
    },
    GetterEntry {
        class: class_of::<EventExporterCounts>,
        read: read_event_exporter_counts,
    },
    GetterEntry {
        class: class_of::<EventExporterFailure>,
        read: read_event_exporter_failure,
    },
    GetterEntry {
        class: class_of::<EventExporterCommandScope>,
        read: read_event_exporter_command_scope,
    },
];
