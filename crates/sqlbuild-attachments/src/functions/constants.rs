//! Reference kinds a function body's references are validated by.

pub(crate) const REF_KIND: &str = "ref";
pub(crate) const SEED_KIND: &str = "seed";
pub(crate) const SOURCE_KIND: &str = "source";
pub(crate) const DBT_REF_KIND: &str = "dbt_ref";
pub(crate) const UDF_KIND: &str = "udf";
pub(crate) const TABLE_FUNCTION_KIND: &str = "table_fn";
/// The frame library whose columns hold Snowflake's fixed-point values as Python objects.
pub(crate) const POLARS_DATAFRAME: &str = "polars";
/// The fixed-point type names Snowflake passes to Python as `Decimal`.
pub(crate) const FIXED_POINT_TYPES: [&str; 4] = ["NUMBER", "DECIMAL", "DEC", "NUMERIC"];
