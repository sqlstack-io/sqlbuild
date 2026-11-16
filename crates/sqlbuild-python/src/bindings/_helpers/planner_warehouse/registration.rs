//! Register native warehouse inspection for planning.

use pyo3::prelude::{Bound, PyModule, PyModuleMethods, PyResult};
use sqlbuild_python_planner::planner_warehouse::main::inspection::gather_warehouse_snapshot_function::gather_warehouse_snapshot_function;
use sqlbuild_python_planner::planner_warehouse::main::inspection::migration_fingerprint_work_info_function::migration_fingerprint_work_info_function;
use sqlbuild_python_planner::planner_warehouse::main::inspection::planner_metadata_name_filter_function::planner_metadata_name_filter_function;
use sqlbuild_python_planner::planner_warehouse::main::inspection::watermark_type_is_compatible_function::watermark_type_is_compatible_function;
use sqlbuild_python_planner::planner_warehouse::main::sources::build_planner_source_freshness_result_of_function::build_planner_source_freshness_result_of_function;
use sqlbuild_python_planner::planner_warehouse::main::sources::check_selected_source_tables_exist_binding_function::check_selected_source_tables_exist_binding_function;
use sqlbuild_python_planner::planner_warehouse::main::sources::gather_source_columns_function::gather_source_columns_function;
use sqlbuild_python_planner::planner_warehouse::main::sources::list_source_candidates_function::list_source_candidates_function;
use sqlbuild_python_planner::planner_warehouse::main::sources::semantic_source_columns_function::semantic_source_columns_function;

pub(crate) fn register(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module
        .add_class::<sqlbuild_python_planner::planner_warehouse::models::MigrationFingerprintCache>(
        )?;
    module.add_function(migration_fingerprint_work_info_function(module)?)?;
    module.add_function(gather_warehouse_snapshot_function(module)?)?;
    module.add_function(watermark_type_is_compatible_function(module)?)?;
    module.add_function(gather_source_columns_function(module)?)?;
    module.add_function(planner_metadata_name_filter_function(module)?)?;
    module.add_function(list_source_candidates_function(module)?)?;
    module.add_function(semantic_source_columns_function(module)?)?;
    module.add_function(check_selected_source_tables_exist_binding_function(module)?)?;
    module.add_function(build_planner_source_freshness_result_of_function(module)?)
}
