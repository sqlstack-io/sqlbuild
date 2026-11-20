//! The static facts of one Python function file's `@udf(...)` function.

/// One keyword of the `@udf(...)` call, in source order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UdfKeyword {
    /// The keyword name, or `None` for a `**mapping` expansion.
    pub name: Option<String>,
    /// The 1-based line the keyword starts on.
    pub line: usize,
}

/// Which decorator marks a warehouse Python function.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FunctionDecorator {
    /// `@udf(...)`: a scalar function.
    Udf,
    /// `@table_fn(...)`: a table function that receives a whole partition.
    TableFn,
}

/// The dataframe library a function's first parameter is annotated with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataFrameLibrary {
    Pandas,
    Polars,
}

/// The single `@udf(...)` or `@table_fn(...)` function of a module and what its warehouse body drops.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UdfFunction {
    pub name: String,
    pub decorator: FunctionDecorator,
    /// The library of a `pandas.DataFrame` or `polars.DataFrame` first-parameter annotation.
    pub dataframe: Option<DataFrameLibrary>,
    pub keywords: Vec<UdfKeyword>,
    /// Whether the decorator call passes positional arguments.
    pub has_positional_arguments: bool,
    /// The cleaned docstring, as `ast.get_docstring` returns it.
    pub docstring: Option<String>,
    /// The module statement index of the function.
    pub statement_index: usize,
    /// The decorator indices of the function the warehouse body drops.
    pub removed_decorators: Vec<usize>,
    /// The module statement indices of the `udf` imports the warehouse body drops.
    pub removed_statements: Vec<usize>,
}

/// How many top-level functions a module decorates with `@udf(...)` or `@table_fn(...)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UdfShape {
    Missing,
    Multiple,
    Single(UdfFunction),
}
