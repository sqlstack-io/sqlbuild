//! `compile_lineage`: each compiled model's column lineage from SQLQuack, for Snowflake and DuckDB.

use rayon::ThreadPool;
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use sqlbuild_core::panics::main::catch_compiler_panic::catch_compiler_panic;

use crate::sqlquack_analysis::_helpers::compile::model_lineage;
use crate::sqlquack_analysis::_helpers::schema::sqlquack_dialect;
use crate::sqlquack_analysis::models::{CompileLineageColumn, CompiledModelSql};

/// Each model's lineage, `None` for one SQLQuack cannot read; `None` overall for another dialect.
pub fn compile_lineage(
    dialect: &str,
    models: &[CompiledModelSql<'_>],
    pool: &ThreadPool,
) -> Result<Option<Vec<Option<Vec<CompileLineageColumn>>>>, String> {
    let Some((dialect, profile)) = sqlquack_dialect(dialect) else {
        return Ok(None);
    };
    pool.install(|| {
        models
            .par_iter()
            .map(|model| catch_compiler_panic(|| Ok(model_lineage(model, dialect, profile))))
            .collect::<Result<Vec<_>, String>>()
    })
    .map(Some)
}
