use crate::functions::main::frame_argument_error::frame_argument_error;
use crate::functions::tests::test_types::FrameArgumentTestCase;

const DECIMAL_ERROR: &str = "Python function sql/functions/python/smooth.py argument 'amount' is \
     NUMBER(10, 2), which Snowflake passes to a polars frame as Python Decimal objects, a slow \
     object column; declare it as FLOAT, or as INTEGER when it has no fraction";

#[test]
fn given_table_function_arguments_when_checking_the_frame_then_fractional_numbers_fail_for_polars()
{
    let test_cases = [
        FrameArgumentTestCase {
            description: "a fractional NUMBER fails for polars",
            dataframe: Some("polars"),
            argument_type: "NUMBER(10, 2)",
            expected_error: Some(DECIMAL_ERROR),
        },
        FrameArgumentTestCase {
            description: "DECIMAL and NUMERIC are the same type in any case",
            dataframe: Some("polars"),
            argument_type: "decimal(12,4)",
            expected_error: Some(
                "Python function sql/functions/python/smooth.py argument 'amount' is \
                 decimal(12,4), which Snowflake passes to a polars frame as Python Decimal \
                 objects, a slow object column; declare it as FLOAT, or as INTEGER when it has \
                 no fraction",
            ),
        },
        FrameArgumentTestCase {
            description: "DEC is a synonym of NUMBER",
            dataframe: Some("polars"),
            argument_type: "DEC(10, 2)",
            expected_error: Some(
                "Python function sql/functions/python/smooth.py argument 'amount' is \
                 DEC(10, 2), which Snowflake passes to a polars frame as Python Decimal \
                 objects, a slow object column; declare it as FLOAT, or as INTEGER when it has \
                 no fraction",
            ),
        },
        FrameArgumentTestCase {
            description: "a scale of zero is an integer",
            dataframe: Some("polars"),
            argument_type: "NUMBER(38, 0)",
            expected_error: None,
        },
        FrameArgumentTestCase {
            description: "NUMBER without a scale defaults to zero",
            dataframe: Some("polars"),
            argument_type: "NUMBER(38)",
            expected_error: None,
        },
        FrameArgumentTestCase {
            description: "a pandas frame keeps its Decimal objects",
            dataframe: Some("pandas"),
            argument_type: "NUMBER(10, 2)",
            expected_error: None,
        },
        FrameArgumentTestCase {
            description: "a scalar UDF has no frame",
            dataframe: None,
            argument_type: "NUMBER(10, 2)",
            expected_error: None,
        },
    ];

    for test_case in test_cases {
        let arguments: Vec<(String, String)> = vec![
            ("ts".to_owned(), "FLOAT".to_owned()),
            ("amount".to_owned(), test_case.argument_type.to_owned()),
        ];
        assert_eq!(
            frame_argument_error(
                "Python function sql/functions/python/smooth.py",
                test_case.dataframe,
                &arguments
            )
            .as_deref(),
            test_case.expected_error,
            "{}",
            test_case.description
        );
    }
}
