use crate::command_connection::errors::CommandConnectionError;

pub(super) struct CliWarehouseTestCase {
    pub(super) description: &'static str,
    pub(super) cli_warehouse: &'static str,
    pub(super) expected_message: Option<&'static str>,
}

pub(super) struct JobWarehouseTestCase {
    pub(super) description: &'static str,
    pub(super) value: Option<&'static str>,
    pub(super) expected_result: Result<&'static str, &'static str>,
}

pub(super) struct SnowflakeKeyTestCase {
    pub(super) description: &'static str,
    pub(super) adapter_name: &'static str,
    pub(super) keys: &'static [&'static str],
    pub(super) expected_warning: Option<&'static str>,
}

pub(super) struct ExpandedWarehouseTestCase {
    pub(super) description: &'static str,
    pub(super) local_value: Option<&'static str>,
    pub(super) project_value: Option<&'static str>,
    pub(super) expanded: &'static str,
    pub(super) expected_result: Result<(), CommandConnectionError>,
}
