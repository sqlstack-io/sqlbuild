//! Write the executed SQL unit-test comparison statements under `target/run/tests`.

use crate::analysis::main::single_comparison_sql::single_comparison_sql;
use crate::compile_outputs::_helpers::runtime_target::RUN_DIR;
use crate::compile_outputs::_helpers::runtime_target::TESTS_DIR;
use crate::compile_outputs::_helpers::runtime_target::TestKey;
use crate::compile_outputs::_helpers::runtime_target::test_key;
use crate::compile_outputs::_helpers::runtime_target::write_text;
use crate::compile_outputs::main::planned_sql_test_path::planned_sql_test_path;
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use sqlbuild_core::text::main::python_isspace::python_isspace;
use sqlbuild_host::models::SqlTestExecutionResult;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use std::collections::HashSet;

/// Write the executed SQL unit-test comparison statements under `target/run/tests`.
pub fn write_test_runtime_target(
    (target_dir, adapter): (&Bound<'_, PyAny>, &Bound<'_, PyAny>),
    plan: &Bound<'_, PyAny>,
    results: &[SqlTestExecutionResult<Bound<'_, PyAny>>],
) -> PyResult<()> {
    let mut executed: HashSet<TestKey> = HashSet::new();
    for result in results {
        executed.insert(TestKey {
            name: result.test_name.clone(),
            source_path: result.source_path.as_ref().map(|path| {
                path.to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/")
            }),
            block_index: result.block_index,
            case_name: result.case_name.clone(),
        });
    }
    let tests_dir: Bound<'_, PyAny> = target_dir.div(RUN_DIR)?.div(TESTS_DIR)?;
    for entry in native_field(plan, "test_entries")?.try_iter()? {
        let entry: Bound<'_, PyAny> = entry?;
        if !executed.contains(&test_key(&entry, "name")?) {
            continue;
        }
        let sql: String = single_comparison_sql(&entry, adapter)?;
        write_text(
            &tests_dir.div(planned_sql_test_path(&entry)?)?,
            &format!("{}\n", sql.trim_end_matches(python_isspace)),
        )?;
    }
    Ok(())
}
