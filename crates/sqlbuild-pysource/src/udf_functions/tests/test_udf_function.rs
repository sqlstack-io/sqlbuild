use crate::udf_functions::main::udf_function::udf_function;
use crate::udf_functions::models::{DataFrameLibrary, FunctionDecorator, UdfFunction, UdfShape};
use crate::udf_functions::tests::helpers::keyword;
use crate::udf_functions::tests::test_types::UdfFunctionTestCase;

/// Expectations recorded from the CPython `ast` implementation this replaces.
#[test]
fn given_function_file_when_reading_udf_then_matches_cpython_facts() {
    let test_cases = [
        UdfFunctionTestCase {
            description: "keywords, cleaned docstring, dropped import and decorator",
            contents: "from sqlbuild import udf\nimport os\n\n@other\n@udf(returns=\"INTEGER\",\n     arguments=[\n        \"x INTEGER\",\n     ])\ndef add_one(x):\n    \"\"\"Add one.\n\n        Indented detail.\n    Trailing.\n    \"\"\"\n    return x + 1\n",
            expected_shape: Some(UdfShape::Single(UdfFunction {
                name: "add_one".to_owned(),
                decorator: FunctionDecorator::Udf,
                dataframe: None,
                keywords: vec![keyword(Some("returns"), 5), keyword(Some("arguments"), 6)],
                has_positional_arguments: false,
                docstring: Some("Add one.\n\n    Indented detail.\nTrailing.".to_owned()),
                statement_index: 2,
                removed_decorators: vec![1],
                removed_statements: vec![0],
            })),
        },
        UdfFunctionTestCase {
            description: "an attribute decorator on an async function with tab-indented docstring",
            contents: "import sqlbuild\n@sqlbuild.udf(**extra)\nasync def f():\n\t\"\"\"\n\tTabbed.\n\t  More.\n\t\"\"\"\n",
            expected_shape: Some(UdfShape::Single(UdfFunction {
                name: "f".to_owned(),
                decorator: FunctionDecorator::Udf,
                dataframe: None,
                keywords: vec![keyword(None, 2)],
                has_positional_arguments: false,
                docstring: Some("Tabbed.\n  More.".to_owned()),
                statement_index: 1,
                removed_decorators: vec![0],
                removed_statements: Vec::new(),
            })),
        },
        UdfFunctionTestCase {
            description: "a bare decorator is not a udf call",
            contents: "@udf\ndef f():\n    pass\n",
            expected_shape: Some(UdfShape::Missing),
        },
        UdfFunctionTestCase {
            description: "two decorated functions",
            contents: "@udf()\ndef f():\n    pass\n\n@udf()\ndef g():\n    pass\n",
            expected_shape: Some(UdfShape::Multiple),
        },
        UdfFunctionTestCase {
            description: "a table function with a polars alias annotation drops both imports",
            contents: "import polars as pl\nfrom sqlbuild.functions import table_fn\n\n@table_fn(returns={\"a\": \"FLOAT\"})\ndef main(rows: pl.DataFrame) -> pl.DataFrame:\n    return rows\n",
            expected_shape: Some(UdfShape::Single(UdfFunction {
                name: "main".to_owned(),
                decorator: FunctionDecorator::TableFn,
                dataframe: Some(DataFrameLibrary::Polars),
                keywords: vec![keyword(Some("returns"), 4)],
                has_positional_arguments: false,
                docstring: None,
                statement_index: 2,
                removed_decorators: vec![0],
                removed_statements: vec![1],
            })),
        },
        UdfFunctionTestCase {
            description: "a pandas DataFrame imported by name, with polars also imported",
            contents: "import polars\nfrom pandas import DataFrame as Frame\nfrom sqlbuild import table_fn\n\n@table_fn()\ndef main(rows: Frame) -> polars.DataFrame:\n    return rows\n",
            expected_shape: Some(UdfShape::Single(UdfFunction {
                name: "main".to_owned(),
                decorator: FunctionDecorator::TableFn,
                dataframe: Some(DataFrameLibrary::Pandas),
                keywords: Vec::new(),
                has_positional_arguments: false,
                docstring: None,
                statement_index: 3,
                removed_decorators: vec![0],
                removed_statements: vec![2],
            })),
        },
        UdfFunctionTestCase {
            description: "an unannotated or quoted parameter names no library",
            contents: "import pandas as pd\nfrom sqlbuild.functions import table_fn\n\n@table_fn()\ndef main(rows: \"pd.DataFrame\"):\n    return rows\n",
            expected_shape: Some(UdfShape::Single(UdfFunction {
                name: "main".to_owned(),
                decorator: FunctionDecorator::TableFn,
                dataframe: None,
                keywords: Vec::new(),
                has_positional_arguments: false,
                docstring: None,
                statement_index: 2,
                removed_decorators: vec![0],
                removed_statements: vec![1],
            })),
        },
        UdfFunctionTestCase {
            description: "a udf and a table function in one module",
            contents: "@udf()\ndef f():\n    pass\n\n@table_fn()\ndef g(rows):\n    pass\n",
            expected_shape: Some(UdfShape::Multiple),
        },
        UdfFunctionTestCase {
            description: "invalid syntax",
            contents: "def f(:\n",
            expected_shape: None,
        },
    ];
    for test_case in test_cases {
        assert_eq!(
            udf_function(test_case.contents, (3, 12)),
            test_case.expected_shape,
            "{}",
            test_case.description
        );
    }
}
