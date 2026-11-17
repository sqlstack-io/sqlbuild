//! Whether a declared column type and a warehouse-listed one name the type the engine stores.

use crate::analysis_route::constants::{SNOWFLAKE_DEFAULT_PRECISION, SNOWFLAKE_DIALECT};
use crate::analysis_route::main::analysis_route::analysis_route;
use crate::analysis_route::models::AnalysisRoute;
use crate::type_system::main::same_type::same_type;
use crate::type_system::models::{NormalizedType, PythonInteger, TypeFamily};

/// Whether `declared` and `listed` are one stored type in `dialect`, its defaults and synonyms met.
#[must_use]
pub fn same_column_type(dialect: Option<&str>, declared: &str, listed: &str) -> bool {
    if declared.trim().eq_ignore_ascii_case(listed.trim()) {
        return true;
    }
    let AnalysisRoute::Native(native) = analysis_route(dialect) else {
        return false;
    };
    let (Some(declared), Some(listed)) = (
        native.normalize_type(declared),
        native.normalize_type(listed),
    ) else {
        return false;
    };
    let (declared, listed): (NormalizedType, NormalizedType) = match native.name() {
        SNOWFLAKE_DIALECT => (snowflake_defaults(declared), snowflake_defaults(listed)),
        _ => (declared, listed),
    };
    native.same_type(&declared, &listed) || same_type(&declared, &listed, native.name())
}

/// A Snowflake fixed-point type written without precision as the `NUMBER(38,0)` it stores.
fn snowflake_defaults(normalized: NormalizedType) -> NormalizedType {
    if normalized.family != TypeFamily::Decimal || normalized.precision.is_some() {
        return normalized;
    }
    let precision: PythonInteger = PythonInteger::from(SNOWFLAKE_DEFAULT_PRECISION);
    let scale: PythonInteger = normalized
        .scale
        .clone()
        .unwrap_or_else(|| PythonInteger::from(0));
    NormalizedType {
        normalized_name: format!("DECIMAL({precision},{scale})"),
        family: TypeFamily::Decimal,
        precision: Some(precision),
        scale: Some(scale),
        length: None,
    }
}
