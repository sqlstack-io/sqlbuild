//! SQL test plan entries and results as the command renders them.

/// One typed parameter value of a parameterized test case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestParameter {
    pub name: String,
    pub value_type: String,
    pub nullable: bool,
    /// The value as compact ASCII JSON.
    pub rendered: String,
}

/// One selected test as `--inspect` shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestInspectionEntry {
    pub label: String,
    pub mocked_sources: Vec<String>,
    pub mocked_refs: Vec<String>,
    pub mocked_seeds: Vec<String>,
    pub mocked_dbt_refs: Vec<String>,
    pub mocked_table_functions: Vec<String>,
    pub real_models: Vec<String>,
    pub expected_models: Vec<String>,
    /// Why `--since` selected the test; empty without `--since`.
    pub selection_reasons: Vec<String>,
}

/// One plan warning: its severity value and message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestPlanWarning {
    pub severity: String,
    pub message: String,
}

/// One test that did not pass.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TestFailure {
    pub label: String,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub error_help: Option<String>,
    /// Whether the test was refused (counted under REFUSED), whatever its code.
    pub refused: bool,
}
