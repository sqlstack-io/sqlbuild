use std::collections::BTreeSet;

use sqlbuild_core::plan_display::errors::UnknownCursorGrain;

use crate::planner_strategy::models::{
    AuditAttachmentKind, CapDirection, CappedBound, ChangeFacts, CursorInputColumnCheck,
    CursorType, LookbackConfig, MaterializationType, PlanAction, PlanReason, SchemaActionRow,
    WarningRow,
};

pub(super) struct ActionTestCase {
    pub(super) description: &'static str,
    pub(super) materialization: MaterializationType,
    pub(super) strategy: Option<&'static str>,
    pub(super) change: ChangeFacts,
    pub(super) full_refresh: bool,
    pub(super) expected_action: Result<(PlanAction, PlanReason), String>,
}

pub(super) struct SchemaActionTestCase {
    pub(super) description: &'static str,
    pub(super) on_schema_change: Option<&'static str>,
    pub(super) expected_actions: Vec<SchemaActionRow>,
}

pub(super) struct WarningTestCase {
    pub(super) description: &'static str,
    pub(super) on_schema_change: Option<&'static str>,
    pub(super) type_enforcement: bool,
    pub(super) change: ChangeFacts,
    pub(super) expected_warnings: Vec<WarningRow>,
}

pub(super) struct ClassifyTestCase {
    pub(super) description: &'static str,
    pub(super) sql_type: &'static str,
    pub(super) expected_cursor_type: Option<CursorType>,
}

pub(super) struct IntegerBatchTestCase {
    pub(super) description: &'static str,
    pub(super) bounds: (i128, i128),
    pub(super) batch_size: &'static str,
    pub(super) direction: CapDirection,
    pub(super) expected_count: i128,
    pub(super) expected_cap: Option<CappedBound<i128>>,
}

pub(super) struct TimelineBatchTestCase {
    pub(super) description: &'static str,
    pub(super) bounds: (i64, i64),
    pub(super) max_batches: i128,
    pub(super) direction: CapDirection,
    pub(super) expected_count: i128,
    pub(super) expected_cap: CappedBound<i64>,
}

pub(super) struct AttachmentTestCase {
    pub(super) description: &'static str,
    pub(super) models: Vec<String>,
    pub(super) sources: Vec<String>,
    pub(super) expected_attachment: (AuditAttachmentKind, Option<String>),
}

pub(super) struct LineageTestCase {
    pub(super) description: &'static str,
    pub(super) input: &'static str,
    pub(super) capped: &'static [&'static str],
    pub(super) expected_reference: Result<String, String>,
    pub(super) expected_capped: Result<Option<String>, String>,
}

pub(super) struct LimitWarningTestCase {
    pub(super) description: &'static str,
    pub(super) limit: (Option<i128>, i128),
    pub(super) action: (&'static str, bool),
    pub(super) expected_warning: Option<&'static str>,
}

/// Whole numbers stepping by three.
pub(super) struct Steps;

/// `orders` reads `customers` and the `discount` function; `discount` reads `products`; `customers` is a microbatch model reading `events` as its watermark input.
pub(super) struct Lineage {
    pub(super) capped: BTreeSet<String>,
}

pub(super) struct CursorInputColumnTestCase {
    pub(super) description: &'static str,
    pub(super) check: CursorInputColumnCheck,
    pub(super) expected_failure: Option<(String, &'static str)>,
}

pub(super) struct LookbackTestCase {
    pub(super) description: &'static str,
    pub(super) config: LookbackConfig,
    pub(super) expected_lookback: Result<Option<String>, UnknownCursorGrain>,
}

pub(super) struct TableTypeDdlTestCase {
    pub(super) description: &'static str,
    pub(super) ddl: &'static str,
    pub(super) table_type: &'static str,
    pub(super) expected_ddl: &'static str,
}

pub(super) struct PhysicalLayoutWarningTestCase {
    pub(super) description: &'static str,
    pub(super) materialization: MaterializationType,
    pub(super) table_recreated: bool,
    pub(super) previous_metadata_json: Option<&'static str>,
    pub(super) current_metadata_json: &'static str,
    pub(super) expected_message: Option<&'static str>,
}
