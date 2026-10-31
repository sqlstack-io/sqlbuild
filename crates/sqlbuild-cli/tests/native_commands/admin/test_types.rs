pub(crate) struct AdminTestCase {
    pub(crate) description: &'static str,
    pub(crate) location_name: &'static str,
    pub(crate) expected_name: &'static str,
    pub(crate) expected_actions: [&'static str; 9],
}

pub(crate) struct PrincipalAdminTestCase {
    pub(crate) description: &'static str,
    pub(crate) kind: &'static str,
    pub(crate) expected_name: &'static str,
    pub(crate) expected_actions: [&'static str; 3],
}

pub(crate) struct TriggerAdminTestCase {
    pub(crate) description: &'static str,
    pub(crate) change: &'static str,
    pub(crate) cursor_arguments: &'static [&'static str],
    pub(crate) expected_status: &'static str,
    pub(crate) expected_cursor: Option<&'static str>,
}

#[cfg(unix)]
pub(crate) struct AdminDeliveryTestCase {
    pub(crate) description: &'static str,
    pub(crate) name_bytes: usize,
    pub(crate) expected_exit: i32,
    pub(crate) expected_rows: (i64, i64),
}
