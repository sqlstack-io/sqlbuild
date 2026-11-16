//! Format redundant fixture nulls.

use crate::rules::constants::MODEL_TEST_MODE;
use pyo3::exceptions::{PyOSError, PySyntaxError, PyUnicodeError, PyValueError};
use pyo3::prelude::{Bound, PyAny, PyAnyMethods, PyResult};
use pyo3::pyfunction;
use pyo3::types::{PyDict, PyDictMethods, PyStringMethods};
use sqlbuild_python_base::boundary::main::compiler_guard::compiler_guard;
use sqlbuild_python_base::boundary::main::value_error::value_error;
use sqlbuild_python_base::records::main::fields::field::field as native_field;
use sqlbuild_python_front::discovery::main::records::parse_sql_test_file::parse_sql_test_file;
use sqlbuild_python_front::sqltext::models::LexicalSyntaxInput;
use sqlbuild_rules::lint_text::main::format_fixture_blocks::format_fixture_blocks;
use sqlbuild_rules::lint_text::models::FixtureFormatBlock;
use sqlbuild_sqltext::sql_scan::models::LexicalSyntax;

#[pyfunction]
#[pyo3(signature = (*, files, project_dir, discovered_inputs, sql_lexical_syntax))]
pub fn format_redundant_fixture_nulls<'py>(
    files: &Bound<'py, PyDict>,
    project_dir: &Bound<'py, PyAny>,
    discovered_inputs: &Bound<'py, PyAny>,
    sql_lexical_syntax: &Bound<'py, PyAny>,
) -> PyResult<Bound<'py, PyDict>> {
    compiler_guard(|| {
        let py = files.py();
        let relations = crate::rules::_helpers::sql::fixture_schemas::relations(discovered_inputs)?;
        let syntax: LexicalSyntax = native_field(sql_lexical_syntax, "native_mapping")?
            .extract::<LexicalSyntaxInput>()?
            .into();
        let updated = PyDict::new(py);
        for file in native_field(discovered_inputs, "test_files")?.try_iter()? {
            let file = file?;
            let relative = native_field(&file, "relative_path")?;
            let path = project_dir
                .call_method1("__truediv__", (&relative,))?
                .call_method0("resolve")?;
            let Some(original) = files.get_item(&path)? else {
                continue;
            };
            let contents: String = updated.get_item(&path)?.unwrap_or(original).extract()?;
            let blocks = match parse_sql_test_file(contents.clone(), &path) {
                Ok(blocks) => blocks,
                Err(error)
                    if error.is_instance_of::<PyOSError>(py)
                        || error.is_instance_of::<PyUnicodeError>(py)
                        || error.is_instance_of::<PyValueError>(py)
                        || error.is_instance_of::<PySyntaxError>(py) =>
                {
                    continue;
                }
                Err(error) => return Err(error),
            };
            let blocks = blocks
                .try_iter()?
                .map(|block| {
                    let block = block?;
                    Ok(FixtureFormatBlock {
                        sql_body: native_field(&block, "sql_body")?.extract()?,
                        model_mode: native_field(&native_field(&block, "mode")?, "value")?
                            .extract::<String>()?
                            == MODEL_TEST_MODE,
                    })
                })
                .collect::<PyResult<Vec<_>>>()?;
            let label = relative.str()?.to_str()?.to_owned();
            if let Some(result) =
                format_fixture_blocks((&contents, &label), &blocks, &relations, &syntax)
                    .map_err(value_error)?
            {
                updated.set_item(path, result)?;
            }
        }
        Ok(updated)
    })
}
