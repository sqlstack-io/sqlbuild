//! Public constants of live warehouse reads.

use sqlbuild_host::entries::models::AdapterHost;

pub const ACTIVE_CATALOG: AdapterHost = AdapterHost::ActiveInspectionCatalog;

pub const INSPECTION_CATALOG: AdapterHost = AdapterHost::OpenInspectionCatalog;

pub const RELATIONS_MAIN_LIST: AdapterHost = AdapterHost::ListRelationsForInspection;

pub const RELATIONS_MAIN_COLUMNS: AdapterHost = AdapterHost::GetColumnsForInspection;

pub const RECORD_QUERY: AdapterHost = AdapterHost::RecordInspectionQuery;

pub const RUN_RECORDED_QUERY: AdapterHost = AdapterHost::RunRecordedInspectionQuery;

pub const QUERY_RECORD: AdapterHost = AdapterHost::InspectionQueryRecord;

/// The adapter method that runs one statement on a connection.
pub const EXECUTE: &str = "execute";
