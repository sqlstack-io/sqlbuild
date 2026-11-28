/// Each listed source's name and its declared columns.
pub(super) type ListedSources =
    &'static [(&'static str, &'static [(&'static str, &'static str)])];

pub(super) struct CheckLoadTestCase {
    pub(super) description: &'static str,
    pub(super) settings: &'static str,
    pub(super) sources: ListedSources,
    /// The `on_schema_change` every listed source declares.
    pub(super) on_schema_change: Option<&'static str>,
    pub(super) expected_result: Result<&'static str, (&'static str, &'static [&'static str])>,
}

pub(super) struct YamlLineTestCase {
    pub(super) description: &'static str,
    pub(super) path: &'static [&'static str],
    pub(super) expected_line: Option<usize>,
}

pub(super) struct LoadMessageTestCase {
    pub(super) description: &'static str,
    pub(super) message: &'static str,
    pub(super) expected_message: &'static str,
}

pub(super) struct CheckLoaderTestCase {
    pub(super) description: &'static str,
    pub(super) connection: Option<&'static str>,
    pub(super) settings: &'static str,
    /// The `on_schema_change` the source it fills declares.
    pub(super) on_schema_change: Option<&'static str>,
    /// The expected table settings as TOML text, or the message.
    pub(super) expected_result: Result<&'static str, &'static str>,
}

pub(super) struct LoaderSourceTestCase {
    pub(super) description: &'static str,
    pub(super) connection: Option<&'static str>,
    pub(super) settings: &'static str,
    pub(super) expected_source: &'static str,
}

pub(super) struct DeclaredTypeTestCase {
    pub(super) description: &'static str,
    pub(super) warehouse: sqlload::column_types::models::Warehouse,
    pub(super) declared: &'static str,
    /// The type SQLLoad creates when it differs from the declared one, compared by name.
    pub(super) expected_created: Option<&'static str>,
}

pub(super) struct SqlloadColumnTypeTestCase {
    pub(super) description: &'static str,
    pub(super) warehouse: sqlload::column_types::models::Warehouse,
    pub(super) declared: &'static str,
    pub(super) expected_sqlload_type: &'static str,
}

pub(super) struct AdviceRoundTripTestCase {
    pub(super) description: &'static str,
    pub(super) warehouse: sqlload::column_types::models::Warehouse,
    /// Whether declaring the advice for each logical type creates the type SQLLoad created.
    pub(super) expected_advice_creates_exactly: bool,
}
