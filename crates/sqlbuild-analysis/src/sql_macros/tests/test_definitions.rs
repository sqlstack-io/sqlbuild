use crate::sql_macros::main::parse_sql_macro::parse_sql_macro;
use crate::sql_macros::main::sql_macro_cycle::sql_macro_cycle;
use crate::sql_macros::tests::helpers::owned;
use crate::sql_macros::tests::test_types::{
    CycleTestCase, DefinitionErrorTestCase, DefinitionTestCase, MacroDescriptionTestCase,
    MacroQuoteErrorTestCase,
};

const QUOTE_MESSAGE: &str = "a double quote ends this \"...\" string early; use \"\"\"…\"\"\" for text containing double quotes";

#[test]
fn given_valid_macro_files_when_parsing_then_each_parameter_and_the_body_are_read() {
    let test_cases = [
        DefinitionTestCase {
            description: "every parameter type",
            source: "MACRO (\n  description \"Every type\",\n  arguments (\n    amount BIGINT,\n    places INT CONST DEFAULT 2,\n    codes LIST VARCHAR,\n    unique_codes SET VARCHAR,\n    status ENUM order_status,\n    src RELATION (id BIGINT, created_at TIMESTAMP, keys COLUMNS),\n    price DECIMAL(18, 2),\n  ),\n);\n\nWITH picked AS (SELECT @src.keys FROM @src)\n",
            expected_labels: &[
                "BIGINT",
                "INT CONST",
                "LIST VARCHAR",
                "SET VARCHAR",
                "ENUM order_status",
                "RELATION (id BIGINT, created_at TIMESTAMP, keys COLUMNS)",
                "DECIMAL(18, 2)",
            ],
            expected_form: "CTE-list",
            expected_body_position: (14, 1),
        },
        DefinitionTestCase {
            description: "COLUMNS slots in every list position, single slots read directly and through aliases",
            source: "MACRO (\n  description \"Weekly rates\",\n  arguments (weekly RELATION (keys COLUMNS, carried COLUMNS, order_count BIGINT)),\n);\n\nWITH counted AS (\n  SELECT s.@weekly.keys, s.@weekly.carried, s.@weekly.order_count AS n\n  FROM @weekly AS s\n),\nranked AS (\n  SELECT @weekly.keys, n, ROW_NUMBER() OVER (PARTITION BY @weekly.keys ORDER BY @weekly.keys) AS r\n  FROM counted\n),\nsummed AS (\n  SELECT DISTINCT @weekly.keys, SUM(n) AS total FROM ranked GROUP BY @weekly.keys, n ORDER BY n, @weekly.keys\n),\njoined AS (SELECT * FROM summed JOIN counted USING (@weekly.keys))\n",
            expected_labels: &["RELATION (order_count BIGINT, keys COLUMNS, carried COLUMNS)"],
            expected_form: "CTE-list",
            expected_body_position: (6, 1),
        },
        DefinitionTestCase {
            description: "list parameter in PIVOT and runtime parameter in a WHERE list",
            source: "MACRO (description \"x\", arguments (src RELATION (status VARCHAR, amount BIGINT), statuses LIST VARCHAR, kept VARCHAR));\nWITH pivoted AS (SELECT * FROM @src PIVOT (SUM(amount) FOR status IN @statuses) WHERE status IN (@kept, 'x'))",
            expected_labels: &[
                "RELATION (status VARCHAR, amount BIGINT)",
                "LIST VARCHAR",
                "VARCHAR",
            ],
            expected_form: "CTE-list",
            expected_body_position: (2, 1),
        },
        DefinitionTestCase {
            description: "ANY parameter, with and without a default",
            source: "MACRO (description \"x\", arguments (store VARCHAR, number ANY, separator ANY DEFAULT '::'), returns VARCHAR);\n@store || @separator || CAST(@number AS VARCHAR)",
            expected_labels: &["VARCHAR", "ANY", "ANY"],
            expected_form: "expression",
            expected_body_position: (2, 1),
        },
        DefinitionTestCase {
            description: "CONST parameter as LIMIT",
            source: "MACRO (description \"x\", arguments (src RELATION (id BIGINT), n BIGINT CONST));\nWITH firsts AS (SELECT id FROM @src LIMIT @n)",
            expected_labels: &["RELATION (id BIGINT)", "BIGINT CONST"],
            expected_form: "CTE-list",
            expected_body_position: (2, 1),
        },
        DefinitionTestCase {
            description: "CONST parameter as INTERVAL unit",
            source: "MACRO (description \"x\", arguments (at TIMESTAMP, unit VARCHAR CONST), returns TIMESTAMP);\n@at + INTERVAL 1 @unit",
            expected_labels: &["TIMESTAMP", "VARCHAR CONST"],
            expected_form: "expression",
            expected_body_position: (2, 1),
        },
        DefinitionTestCase {
            description: "CONST parameter as ROUND precision",
            source: "MACRO (description \"x\", arguments (n BIGINT, places INT CONST), returns DECIMAL(18, 2));\nROUND(@n * 1.5, @places) + @n",
            expected_labels: &["BIGINT", "INT CONST"],
            expected_form: "expression",
            expected_body_position: (2, 1),
        },
    ];

    for test_case in test_cases {
        let definition = parse_sql_macro("m", "sql/macros/sql/m.sql", test_case.source)
            .expect(test_case.description);

        let labels: Vec<String> = definition
            .parameters
            .iter()
            .map(|parameter| parameter.kind.label())
            .collect();
        assert_eq!(
            labels, test_case.expected_labels,
            "{}",
            test_case.description
        );
        assert_eq!(
            definition.form.label(),
            test_case.expected_form,
            "{}",
            test_case.description
        );
        assert_eq!(
            (definition.body_line, definition.body_column),
            test_case.expected_body_position,
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_invalid_macro_files_when_parsing_then_each_is_rejected() {
    let test_cases = [
        DefinitionErrorTestCase {
            description: "untyped argument",
            source: "MACRO (description \"x\", arguments (amount), returns BIGINT);\namount",
            expected_code: "P015",
            expected_message: "argument amount of macro m() has no type",
        },
        DefinitionErrorTestCase {
            description: "raw text argument",
            source: "MACRO (description \"x\", arguments (amount SQL), returns BIGINT);\namount",
            expected_code: "P015",
            expected_message: "argument amount of macro m() is raw text",
        },
        DefinitionErrorTestCase {
            description: "reference in a body",
            source: "MACRO (description \"x\");\nWITH c AS (SELECT * FROM __ref(\"stg_orders\"))",
            expected_code: "P016",
            expected_message: "macro m() reads model:stg_orders through __ref() in its body",
        },
        DefinitionErrorTestCase {
            description: "Python macro in a body",
            source: "MACRO (description \"x\", returns BIGINT);\n@py.other()",
            expected_code: "P020",
            expected_message: "SQL macro m() calls Python macro other()",
        },
        DefinitionErrorTestCase {
            description: "bare macro call in a body",
            source: "MACRO (description \"x\", returns BIGINT);\n@other()",
            expected_code: "P014",
            expected_message: "macro call @other() in SQL macro m() does not say which kind of macro it calls",
        },
        DefinitionErrorTestCase {
            description: "CTE list with returns",
            source: "MACRO (description \"x\", returns BIGINT);\nWITH a AS (SELECT 1)",
            expected_code: "P015",
            expected_message: "CTE-list macro m() cannot declare returns",
        },
        DefinitionErrorTestCase {
            description: "relation default",
            source: "MACRO (description \"x\", arguments (src RELATION (id BIGINT) DEFAULT orders));\nWITH a AS (SELECT * FROM src)",
            expected_code: "P015",
            expected_message: "argument src is a RELATION and cannot have a DEFAULT",
        },
        DefinitionErrorTestCase {
            description: "a slot declared twice",
            source: "MACRO (description \"x\", arguments (src RELATION (keys COLUMNS, KEYS BIGINT)));\nWITH a AS (SELECT * FROM @src)",
            expected_code: "P015",
            expected_message: "RELATION argument src declares slot KEYS twice",
        },
        DefinitionErrorTestCase {
            description: "a type constraint on a COLUMNS slot",
            source: "MACRO (description \"x\", arguments (src RELATION (keys COLUMNS OF VARCHAR)));\nWITH a AS (SELECT * FROM @src)",
            expected_code: "P015",
            expected_message: "slot src.keys COLUMNS takes nothing after it; a COLUMNS slot is untyped and each passed column keeps its own type",
        },
        DefinitionErrorTestCase {
            description: "a COLUMNS slot in a WHERE clause",
            source: "MACRO (description \"x\", arguments (src RELATION (keys COLUMNS)));\nWITH a AS (SELECT * FROM @src WHERE @src.keys IS NOT NULL)",
            expected_code: "P015",
            expected_message: "COLUMNS slot src.keys is read as @src.keys in macro m() where a list item cannot stand; a COLUMNS slot is pasted as its columns, comma-separated, so it stands only as a whole item of a select list, PARTITION BY, GROUP BY, ORDER BY or USING (...)",
        },
        DefinitionErrorTestCase {
            description: "a COLUMNS slot inside a function call",
            source: "MACRO (description \"x\", arguments (src RELATION (keys COLUMNS)));\nWITH a AS (SELECT COUNT(DISTINCT @src.keys) AS n FROM @src)",
            expected_code: "P015",
            expected_message: "COLUMNS slot src.keys is read as @src.keys in macro m() where a list item cannot stand",
        },
        DefinitionErrorTestCase {
            description: "a COLUMNS slot renamed with AS",
            source: "MACRO (description \"x\", arguments (src RELATION (keys COLUMNS)));\nWITH a AS (SELECT s.@src.keys AS k FROM @src AS s)",
            expected_code: "P015",
            expected_message: "COLUMNS slot src.keys is read as s.@src.keys in macro m() where a list item cannot stand",
        },
        DefinitionErrorTestCase {
            description: "a COLUMNS slot ordered descending",
            source: "MACRO (description \"x\", arguments (src RELATION (keys COLUMNS)));\nWITH a AS (SELECT * FROM @src ORDER BY @src.keys DESC)",
            expected_code: "P015",
            expected_message: "COLUMNS slot src.keys is read as @src.keys in macro m() where a list item cannot stand",
        },
        DefinitionErrorTestCase {
            description: "a COLUMNS slot in an expression",
            source: "MACRO (description \"x\", arguments (src RELATION (keys COLUMNS)));\nWITH a AS (SELECT @src.keys || 'x' AS k FROM @src)",
            expected_code: "P015",
            expected_message: "COLUMNS slot src.keys is read as @src.keys in macro m() where a list item cannot stand",
        },
        DefinitionErrorTestCase {
            description: "a COLUMNS slot as an expression macro body",
            source: "MACRO (description \"x\", arguments (src RELATION (keys COLUMNS)));\n@src.keys",
            expected_code: "P015",
            expected_message: "COLUMNS slot src.keys is read as @src.keys in macro m() where a list item cannot stand",
        },
        DefinitionErrorTestCase {
            description: "a COLUMNS slot in a CASE branch",
            source: "MACRO (description \"x\", arguments (src RELATION (keys COLUMNS)));\nSELECT CASE WHEN TRUE THEN @src.keys END AS k",
            expected_code: "P015",
            expected_message: "COLUMNS slot src.keys is read as @src.keys in macro m() where a list item cannot stand",
        },
        DefinitionErrorTestCase {
            description: "an aliased read naming no slot",
            source: "MACRO (description \"x\", arguments (src RELATION (order_count BIGINT)));\nWITH a AS (SELECT s.@src.refund_count AS r FROM @src AS s)",
            expected_code: "P022",
            expected_message: "s.@src.refund_count in the body of macro m() names no slot of RELATION argument src",
        },
        DefinitionErrorTestCase {
            description: "an aliased read naming no slot at all",
            source: "MACRO (description \"x\", arguments (src RELATION (order_count BIGINT)));\nWITH a AS (SELECT s.@src AS r FROM @src AS s)",
            expected_code: "P022",
            expected_message: "s.@src in the body of macro m() reads RELATION argument src through an alias without naming a slot",
        },
        DefinitionErrorTestCase {
            description: "constant default that is not a literal",
            source: "MACRO (description \"x\", arguments (a INT CONST DEFAULT b + 1), returns BIGINT);\na",
            expected_code: "P015",
            expected_message: "DEFAULT of argument a of macro m() expects INT CONST",
        },
        DefinitionErrorTestCase {
            description: "undeclared parameter reference",
            source: "MACRO (description \"x\", arguments (a BIGINT), returns BIGINT);\n@a + @b",
            expected_code: "P022",
            expected_message: "@b in the body of macro m() names no parameter of the macro",
        },
        DefinitionErrorTestCase {
            description: "relation parameter written as a bare table name",
            source: "MACRO (description \"x\", arguments (src RELATION (id BIGINT)));\nWITH a AS (SELECT id FROM src)",
            expected_code: "P022",
            expected_message: "src is a parameter of macro m(); write @src",
        },
        DefinitionErrorTestCase {
            description: "relation parameter written as a bare qualifier",
            source: "MACRO (description \"x\", arguments (src RELATION (id BIGINT)));\nSELECT src.id",
            expected_code: "P022",
            expected_message: "src is a parameter of macro m(); write @src",
        },
        DefinitionErrorTestCase {
            description: "built-in parameter name",
            source: "MACRO (description \"x\", arguments (const BIGINT), returns BIGINT);\n1",
            expected_code: "P015",
            expected_message: "argument const of macro m() uses a built-in name",
        },
        DefinitionErrorTestCase {
            description: "LIST with CONST",
            source: "MACRO (description \"x\", arguments (codes LIST VARCHAR CONST), returns BOOLEAN);\n'a' IN @codes",
            expected_code: "P015",
            expected_message: "argument codes LIST VARCHAR CONST: a LIST argument is always known at compile time, so it takes no CONST",
        },
        DefinitionErrorTestCase {
            description: "ANY with CONST",
            source: "MACRO (description \"x\", arguments (n ANY CONST), returns BIGINT);\n@n",
            expected_code: "P015",
            expected_message: "argument n ANY CONST: a CONST argument needs a type its values are checked against",
        },
        DefinitionErrorTestCase {
            description: "ANY with more after it",
            source: "MACRO (description \"x\", arguments (n ANY[]), returns BIGINT);\n@n",
            expected_code: "P015",
            expected_message: "argument n ANY takes nothing after it",
        },
        DefinitionErrorTestCase {
            description: "ANY family with CONST",
            source: "MACRO (description \"x\", arguments (n ANY NUMERIC CONST), returns BIGINT);\n@n",
            expected_code: "P015",
            expected_message: "argument n ANY NUMERIC takes nothing after it",
        },
        DefinitionErrorTestCase {
            description: "LIST of an ANY family",
            source: "MACRO (description \"x\", arguments (codes LIST ANY TEXT), returns BOOLEAN);\n'a' IN @codes",
            expected_code: "P015",
            expected_message: "argument codes LIST ANY: ANY is for scalar arguments only",
        },
        DefinitionErrorTestCase {
            description: "returns TYPE OF an unknown argument",
            source: "MACRO (description \"x\", arguments (v ANY), returns TYPE OF w);\n@v",
            expected_code: "P015",
            expected_message: "returns TYPE OF w names no argument of macro m()",
        },
        DefinitionErrorTestCase {
            description: "returns TYPE OF a LIST argument",
            source: "MACRO (description \"x\", arguments (v LIST VARCHAR), returns TYPE OF v);\n'a' IN @v",
            expected_code: "P015",
            expected_message: "returns TYPE OF v names a LIST VARCHAR argument of macro m(); only a scalar argument has a type to return",
        },
        DefinitionErrorTestCase {
            description: "returns TYPE OF on a CTE list",
            source: "MACRO (description \"x\", arguments (src RELATION (id BIGINT)), returns TYPE OF src);\nWITH a AS (SELECT * FROM @src)",
            expected_code: "P015",
            expected_message: "CTE-list macro m() cannot declare returns",
        },
        DefinitionErrorTestCase {
            description: "LIST of ANY",
            source: "MACRO (description \"x\", arguments (codes LIST ANY), returns BOOLEAN);\n'a' IN @codes",
            expected_code: "P015",
            expected_message: "argument codes LIST ANY: ANY is for scalar arguments only",
        },
        DefinitionErrorTestCase {
            description: "ANY parameter as LIMIT",
            source: "MACRO (description \"x\", arguments (src RELATION (id BIGINT), n ANY));\nWITH firsts AS (SELECT id FROM @src LIMIT @n)",
            expected_code: "P023",
            expected_message: "n is used where a literal is required; declare it with a type and CONST, such as n VARCHAR CONST",
        },
        DefinitionErrorTestCase {
            description: "array type with CONST",
            source: "MACRO (description \"x\", arguments (codes VARCHAR[] CONST), returns BOOLEAN);\nlen(@codes) > 0",
            expected_code: "P015",
            expected_message: "argument codes VARCHAR[] CONST: CONST is for scalar types only",
        },
        DefinitionErrorTestCase {
            description: "runtime parameter as LIMIT",
            source: "MACRO (description \"x\", arguments (src RELATION (id BIGINT), n BIGINT));\nWITH firsts AS (SELECT id FROM @src LIMIT @n)",
            expected_code: "P023",
            expected_message: "n is used where a literal is required; declare it n BIGINT CONST",
        },
        DefinitionErrorTestCase {
            description: "runtime parameter as OFFSET",
            source: "MACRO (description \"x\", arguments (src RELATION (id BIGINT), n BIGINT));\nWITH rest AS (SELECT id FROM @src LIMIT 10 OFFSET @n)",
            expected_code: "P023",
            expected_message: "n is used where a literal is required",
        },
        DefinitionErrorTestCase {
            description: "runtime parameter as FETCH FIRST",
            source: "MACRO (description \"x\", arguments (src RELATION (id BIGINT), n BIGINT));\nWITH firsts AS (SELECT id FROM @src FETCH FIRST @n ROWS ONLY)",
            expected_code: "P023",
            expected_message: "n is used where a literal is required",
        },
        DefinitionErrorTestCase {
            description: "runtime parameter as INTERVAL unit",
            source: "MACRO (description \"x\", arguments (at TIMESTAMP, unit VARCHAR), returns TIMESTAMP);\n@at + INTERVAL 1 @unit",
            expected_code: "P023",
            expected_message: "unit is used where a literal is required; declare it unit VARCHAR CONST",
        },
        DefinitionErrorTestCase {
            description: "runtime parameter as EXTRACT unit",
            source: "MACRO (description \"x\", arguments (at TIMESTAMP, part VARCHAR), returns BIGINT);\nEXTRACT(@part FROM @at)",
            expected_code: "P023",
            expected_message: "part is used where a literal is required",
        },
        DefinitionErrorTestCase {
            description: "runtime parameter as PIVOT value",
            source: "MACRO (description \"x\", arguments (src RELATION (status VARCHAR, amount BIGINT), first_status VARCHAR));\nWITH pivoted AS (SELECT * FROM @src PIVOT (SUM(amount) FOR status IN (@first_status, 'closed')))",
            expected_code: "P023",
            expected_message: "first_status is used where a literal is required",
        },
        DefinitionErrorTestCase {
            description: "missing header",
            source: "SELECT 1",
            expected_code: "P015",
            expected_message: "SQL macro file sql/macros/sql/m.sql must start with a MACRO (...) header",
        },
        DefinitionErrorTestCase {
            description: "unknown header key",
            source: "MACRO (description \"x\", owner \"me\", returns BIGINT);\n1",
            expected_code: "P015",
            expected_message: "unknown MACRO header key owner",
        },
    ];

    for test_case in test_cases {
        let error = parse_sql_macro("m", "sql/macros/sql/m.sql", test_case.source)
            .expect_err(test_case.description);

        assert_eq!(
            error.code, test_case.expected_code,
            "{}: {}",
            test_case.description, error.message
        );
        assert!(
            error.message.starts_with(test_case.expected_message),
            "{}: {}",
            test_case.description,
            error.message
        );
    }
}

#[test]
fn given_macro_descriptions_when_parsing_then_the_folded_text_is_stored() {
    let test_cases = [
        MacroDescriptionTestCase {
            description: "a triple-quoted description on one line keeps its quotes",
            source: "MACRO (description \"\"\"Matches \"Miss\", it's.\"\"\", returns VARCHAR);\n'x'\n",
            expected_description: "Matches \"Miss\", it's.",
        },
        MacroDescriptionTestCase {
            description: "a triple-quoted block across lines is folded",
            source: "MACRO (\n  description \"\"\"\n    True for \"Mrs. C. O'Brien\",\n    any case.\n  \"\"\",\n  returns VARCHAR,\n);\n'x'\n",
            expected_description: "True for \"Mrs. C. O'Brien\", any case.",
        },
        MacroDescriptionTestCase {
            description: "a backslash escapes a quote in a single-quoted description",
            source: "MACRO (\n  description 'it\\'s here',\n  arguments (v VARCHAR),\n  returns VARCHAR,\n);\n\nLOWER(@v)\n",
            expected_description: "it's here",
        },
        MacroDescriptionTestCase {
            description: "a doubled quote is one quote in a single-quoted description",
            source: "MACRO (\n  description 'it''s',\n  returns VARCHAR,\n);\n'x'\n",
            expected_description: "it's",
        },
        MacroDescriptionTestCase {
            description: "a double-quoted description may end in a backslash",
            source: "MACRO (\n  description \"ends in C:\\\",\n  returns VARCHAR,\n);\n'x'\n",
            expected_description: "ends in C:\\",
        },
        MacroDescriptionTestCase {
            description: "a wrapped description stores the same as one line",
            source: "MACRO (\n  description \"True for a woman's title,\nany case.\",\n  returns VARCHAR,\n);\n'it''s'\n",
            expected_description: "True for a woman's title, any case.",
        },
    ];

    for test_case in test_cases {
        let definition = parse_sql_macro("m", "sql/macros/sql/m.sql", test_case.source)
            .expect(test_case.description);

        assert_eq!(
            definition.description.as_deref(),
            Some(test_case.expected_description),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_misquoted_macro_descriptions_when_parsing_then_the_quote_is_located() {
    let test_cases = [
        MacroQuoteErrorTestCase {
            description: "an escaped quote on one line fails at the quote",
            source: "MACRO (\n  description \"Matches a title (\\\"Miss\\\").\",\n  arguments (value VARCHAR),\n  returns VARCHAR,\n);\n@value\n",
            expected_message: QUOTE_MESSAGE,
            expected_location: (2, 34),
        },
        MacroQuoteErrorTestCase {
            description: "apostrophes elsewhere no longer let an escaped quote through",
            source: "MACRO (\n  description \"True when a name starts with a woman's title. Any case,\nwith or without a dot (\\\"MISS ANNA PARKER\\\", \\\"Mrs. C. O'Brien\\\").\",\n  arguments (value VARCHAR),\n  returns VARCHAR,\n);\n@value\n",
            expected_message: QUOTE_MESSAGE,
            expected_location: (3, 25),
        },
        MacroQuoteErrorTestCase {
            description: "a bare quote that ends the string early fails where it is",
            source: "MACRO (description \"Matches \"Miss\".\", returns VARCHAR);\n'x'\n",
            expected_message: QUOTE_MESSAGE,
            expected_location: (1, 29),
        },
        MacroQuoteErrorTestCase {
            description: "doubled double quotes fail at the second quote",
            source: "MACRO (description \"say \"\"hi\"\" now\", returns VARCHAR);\n'x'\n",
            expected_message: QUOTE_MESSAGE,
            expected_location: (1, 25),
        },
        MacroQuoteErrorTestCase {
            description: "a lone quote inside a single-quoted description",
            source: "MACRO (description 'it's here', returns VARCHAR);\n'x'\n",
            expected_message: "a single quote ends this '...' string early",
            expected_location: (1, 23),
        },
        MacroQuoteErrorTestCase {
            description: "an unclosed triple quote reports where it started",
            source: "MACRO (\n  description \"\"\"never closed,\n  returns VARCHAR,\n);\n'x'\n",
            expected_message: "this \"\"\" string is never closed",
            expected_location: (2, 15),
        },
        MacroQuoteErrorTestCase {
            description: "a triple quote inside a triple-quoted string",
            source: "MACRO (description \"\"\"a \"\"\"b\"\"\" c\"\"\", returns VARCHAR);\n'x'\n",
            expected_message: "\"\"\" inside a \"\"\"...\"\"\" string ends it early",
            expected_location: (1, 25),
        },
    ];

    for test_case in test_cases {
        let error = parse_sql_macro("m", "sql/macros/sql/m.sql", test_case.source)
            .expect_err(test_case.description);
        let location = error
            .locations
            .first()
            .map(|location| (location.line, location.column));

        assert_eq!(error.code, "P015", "{}", test_case.description);
        assert!(
            error.message.starts_with(test_case.expected_message),
            "{}: {}",
            test_case.description,
            error.message
        );
        assert_eq!(
            location,
            Some(test_case.expected_location),
            "{}",
            test_case.description
        );
    }
}

#[test]
fn given_macros_calling_each_other_when_checking_then_the_cycle_is_named() {
    let test_cases = [
        CycleTestCase {
            description: "two macros calling each other",
            calls: &[
                ("net_amount", &["cents_to_dollars"]),
                ("cents_to_dollars", &["net_amount"]),
                ("other", &[]),
            ],
            expected_cycle: Some(&["net_amount", "cents_to_dollars", "net_amount"]),
        },
        CycleTestCase {
            description: "a chain without a cycle",
            calls: &[
                ("net_amount", &["cents_to_dollars"]),
                ("cents_to_dollars", &[]),
            ],
            expected_cycle: None,
        },
    ];

    for test_case in test_cases {
        let calls: Vec<(String, Vec<String>)> = test_case
            .calls
            .iter()
            .map(|(name, called)| ((*name).to_owned(), owned(called)))
            .collect();

        let cycle: Option<Vec<String>> = sql_macro_cycle(&calls);

        assert_eq!(
            cycle,
            test_case.expected_cycle.map(owned),
            "{}",
            test_case.description
        );
    }
}
