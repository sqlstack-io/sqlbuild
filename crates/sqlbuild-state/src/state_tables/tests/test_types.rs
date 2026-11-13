use crate::state_tables::models::TableColumn;

pub(crate) struct CreateTableTestCase {
    pub(crate) description: &'static str,
    pub(crate) columns: Vec<TableColumn>,
    pub(crate) expected_sql: &'static str,
}

pub(crate) struct ValuesInsertTestCase {
    pub(crate) description: &'static str,
    pub(crate) rows: Vec<Vec<String>>,
    pub(crate) expected_sql: &'static str,
}

pub(crate) struct HistoryReadTestCase {
    pub(crate) description: &'static str,
    pub(crate) adapter: &'static str,
    pub(crate) has_event_id: bool,
    pub(crate) after: Option<Vec<Option<String>>>,
    pub(crate) expected_sql: &'static str,
}

pub(crate) struct KeyReadTestCase {
    pub(crate) description: &'static str,
    pub(crate) has_event_id: bool,
    pub(crate) rows: Vec<Vec<Option<String>>>,
    pub(crate) expected_sql: &'static str,
}
