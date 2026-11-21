//! The decorator and imports that mark a warehouse Python function file.

/// The decorator name a UDF function is marked with, bare or as an attribute.
pub const UDF_DECORATOR_NAME: &str = "udf";
/// The decorator name a Python table function is marked with, bare or as an attribute.
pub const TABLE_FN_DECORATOR_NAME: &str = "table_fn";
/// The class name a dataframe annotation names, as `<module>.DataFrame` or an imported `DataFrame`.
pub(crate) const DATAFRAME_CLASS: &str = "DataFrame";
/// The modules whose `DataFrame` selects pandas or polars.
pub(crate) const PANDAS_MODULE: &str = "pandas";
pub(crate) const POLARS_MODULE: &str = "polars";
/// The modules whose `udf` and `table_fn` imports the warehouse body drops.
pub const UDF_IMPORT_MODULES: [&str; 2] = ["sqlbuild", "sqlbuild.functions"];
/// The keywords `@udf(...)` accepts.
pub const UDF_KEYS: [&str; 9] = [
    "description",
    "arguments",
    "returns",
    "runtime_version",
    "entry_point",
    "packages",
    "database",
    "schema",
    "tags",
];
