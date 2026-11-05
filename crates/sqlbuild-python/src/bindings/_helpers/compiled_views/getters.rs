//! Direct `#[getter]` reads of this module's native pyclasses, installed into `records::getters`.

use pyo3::IntoPyObjectExt;
use pyo3::prelude::{Bound, PyAny, PyResult, Python};

use crate::bindings::_helpers::compiled_views::audit_views::{
    NativeAuditInput, NativeCompiledAudit,
};
use crate::bindings::_helpers::compiled_views::compiled_model_view::NativeCompiledModel;
use crate::bindings::_helpers::compiled_views::function_views::{
    NativeCompiledFunction, NativeFunctionInput,
};
use crate::bindings::_helpers::compiled_views::inputs_view::NativeInputsView;
use crate::bindings::_helpers::compiled_views::model_input_view::NativeModelInput;
use crate::bindings::_helpers::compiled_views::project_view::NativeProjectView;
use crate::bindings::_helpers::compiled_views::seed_views::{NativeCompiledSeed, NativeSeedInput};
use crate::bindings::_helpers::compiled_views::source_views::{
    NativeCompiledSource, NativeSourceInput,
};
use crate::bindings::_helpers::records::getters::class_of;
use crate::bindings::_helpers::records::models::GetterEntry;

/// The getter readers of this module's native pyclasses.
pub(crate) const GETTERS: &[GetterEntry] = &[
    GetterEntry {
        class: class_of::<NativeAuditInput>,
        read: read_native_audit_input,
    },
    GetterEntry {
        class: class_of::<NativeCompiledAudit>,
        read: read_native_compiled_audit,
    },
    GetterEntry {
        class: class_of::<NativeCompiledModel>,
        read: read_native_compiled_model,
    },
    GetterEntry {
        class: class_of::<NativeCompiledFunction>,
        read: read_native_compiled_function,
    },
    GetterEntry {
        class: class_of::<NativeFunctionInput>,
        read: read_native_function_input,
    },
    GetterEntry {
        class: class_of::<NativeInputsView>,
        read: read_native_inputs_view,
    },
    GetterEntry {
        class: class_of::<NativeModelInput>,
        read: read_native_model_input,
    },
    GetterEntry {
        class: class_of::<NativeProjectView>,
        read: read_native_project_view,
    },
    GetterEntry {
        class: class_of::<NativeCompiledSeed>,
        read: read_native_compiled_seed,
    },
    GetterEntry {
        class: class_of::<NativeSeedInput>,
        read: read_native_seed_input,
    },
    GetterEntry {
        class: class_of::<NativeCompiledSource>,
        read: read_native_compiled_source,
    },
    GetterEntry {
        class: class_of::<NativeSourceInput>,
        read: read_native_source_input,
    },
];

fn read_native_audit_input<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeAuditInput>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "audit_file" => this
            .audit_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "audit_block" => this
            .audit_block(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "sql_body" => this
            .sql_body(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "evaluation_mode" => this
            .evaluation_mode(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "measurement_contract" => this
            .measurement_contract(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "thresholds" => this
            .thresholds(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "minimum_samples" => this
            .minimum_samples(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "measure_sql" => this
            .measure_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "evidence_sql" => this
            .evidence_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "evidence_limit" => this
            .evidence_limit(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "references" => this
            .references(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "attached_target_kind" => this
            .attached_target_kind(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "attached_target_name" => this
            .attached_target_name(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "attached_column_name" => this
            .attached_column_name(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "severity" => this
            .severity(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "run_scope" => this
            .run_scope(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "always_run" => this
            .always_run(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "description" => this
            .description(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "meta" => this.meta(py).and_then(|value| value.into_bound_py_any(py)),
        "declaration_usages" => this
            .declaration_usages(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "name" => this.name(py).and_then(|value| value.into_bound_py_any(py)),
        "assembly_row" => this
            .assembly_row(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_native_compiled_audit<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeCompiledAudit>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "audit_file" => this
            .audit_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "audit_block" => this
            .audit_block(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "sql_body" => this
            .sql_body(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "evaluation_mode" => this
            .evaluation_mode(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "measurement_contract" => this
            .measurement_contract(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "thresholds" => this
            .thresholds(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "minimum_samples" => this
            .minimum_samples(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "measure_sql" => this
            .measure_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "evidence_sql" => this
            .evidence_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "evidence_limit" => this
            .evidence_limit(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "references" => this
            .references(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "attached_target_kind" => this
            .attached_target_kind(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "attached_target_name" => this
            .attached_target_name(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "attached_column_name" => this
            .attached_column_name(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "severity" => this
            .severity(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "run_scope" => this
            .run_scope(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "always_run" => this
            .always_run(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "description" => this
            .description(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "meta" => this.meta(py).and_then(|value| value.into_bound_py_any(py)),
        "key" => this.key(py).and_then(|value| value.into_bound_py_any(py)),
        "scope_deps" => this
            .scope_deps(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "name" => this.name(py).and_then(|value| value.into_bound_py_any(py)),
        "definition_name" => this
            .definition_name(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_native_compiled_model<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeCompiledModel>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "key" => this.key(py).and_then(|value| value.into_bound_py_any(py)),
        "deps" => this.deps(py).and_then(|value| value.into_bound_py_any(py)),
        "name" => this.name(py).and_then(|value| value.into_bound_py_any(py)),
        "relative_path" => this
            .relative_path(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "query_sql" => this
            .query_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "config" => this
            .config(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "destination" => this
            .destination(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "references" => this
            .references(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "reference_rows" => this
            .reference_rows(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "relation_key_row" => this
            .relation_key_row(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "key_row" => this
            .key_row(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "dep_rows" => this
            .dep_rows(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "destination_row" => this
            .destination_row(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "inferred_rows" => this
            .inferred_rows(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "schema_column_rows" => this
            .schema_column_rows(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "has_dynamic_columns" => this
            .dynamic_columns_declared(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "output_column_count" => this
            .output_column_count(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "schema_entry" => this
            .schema_entry(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "inferred_columns" => this
            .inferred_columns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "fast_lineage_columns" => this
            .fast_lineage_columns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "fast_lineage_has_star" => this
            .fast_lineage_has_star(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "fast_lineage_star_resolved" => this
            .fast_lineage_star_resolved(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "authored_sql" => this
            .authored_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "authored_query_sql" => this
            .authored_query_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "output_column_locations" => this
            .output_column_locations(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "extract_implicit_alias_columns" => this
            .extract_implicit_alias_columns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "macro_deps" => this
            .macro_deps(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "enum_declarations" => this
            .enum_declarations(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "constant_declarations" => this
            .constant_declarations(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "enum_columns" => this
            .enum_columns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "binding_diagnostics" => this
            .binding_diagnostics(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "binding_validated" => this
            .binding_validated(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "dynamic_column_contract" => this
            .dynamic_column_contract(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "unchecked_output_columns" => this
            .unchecked_output_columns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "rejected_sql_analysis_opt_out" => this
            .rejected_sql_analysis_opt_out(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_native_compiled_function<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeCompiledFunction>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "name" => this.name(py).and_then(|value| value.into_bound_py_any(py)),
        "arguments" => this
            .arguments(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "returns" => this
            .returns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "body_sql" => this
            .body_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "return_columns" => this
            .return_columns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "references" => this
            .references(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "language" => this
            .language(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "runtime_version" => this
            .runtime_version(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "entry_point" => this
            .entry_point(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "packages" => this
            .packages(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "tags" => this.tags(py).and_then(|value| value.into_bound_py_any(py)),
        "description" => this
            .description(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "key" => this.key(py).and_then(|value| value.into_bound_py_any(py)),
        "deps" => this.deps(py).and_then(|value| value.into_bound_py_any(py)),
        "relative_path" => this
            .relative_path(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "source_file_path" => this
            .source_file_path(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "destination" => this
            .destination(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "fingerprint_destination" => this
            .fingerprint_destination(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_native_function_input<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeFunctionInput>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "name" => this.name(py).and_then(|value| value.into_bound_py_any(py)),
        "arguments" => this
            .arguments(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "returns" => this
            .returns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "body_sql" => this
            .body_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "return_columns" => this
            .return_columns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "references" => this
            .references(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "language" => this
            .language(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "runtime_version" => this
            .runtime_version(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "entry_point" => this
            .entry_point(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "packages" => this
            .packages(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "tags" => this.tags(py).and_then(|value| value.into_bound_py_any(py)),
        "description" => this
            .description(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "function_file" => this
            .function_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "database" => this
            .database(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "schema" => this
            .schema(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "logical_database" => this
            .logical_database(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "logical_schema" => this
            .logical_schema(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "fingerprint_database" => this
            .fingerprint_database(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "fingerprint_schema" => this
            .fingerprint_schema(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "fingerprint_logical_database" => this
            .fingerprint_logical_database(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "fingerprint_logical_schema" => this
            .fingerprint_logical_schema(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "declaration_usages" => this
            .declaration_usages(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "assembly_row" => this
            .assembly_row(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_native_inputs_view<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeInputsView>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "run_id" => this.run_id().into_bound_py_any(py),
        "effective_target_name" => this.effective_target_name().into_bound_py_any(py),
        "no_sql_validation" => this.is_sql_validation_skipped().into_bound_py_any(py),
        "model_inputs" => this.model_inputs(py).into_bound_py_any(py),
        "native_project" => this.native_project(py).into_bound_py_any(py),
        "project_config" => this.project_config(py).into_bound_py_any(py),
        "local_config" => this.local_config(py).into_bound_py_any(py),
        "discovered_inputs" => this.discovered_inputs(py).into_bound_py_any(py),
        "sql_lexical_syntax" => this.sql_lexical_syntax(py).into_bound_py_any(py),
        "effective_target" => this.effective_target(py).into_bound_py_any(py),
        "compile_cache_dir" => this.compile_cache_dir(py).into_bound_py_any(py),
        "effective_connection" => this.effective_connection(py).into_bound_py_any(py),
        "effective_vars" => this.effective_vars(py).into_bound_py_any(py),
        "macro_context" => this.macro_context(py).into_bound_py_any(py),
        "loaded_macros" => this.loaded_macros(py).into_bound_py_any(py),
        "public_enums" => this.public_enums(py).into_bound_py_any(py),
        "public_constants" => this.public_constants(py).into_bound_py_any(py),
        "external_sql_reference_resolver" => this
            .external_sql_reference_resolver(py)
            .into_bound_py_any(py),
        "effective_settings" => this.effective_settings(py).into_bound_py_any(py),
        "seed_inputs" => this.seed_inputs(py).into_bound_py_any(py),
        "source_inputs" => this.source_inputs(py).into_bound_py_any(py),
        "sql_function_inputs" => this.sql_function_inputs(py).into_bound_py_any(py),
        "test_inputs" => this.test_inputs(py).into_bound_py_any(py),
        "scenario_inputs" => this.scenario_inputs(py).into_bound_py_any(py),
        "audit_inputs" => this.audit_inputs(py).into_bound_py_any(py),
        "diagnostics" => this.diagnostics(py).into_bound_py_any(py),
        "scope_index" => this.scope_index(py).into_bound_py_any(py),
        "declaration_scope" => this.declaration_scope(py).into_bound_py_any(py),
        _ => return None,
    })
}

fn read_native_model_input<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeModelInput>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "model_file" => this
            .model_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "config" => this
            .config(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "query_sql" => this
            .query_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "macro_source_sql" => this
            .macro_source_sql(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "references" => this
            .references(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "schema_entry" => this
            .schema_entry(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "reference_rows" => this
            .reference_rows(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "relation_key_row" => this
            .relation_key_row(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "attached_schema_name" => this
            .attached_schema_name(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "schema_file" => this.schema_file(py).into_bound_py_any(py),
        "sql_validation_enabled" => this
            .sql_validation_enabled(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "rejected_sql_analysis_opt_out" => this
            .rejected_sql_analysis_opt_out(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "enum_declarations" => this
            .enum_declarations(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "constant_declarations" => this
            .constant_declarations(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "enum_columns" => this
            .enum_columns(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "macro_deps" => this
            .macro_deps(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "macro_usages" => this
            .macro_usages(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "declaration_usages" => this
            .declaration_usages(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "sql_expansion" => this
            .sql_expansion(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "lint_expansion" => this
            .lint_expansion(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_native_project_view<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeProjectView>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "run_id" => this.run_id().into_bound_py_any(py),
        "effective_target_name" => this.effective_target_name().into_bound_py_any(py),
        "effective_target_database" => this.effective_target_database().into_bound_py_any(py),
        "effective_target_schema" => this.effective_target_schema().into_bound_py_any(py),
        "sql_analysis_dialect" => this.sql_analysis_dialect().into_bound_py_any(py),
        "enforce_explicit_references" => {
            this.is_explicit_reference_enforced().into_bound_py_any(py)
        }
        "models" => this.models(py).into_bound_py_any(py),
        "native_project" => this.native_project(py).into_bound_py_any(py),
        "effective_connection" => this.effective_connection(py).into_bound_py_any(py),
        "effective_vars" => this.effective_vars(py).into_bound_py_any(py),
        "binding_catalog" => this.binding_catalog(py).into_bound_py_any(py),
        "sql_lexical_syntax" => this.sql_lexical_syntax(py).into_bound_py_any(py),
        "compile_cache_dir" => this.compile_cache_dir(py).into_bound_py_any(py),
        "loader_functions" => this.loader_functions(py).into_bound_py_any(py),
        "python_execution_inputs" => this.python_execution_inputs().into_bound_py_any(py),
        "hook_functions" => this.hook_functions(py).into_bound_py_any(py),
        "sql_hook_files" => this.sql_hook_files(py).into_bound_py_any(py),
        "materialization_files" => this.materialization_files(py).into_bound_py_any(py),
        "public_enums" => this.public_enums(py).into_bound_py_any(py),
        "public_constants" => this.public_constants(py).into_bound_py_any(py),
        "loaded_macros" => this.loaded_macros(py).into_bound_py_any(py),
        "external_sql_reference_resolver" => this
            .external_sql_reference_resolver(py)
            .into_bound_py_any(py),
        "native_session" => this.native_session(py).into_bound_py_any(py),
        "settings" => this.settings(py).into_bound_py_any(py),
        "scenario" => this.scenario(py).into_bound_py_any(py),
        "sources" => this.sources(py).into_bound_py_any(py),
        "seeds" => this.seeds(py).into_bound_py_any(py),
        "functions" => this.functions(py).into_bound_py_any(py),
        "audits" => this.audits(py).into_bound_py_any(py),
        "sql_tests" => this.sql_tests(py).into_bound_py_any(py),
        "sql_scenarios" => this.sql_scenarios(py).into_bound_py_any(py),
        "diagnostics" => this.diagnostics(py).into_bound_py_any(py),
        "unmatched_literal_sql_relations" => this
            .unmatched_literal_sql_relations(py)
            .into_bound_py_any(py),
        "scope_index" => this.scope_index(py).into_bound_py_any(py),
        "sql_expansions" => this
            .sql_expansions(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "lineage_graph" => this
            .lineage_graph(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_native_compiled_seed<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeCompiledSeed>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "key" => this.key(py).and_then(|value| value.into_bound_py_any(py)),
        "deps" => this.deps(py).into_bound_py_any(py),
        "name" => this.name(py).and_then(|value| value.into_bound_py_any(py)),
        "seed_file" => this
            .seed_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "schema_entry" => this
            .schema_entry(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "schema_file" => this
            .schema_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "destination" => this
            .destination(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "external" => this.is_external().into_bound_py_any(py),
        _ => return None,
    })
}

fn read_native_seed_input<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeSeedInput>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "seed_file" => this
            .seed_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "schema_entry" => this
            .schema_entry(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "schema_file" => this
            .schema_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_native_compiled_source<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeCompiledSource>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "key" => this.key(py).and_then(|value| value.into_bound_py_any(py)),
        "deps" => this.deps(py).into_bound_py_any(py),
        "name" => this.name(py).and_then(|value| value.into_bound_py_any(py)),
        "source_entry" => this
            .source_entry(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "source_file" => this
            .source_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}

fn read_native_source_input<'py>(
    value: &Bound<'py, PyAny>,
    name: &str,
) -> Option<PyResult<Bound<'py, PyAny>>> {
    let py: Python<'py> = value.py();
    let Ok(cell) = value.cast::<NativeSourceInput>() else {
        return None;
    };
    let Ok(this) = cell.try_borrow() else {
        return None;
    };
    Some(match name {
        "source_entry" => this
            .source_entry(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "source_file" => this
            .source_file(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "declaration_usages" => this
            .declaration_usages(py)
            .and_then(|value| value.into_bound_py_any(py)),
        "assembly_row" => this
            .assembly_row(py)
            .and_then(|value| value.into_bound_py_any(py)),
        _ => return None,
    })
}
