"""E2E coverage for SQL macro bodies under Rules and SQL macro expansions passed to Python macros."""

from __future__ import annotations

from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroCompileTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    CHECK_MACRO_PATH,
    WRAPPING_MACROS,
    check_macro,
    inspected_text,
    orders_model,
    output,
    run_project,
    with_rules,
)

_COMPILED: str = "target/compiled/models/marts/orders.sql"
_RULE_FAILURES: tuple[str, ...] = ("L005", "rules-unevaluated", "Native SQL parser")
_CHECK_STATUS: dict[str, str] = orders_model(
    'SELECT order_id, @sql.check(status) AS checked FROM __ref("stg_orders")\n'
)
_COMMENTED_CHECK: dict[str, str] = check_macro(
    "value VARCHAR",
    "-- a status holding an at sign is flagged\nCONTAINS(@value, /* the sign */ '@')",
)
_COMMAND: tuple[str, ...] = ("compile", "--no-cache")


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="parameter_before_words_and_a_parenthesis",
            files={
                **with_rules("SQLCLEAN001"),
                **check_macro("value VARCHAR", "@value IS NULL OR TRIM(@value) = ''"),
                **_CHECK_STATUS,
            },
            expected_returncode=0,
            expected_fragments=("status IS NULL OR TRIM(status) = '' AS checked",),
            unexpected_fragments=_RULE_FAILURES,
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="parameter_before_in_inside_case",
            files={
                **with_rules("SQLCLEAN001"),
                **check_macro(
                    "value VARCHAR",
                    "CASE WHEN @value IN ('open', 'closed') THEN TRUE ELSE FALSE END",
                ),
                **_CHECK_STATUS,
            },
            expected_returncode=0,
            expected_fragments=(
                "CASE WHEN status IN ('open', 'closed') THEN TRUE ELSE FALSE END AS checked",
            ),
            unexpected_fragments=_RULE_FAILURES,
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="parameter_before_and_a_parenthesised_predicate",
            files={
                **with_rules("SQLCLEAN001"),
                **check_macro("value VARCHAR", "@value IS NULL AND (@value IS NULL)"),
                **_CHECK_STATUS,
            },
            expected_returncode=0,
            expected_fragments=("status IS NULL AND (status IS NULL) AS checked",),
            unexpected_fragments=_RULE_FAILURES,
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="parameter_before_in_alone",
            files={
                **with_rules("SQLCLEAN001"),
                **check_macro("value VARCHAR", "@value IN ('', ' ')"),
                **_CHECK_STATUS,
            },
            expected_returncode=0,
            expected_fragments=("status IN ('', ' ') AS checked",),
            unexpected_fragments=_RULE_FAILURES,
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="parameter_compared_then_trimmed",
            files={
                **with_rules("SQLCLEAN001"),
                **check_macro("value VARCHAR", "@value = '' OR TRIM(@value) = ''"),
                **_CHECK_STATUS,
            },
            expected_returncode=0,
            expected_fragments=("status = '' OR TRIM(status) = '' AS checked",),
            unexpected_fragments=_RULE_FAILURES,
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="parenthesised_parameter",
            files={
                **with_rules("SQLCLEAN001"),
                **check_macro("value VARCHAR", "(@value) IS NULL OR TRIM(@value) = ''"),
                **_CHECK_STATUS,
            },
            expected_returncode=0,
            expected_fragments=("(status) IS NULL OR TRIM(status) = '' AS checked",),
            unexpected_fragments=_RULE_FAILURES,
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="body_opening_with_two_parentheses",
            files={
                **with_rules("SQLCLEAN001"),
                **check_macro(
                    "value VARCHAR", "((@value) IS NULL AND (@value) IS NULL) OR TRIM(@value) = ''"
                ),
                **_CHECK_STATUS,
            },
            expected_returncode=0,
            expected_fragments=(
                "((status) IS NULL AND (status) IS NULL) OR TRIM(status) = '' AS checked",
            ),
            unexpected_fragments=_RULE_FAILURES,
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="spaced_enum_and_constant_references_in_a_body_are_declarations",
            files={
                **with_rules("SQLCLEAN001"),
                "constants/fee_divisor.sql": "CONSTANT (name fee_divisor, value 100);\n",
                **check_macro(
                    "value VARCHAR, cents BIGINT",
                    '@value <> @enum ("order_status").CLOSED AND @cents / @const ("fee_divisor") > 1',
                ),
                **orders_model(
                    "SELECT order_id, @sql.check(status, amount_cents) AS checked "
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("status <> 'closed' AND amount_cents / 100 > 1 AS checked",),
            unexpected_fragments=(*_RULE_FAILURES, "P022"),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="a_trailing_comment_before_a_line_break_compiles_unchanged",
            files={
                **check_macro("value VARCHAR", "CONTAINS(@value, 'x') -- why"),
                **orders_model('SELECT order_id, @sql.check(status)\nFROM __ref("stg_orders")\n'),
            },
            expected_returncode=0,
            expected_fragments=("SELECT order_id, CONTAINS(status, 'x') -- why\nFROM __ref(",),
            unexpected_fragments=("-- why\n\n",),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="two_parameters_before_and_a_parenthesised_predicate",
            files={
                **with_rules("SQLCLEAN001"),
                **check_macro("x VARCHAR, y VARCHAR", "@x IS NULL AND (@y IS NULL)"),
                **orders_model(
                    "SELECT order_id, @sql.check(status, country_code) AS checked "
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("status IS NULL AND (country_code IS NULL) AS checked",),
            unexpected_fragments=_RULE_FAILURES,
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="a_finding_in_a_macro_body_is_reported_in_the_macro_file",
            files={
                **with_rules("SQLCLEAN001"),
                **check_macro("value VARCHAR", "((@value) IS NULL) OR @value = NULL"),
                **_CHECK_STATUS,
            },
            expected_returncode=1,
            expected_fragments=("[SQLCLEAN001]", f"--> {CHECK_MACRO_PATH}:7:"),
            unexpected_fragments=_RULE_FAILURES,
        ),
        SqlMacroCompileTestCase(
            description="a_cte_list_body_lints_without_parse_or_unused_cte_findings",
            files={
                **with_rules("SQLCLEAN001", "SQLCLEAN005"),
                "macros/sql/live_orders.sql": (
                    'MACRO (\n  description "Live orders",\n'
                    "  arguments (src RELATION (order_id BIGINT, is_deleted BOOLEAN)),\n);\n\n"
                    "WITH live AS (SELECT s.order_id FROM @src AS s WHERE NOT s.is_deleted),\n"
                    "live_ids AS (SELECT live.order_id FROM live)\n"
                ),
                **orders_model(
                    'WITH raw_orders AS (SELECT * FROM __ref("stg_orders")),\n'
                    "@sql.live_orders(raw_orders)\nSELECT order_id FROM live_ids\n"
                ),
            },
            expected_returncode=0,
            expected_fragments=("Project compiled",),
            unexpected_fragments=(*_RULE_FAILURES, "SQLCLEAN005"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_sql_macro_body_when_compiling_with_rules_then_it_lints_and_expands(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(tmp_path=tmp_path, files=test_case.files, command=_COMMAND)
    printed: str = output(result) + inspected_text(
        project_dir=project_dir, relative_paths=test_case.inspected_paths
    )

    assert result.returncode == test_case.expected_returncode, printed
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed
    for fragment in test_case.unexpected_fragments:
        assert fragment not in printed, printed


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="double_quote_in_a_string_literal",
            files={
                **WRAPPING_MACROS,
                **check_macro("value VARCHAR", "CONTAINS(@value, '\"')"),
                **orders_model(
                    'SELECT order_id, @py.wrap(@sql.check(status)) AS probe FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("(CONTAINS(status, '\"')) AS probe",),
            unexpected_fragments=("P001",),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="double_quote_compared_directly",
            files={
                **WRAPPING_MACROS,
                **check_macro("value VARCHAR", "@value = '\"'"),
                **orders_model(
                    'SELECT order_id, @py.wrap(@sql.check(status)) AS probe FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("(status = '\"') AS probe",),
            unexpected_fragments=("P001",),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="double_quote_as_a_dict_value",
            files={
                **WRAPPING_MACROS,
                **check_macro("value VARCHAR", "CONTAINS(@value, '\"')"),
                **orders_model(
                    'SELECT order_id, @py.pick({"a": @sql.check(status)}) AS probe FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("CONTAINS(status, '\"') AS probe",),
            unexpected_fragments=("P001",),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="quotes_and_backslashes_in_a_nested_list",
            files={
                **WRAPPING_MACROS,
                **check_macro("value VARCHAR", "@value IN ('it''s', '\\\\', '\"')"),
                **orders_model(
                    'SELECT order_id, @py.first([@sql.check(status), [@sql.check(status)]]) AS probe FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("status IN ('it''s', '\\\\', '\"') AS probe",),
            unexpected_fragments=("P001",),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="line_comment_before_the_expression",
            files={
                **WRAPPING_MACROS,
                **check_macro("value VARCHAR", "-- why\nCONTAINS(@value, 'x')"),
                **orders_model(
                    'SELECT order_id, @py.wrap(@sql.check(status)) AS probe FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("(-- why\nCONTAINS(status, 'x')) AS probe",),
            unexpected_fragments=("P001",),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="block_comment_inside_the_expression",
            files={
                **WRAPPING_MACROS,
                **check_macro("value VARCHAR", "CONTAINS(@value, /* why */ 'x')"),
                **orders_model(
                    'SELECT order_id, @py.wrap(@sql.check(status)) AS probe FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("(CONTAINS(status, /* why */ 'x')) AS probe",),
            unexpected_fragments=("P001",),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="line_comment_ending_the_body",
            files={
                **WRAPPING_MACROS,
                **check_macro("value VARCHAR", "CONTAINS(@value, 'x') -- why"),
                **orders_model(
                    'SELECT order_id, @py.wrap(@sql.check(status)) AS probe FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("(CONTAINS(status, 'x') -- why\n) AS probe",),
            unexpected_fragments=("P001",),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="line_comment_ending_a_body_called_directly",
            files={
                **WRAPPING_MACROS,
                **check_macro("value VARCHAR", "CONTAINS(@value, 'x') -- why"),
                **orders_model(
                    'SELECT order_id, @sql.check(status) AS probe FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=0,
            expected_fragments=("CONTAINS(status, 'x') -- why\n AS probe",),
            unexpected_fragments=("P001",),
            inspected_paths=(_COMPILED,),
        ),
        SqlMacroCompileTestCase(
            description="an_unreadable_python_macro_call_names_its_place",
            files={
                **WRAPPING_MACROS,
                **orders_model(
                    'SELECT order_id,\n  @py.wrap(\'oops) AS probe\nFROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=1,
            expected_fragments=(
                "Macro arguments of '@py.wrap' in '",
                "orders.sql' could not be read: a quoted string in @py.wrap is not closed "
                "at line 4, column 3",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_a_sql_macro_call_in_python_macro_arguments_when_compiling_then_python_gets_its_sql(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    project_dir, result = run_project(tmp_path=tmp_path, files=test_case.files, command=_COMMAND)
    printed: str = output(result) + inspected_text(
        project_dir=project_dir, relative_paths=test_case.inspected_paths
    )

    assert result.returncode == test_case.expected_returncode, printed
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed
    for fragment in test_case.unexpected_fragments:
        assert fragment not in printed, printed


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroCompileTestCase(
            description="comments_from_a_macro_expansion_are_not_judged_in_the_caller",
            files={
                **with_rules("SQLCLEAN033"),
                **_COMMENTED_CHECK,
                **orders_model(
                    "SELECT order_id, NOT @sql.check(status) AS plain, "
                    'COALESCE(@sql.check(status), FALSE) AS checked FROM __ref("stg_orders")\n'
                ),
            },
            expected_returncode=1,
            expected_fragments=(f"--> {CHECK_MACRO_PATH}:",),
            unexpected_fragments=("--> models/marts/orders.sql",),
        ),
        SqlMacroCompileTestCase(
            description="comments_a_python_macro_returns_are_judged_in_the_caller",
            files={
                **with_rules("SQLCLEAN033"),
                **WRAPPING_MACROS,
                **orders_model(
                    "SELECT order_id, @py.noted('status') AS noted FROM __ref(\"stg_orders\")\n"
                ),
            },
            expected_returncode=1,
            expected_fragments=(
                "[SQLCLEAN033]",
                "(in SQL generated by @py.noted('status'))",
                "--> models/marts/orders.sql:3:",
            ),
        ),
        SqlMacroCompileTestCase(
            description="the_same_comments_written_in_a_model_are_judged",
            files={
                **with_rules("SQLCLEAN033"),
                **orders_model(
                    "SELECT order_id, NOT -- a status holding an at sign is flagged\n"
                    "CONTAINS(status, /* the sign */ '@') AS plain FROM __ref(\"stg_orders\")\n"
                ),
            },
            expected_returncode=1,
            expected_fragments=("[SQLCLEAN033]", "--> models/marts/orders.sql:3:"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_comments_in_a_sql_macro_when_linting_its_caller_then_only_authored_comments_count(
    test_case: SqlMacroCompileTestCase, tmp_path: Path
) -> None:
    _, result = run_project(tmp_path=tmp_path, files=test_case.files, command=_COMMAND)
    printed: str = output(result)

    assert result.returncode == test_case.expected_returncode, printed
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed
    for fragment in test_case.unexpected_fragments:
        assert fragment not in printed, printed
