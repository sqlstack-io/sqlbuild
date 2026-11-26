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
    /// Where the test runs and why; `None` when the run did not decide.
    pub run_on: Option<crate::run_on::models::RunOnDecision>,
}

/// One plan warning: its severity value and message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TestPlanWarning {
    pub severity: String,
    pub message: String,
}

/// What a failed test's `Failures:` entry and its `Failed (N):` line are built from.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TestFailureFacts {
    /// The full name, never cut.
    pub name: String,
    /// `test` or `scenario`.
    pub kind: String,
    /// `local` or `warehouse`; empty when the command doesn't say.
    pub run_on: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub error_help: Option<String>,
    pub timed_out: bool,
    pub steps: Vec<crate::progress::models::ExpectationStep>,
    /// The phase and leading keyword of the statement that failed, when one did.
    pub statement: Option<(String, String)>,
    /// The compiled SQL the item ran, under `target/`.
    pub compiled_path: Option<String>,
    /// The file the test or scenario is written in.
    pub source_path: Option<String>,
}
