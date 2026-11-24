"""E2E coverage for every SQL macro compile error, through the real CLI on DuckDB."""

from __future__ import annotations

import subprocess
from pathlib import Path

import pytest

from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros._test_types import (
    SqlMacroErrorTestCase,
    SqlMacroRecompileTestCase,
    SqlMacroWaveTestCase,
)
from tests.e2e.src.sqlbuild.cli.commands.main.compile.sql_macros.helpers import (
    AUDIT_COLUMNS,
    BASE_FILES,
    BRACKET,
    CENTS_TO_DOLLARS,
    CLEAN_ORDERS,
    IN_SUPPORTED_COUNTRY,
    IS_OPEN_STATUS,
    LATEST_PER_KEY,
    orders_model,
    output,
    run_project,
    wave_models,
)
from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import run_sqb

_X_HEADER: str = 'MACRO (description "X", arguments ({args}){returns});\n\n'
_CALL_X: dict[str, str] = orders_model("SELECT @sql.x(1) AS a\n")


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroErrorTestCase(
            description="bare_call_names_both_kinds",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    'SELECT\n  @cents_to_dollars(amount_cents) AS d FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="P014",
            expected_fragments=(
                "macro call @cents_to_dollars() does not say which kind of macro it calls",
                "write @sql.cents_to_dollars(...) to call a SQL macro or @py.cents_to_dollars(...) to call a Python macro",
                "sql/models/marts/orders.sql:4:3",
            ),
        ),
        SqlMacroErrorTestCase(
            description="python_file_directly_under_macros",
            files={"sql/macros/currency.py": "def c(x):\n    return x\n"},
            expected_code="D013",
            expected_fragments=(
                "macro file sql/macros/currency.py must live in sql/macros/python/ or sql/macros/sql/",
                "move it to sql/macros/python/currency.py",
            ),
        ),
        SqlMacroErrorTestCase(
            description="sql_file_in_python_folder",
            files={
                "sql/macros/python/c.sql": CENTS_TO_DOLLARS["sql/macros/sql/cents_to_dollars.sql"]
            },
            expected_code="D013",
            expected_fragments=(
                "macro file sql/macros/python/c.sql must live in sql/macros/python/ or sql/macros/sql/",
                "move it to sql/macros/sql/c.sql",
            ),
        ),
        SqlMacroErrorTestCase(
            description="scoped_file_outside_kind_folder",
            files={
                "sql/models/marts/_sqlbuild/_macros/c.sql": CENTS_TO_DOLLARS[
                    "sql/macros/sql/cents_to_dollars.sql"
                ]
            },
            expected_code="D013",
            expected_fragments=(
                "sql/models/marts/_sqlbuild/_macros/python/ or sql/models/marts/_sqlbuild/_macros/sql/",
            ),
        ),
        SqlMacroErrorTestCase(
            description="unknown_sql_macro",
            files=orders_model('SELECT @sql.nope(amount_cents) AS d FROM __ref("stg_orders")\n'),
            expected_code="P001",
            expected_fragments=("Unknown SQL macro '@sql.nope'",),
        ),
        SqlMacroErrorTestCase(
            description="sql_macro_called_as_python",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    'SELECT @py.cents_to_dollars(amount_cents) AS d FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="P020",
            expected_fragments=(
                "cents_to_dollars() is a SQL macro, called as a Python macro with @py.cents_to_dollars()",
                "call it as @sql.cents_to_dollars(...)",
            ),
        ),
        SqlMacroErrorTestCase(
            description="python_macro_called_as_sql",
            files={
                **BRACKET,
                **orders_model('SELECT @sql.bracket(amount_cents) AS d FROM __ref("stg_orders")\n'),
            },
            expected_code="P020",
            expected_fragments=(
                "bracket() is a Python macro, called as a SQL macro with @sql.bracket()",
            ),
        ),
        SqlMacroErrorTestCase(
            description="sql_body_calls_python_macro",
            files={
                **BRACKET,
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "@py.bracket(@v) + 1\n",
                **_CALL_X,
            },
            expected_code="P020",
            expected_fragments=(
                "SQL macro x() calls Python macro bracket(); SQL and Python macros never call each other",
                "sql/macros/sql/x.sql:3:1",
            ),
        ),
        SqlMacroErrorTestCase(
            description="reference_function_in_body",
            files={
                "sql/macros/sql/bad.sql": 'MACRO (\n  description "Bad",\n);\n\nWITH x AS (SELECT * FROM __ref("stg_orders"))\n',
                **orders_model("WITH @sql.bad()\nSELECT 1 AS a\n"),
            },
            expected_code="P016",
            expected_fragments=(
                "macro bad() reads model:stg_orders through __ref() in its body",
                "sql/macros/sql/bad.sql:5:26",
            ),
        ),
        SqlMacroErrorTestCase(
            description="macros_calling_each_other",
            files={
                "sql/macros/sql/a.sql": _X_HEADER.replace("X", "A").format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "@sql.b(@v) + 1\n",
                "sql/macros/sql/b.sql": _X_HEADER.replace("X", "B").format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "@sql.a(@v) + 1\n",
                **orders_model('SELECT @sql.a(amount_cents) AS d FROM __ref("stg_orders")\n'),
            },
            expected_code="P019",
            expected_fragments=("SQL macros call each other in a cycle:",),
        ),
        SqlMacroErrorTestCase(
            description="cte_name_clash",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    'WITH orders_raw AS (SELECT * FROM __ref("stg_orders")),\nlatest AS (SELECT 1 AS x),\n@sql.clean_orders(orders_raw)\nSELECT * FROM latest\n'
                ),
            },
            expected_code="P018",
            expected_fragments=(
                'CTE "latest" from macro clean_orders() clashes with a CTE already defined in model:orders',
            ),
        ),
        SqlMacroErrorTestCase(
            description="column_name_clash",
            files={
                **AUDIT_COLUMNS,
                **orders_model(
                    'SELECT o.created_at, @sql.audit_columns(o) FROM __ref("stg_orders") AS o\n'
                ),
            },
            expected_code="P018",
            expected_fragments=(
                'column "created_at" from macro audit_columns() clashes with a column already defined in model:orders',
            ),
        ),
        SqlMacroErrorTestCase(
            description="cte_macro_called_twice",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    'WITH orders_raw AS (SELECT * FROM __ref("stg_orders")),\n@sql.clean_orders(orders_raw)\n@sql.clean_orders(orders_raw)\nSELECT * FROM latest\n'
                ),
            },
            expected_code="P018",
            expected_fragments=(
                'CTE "live" from macro clean_orders() clashes with a CTE from macro clean_orders()',
            ),
        ),
        SqlMacroErrorTestCase(
            description="missing_argument",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model('SELECT @sql.cents_to_dollars() AS d FROM __ref("stg_orders")\n'),
            },
            expected_code="P017",
            expected_fragments=(
                "call to macro cents_to_dollars() is missing argument amount_cents BIGINT",
            ),
        ),
        SqlMacroErrorTestCase(
            description="unknown_named_argument",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    'SELECT @sql.cents_to_dollars(amount_cents, digits := 2) AS d FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="P017",
            expected_fragments=("macro cents_to_dollars() has no argument digits",),
        ),
        SqlMacroErrorTestCase(
            description="too_many_arguments",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    'SELECT @sql.cents_to_dollars(amount_cents, 2, 3) AS d FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="P017",
            expected_fragments=("macro cents_to_dollars() takes 2 arguments; the call passes 3",),
        ),
        SqlMacroErrorTestCase(
            description="column_for_const_argument",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    'SELECT @sql.cents_to_dollars(amount_cents, places := fee_cents) AS d FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "argument places of macro cents_to_dollars() expects INT CONST: a literal, a scalar constant or an enum value; got fee_cents",
            ),
        ),
        SqlMacroErrorTestCase(
            description="runtime_parameter_forwarded_to_const",
            files={
                **CENTS_TO_DOLLARS,
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="v BIGINT, p INT", returns=", returns DECIMAL(18, 2)"
                )
                + "@sql.cents_to_dollars(@v, @p)\n",
                **orders_model('SELECT @sql.x(amount_cents, 1) AS a FROM __ref("stg_orders")\n'),
            },
            expected_code="P017",
            expected_fragments=(
                "argument places of macro cents_to_dollars() expects INT CONST",
                "declare p CONST in macro x()",
            ),
        ),
        SqlMacroErrorTestCase(
            description="wrong_enum",
            files={
                **IS_OPEN_STATUS,
                "sql/enums/other.sql": 'ENUM (name other, members (X "x"));\n',
                **orders_model(
                    'SELECT order_id FROM __ref("stg_orders") WHERE @sql.is_open_status(status, @enum("other").X)\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                'argument closed of macro is_open_status() expects ENUM order_status: a member of enum order_status, such as @enum("order_status").MEMBER; got @enum("other").X',
            ),
        ),
        SqlMacroErrorTestCase(
            description="literal_for_enum",
            files={
                **IS_OPEN_STATUS,
                **orders_model(
                    "SELECT order_id FROM __ref(\"stg_orders\") WHERE @sql.is_open_status(status, 'closed')\n"
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "argument closed of macro is_open_status() expects ENUM order_status",
            ),
        ),
        SqlMacroErrorTestCase(
            description="scalar_for_list",
            files={
                **IN_SUPPORTED_COUNTRY,
                **orders_model(
                    "SELECT order_id FROM __ref(\"stg_orders\") WHERE @sql.in_supported_country(country_code, countries := 'GB')\n"
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "argument countries of macro in_supported_country() expects LIST VARCHAR: an inline list or set, or a list or set constant; got 'GB'",
            ),
        ),
        SqlMacroErrorTestCase(
            description="expression_for_relation",
            files={
                **AUDIT_COLUMNS,
                **orders_model('SELECT @sql.audit_columns(1 + 2) FROM __ref("stg_orders") AS o\n'),
            },
            expected_code="P017",
            expected_fragments=(
                "argument src of macro audit_columns() expects RELATION (created_at TIMESTAMP, updated_at TIMESTAMP)",
            ),
        ),
        SqlMacroErrorTestCase(
            description="expression_for_a_columns_slot",
            files={
                **LATEST_PER_KEY,
                **orders_model(
                    "WITH orders_raw AS (SELECT * FROM __ref(\"stg_orders\")),\n@sql.latest_per_key(orders_raw, src.keys := (status || 'x'))\nSELECT * FROM latest_rows\n"
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "slot src.keys of macro latest_per_key() takes a parenthesised list of column names of the passed relation; got (status || 'x')",
            ),
        ),
        SqlMacroErrorTestCase(
            description="cte_macro_outside_with_list",
            files={
                **CLEAN_ORDERS,
                **orders_model('SELECT @sql.clean_orders(o) FROM __ref("stg_orders") AS o\n'),
            },
            expected_code="P021",
            expected_fragments=("CTE-list macro clean_orders() must be called inside a WITH list",),
        ),
        SqlMacroErrorTestCase(
            description="column_macro_outside_select_list",
            files={
                **AUDIT_COLUMNS,
                **orders_model(
                    'SELECT o.order_id FROM __ref("stg_orders") AS o WHERE @sql.audit_columns(o)\n'
                ),
            },
            expected_code="P021",
            expected_fragments=(
                "column-list macro audit_columns() must be called as an item of a select list",
            ),
        ),
        SqlMacroErrorTestCase(
            description="returns_that_disagrees_with_the_body_is_reported_at_the_macro_file",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="v VARCHAR", returns=", returns BIGINT"
                )
                + "UPPER(@v)\n",
                **orders_model("SELECT @sql.x('a') AS a\n"),
            },
            expected_code="P024",
            expected_fragments=(
                "macro x() declares returns BIGINT, but its body is VARCHAR\n"
                "  model: orders\n"
                "  --> sql/macros/sql/x.sql:3:1",
                "declare returns VARCHAR, CAST the body to BIGINT, or remove returns",
            ),
        ),
        SqlMacroErrorTestCase(
            description="untyped_argument",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(args="v", returns=", returns BIGINT")
                + "@v + 1\n",
                **_CALL_X,
            },
            expected_code="P015",
            expected_fragments=("argument v of macro x() has no type",),
        ),
        SqlMacroErrorTestCase(
            description="raw_text_argument",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(args="v RAW", returns=", returns BIGINT")
                + "@v + 1\n",
                **_CALL_X,
            },
            expected_code="P015",
            expected_fragments=(
                "argument v of macro x() is raw text; SQL macro arguments are typed SQL",
            ),
        ),
        SqlMacroErrorTestCase(
            description="type_the_adapter_cannot_parse",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="v FOO BAR", returns=", returns BIGINT"
                )
                + "@v + 1\n",
                **_CALL_X,
            },
            expected_code="P015",
            expected_fragments=(
                "argument v of macro x() type 'FOO BAR' is not valid for adapter 'duckdb'",
            ),
        ),
        SqlMacroErrorTestCase(
            description="list_declared_const",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="codes LIST VARCHAR CONST", returns=", returns BOOLEAN"
                )
                + "'GB' IN @codes\n",
                **orders_model('SELECT @sql.x(["GB"]) AS a\n'),
            },
            expected_code="P015",
            expected_fragments=(
                "argument codes LIST VARCHAR CONST: a LIST argument is always known at compile time, so it takes no CONST",
                "declare it codes LIST VARCHAR",
            ),
        ),
        SqlMacroErrorTestCase(
            description="array_declared_const",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="codes VARCHAR[] CONST", returns=", returns BOOLEAN"
                )
                + "len(@codes) > 0\n",
                **orders_model("SELECT @sql.x(['GB']) AS a\n"),
            },
            expected_code="P015",
            expected_fragments=(
                "argument codes VARCHAR[] CONST: CONST is for scalar types only",
                "declare it codes LIST VARCHAR for a list known at compile time, or codes VARCHAR[] for a runtime array",
            ),
        ),
        SqlMacroErrorTestCase(
            description="unnamed_column_list_column",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(args="src RELATION (a BIGINT)", returns="")
                + "SELECT @src.a + 1\n",
                **orders_model("SELECT @sql.x(o) FROM (SELECT 1 AS a) AS o\n"),
            },
            expected_code="P015",
            expected_fragments=(
                "every column of a column-list macro needs a name; add AS <name> to @src.a + 1",
            ),
        ),
        SqlMacroErrorTestCase(
            description="cte_macro_declaring_returns",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "WITH y AS (SELECT @v AS a)\n",
                **orders_model("WITH @sql.x(1)\nSELECT * FROM y\n"),
            },
            expected_code="P015",
            expected_fragments=(
                "CTE-list macro x() cannot declare returns; only expression macros return a value",
            ),
        ),
        SqlMacroErrorTestCase(
            description="built_in_parameter_name",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="enum BIGINT", returns=", returns BIGINT"
                )
                + "1\n",
                **_CALL_X,
            },
            expected_code="P015",
            expected_fragments=("argument enum of macro x() uses a built-in name",),
        ),
        SqlMacroErrorTestCase(
            description="undeclared_parameter",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "@v + @w\n",
                **_CALL_X,
            },
            expected_code="P022",
            expected_fragments=(
                "@w in the body of macro x() names no parameter of the macro",
                "its parameters are @v",
                "sql/macros/sql/x.sql:3:6",
            ),
        ),
        SqlMacroErrorTestCase(
            description="relation_parameter_written_bare",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="src RELATION (order_id BIGINT)", returns=""
                )
                + "WITH ids AS (SELECT order_id FROM src)\n",
                **orders_model(
                    'WITH o AS (SELECT * FROM __ref("stg_orders")),\n@sql.x(o)\nSELECT * FROM ids\n'
                ),
            },
            expected_code="P022",
            expected_fragments=("src is a parameter of macro x(); write @src",),
        ),
        SqlMacroErrorTestCase(
            description="relation_parameter_qualifier_written_bare",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="src RELATION (order_id BIGINT)", returns=""
                )
                + "SELECT src.order_id\n",
                **orders_model('SELECT @sql.x(o) FROM __ref("stg_orders") AS o\n'),
            },
            expected_code="P022",
            expected_fragments=("src is a parameter of macro x(); write @src",),
        ),
        SqlMacroErrorTestCase(
            description="runtime_parameter_as_limit",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="src RELATION (order_id BIGINT), n BIGINT", returns=""
                )
                + "WITH firsts AS (SELECT order_id FROM @src LIMIT @n)\n",
                **orders_model(
                    'WITH o AS (SELECT * FROM __ref("stg_orders")),\n@sql.x(o, 1)\nSELECT * FROM firsts\n'
                ),
            },
            expected_code="P023",
            expected_fragments=(
                "n is used where a literal is required; declare it n BIGINT CONST",
                "uses @n as LIMIT",
            ),
        ),
        SqlMacroErrorTestCase(
            description="runtime_parameter_as_interval_unit",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="at TIMESTAMP, unit VARCHAR", returns=", returns TIMESTAMP"
                )
                + "@at + INTERVAL 1 @unit\n",
                **orders_model('SELECT @sql.x(created_at, day) AS a FROM __ref("stg_orders")\n'),
            },
            expected_code="P023",
            expected_fragments=(
                "unit is used where a literal is required; declare it unit VARCHAR CONST",
                "an INTERVAL unit",
            ),
        ),
        SqlMacroErrorTestCase(
            description="runtime_parameter_as_pivot_value",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="src RELATION (status VARCHAR, amount_cents BIGINT), first_status VARCHAR",
                    returns="",
                )
                + "WITH pivoted AS (SELECT * FROM @src PIVOT (SUM(amount_cents) FOR status IN (@first_status, 'closed')))\n",
                **orders_model(
                    "WITH o AS (SELECT * FROM __ref(\"stg_orders\")),\n@sql.x(o, 'open')\nSELECT * FROM pivoted\n"
                ),
            },
            expected_code="P023",
            expected_fragments=(
                "first_status is used where a literal is required",
                "PIVOT ... IN values",
            ),
        ),
        SqlMacroErrorTestCase(
            description="runtime_parameter_as_round_precision_on_duckdb",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="v DECIMAL(18, 4), places INT", returns=", returns DECIMAL(18, 2)"
                )
                + "ROUND(@v, @places)\n",
                **orders_model(
                    'SELECT @sql.x(amount_cents / 100, 2) AS a FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="P023",
            expected_fragments=(
                "places is used where a literal is required; declare it places INT CONST",
                "ROUND precision",
            ),
        ),
        SqlMacroErrorTestCase(
            description="runtime_parameter_as_date_trunc_unit",
            files={
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="at TIMESTAMP, unit VARCHAR", returns=", returns TIMESTAMP"
                )
                + "DATE_TRUNC(@unit, @at)\n",
                **orders_model(
                    "SELECT @sql.x(created_at, 'month') AS a FROM __ref(\"stg_orders\")\n"
                ),
            },
            expected_code="P023",
            expected_fragments=(
                "unit is used where a literal is required; declare it unit VARCHAR CONST",
                "a DATE_TRUNC unit",
            ),
        ),
        SqlMacroErrorTestCase(
            description="body_constant_not_visible_from_macro_file",
            files={
                "sql/models/marts/_sqlbuild/_constants/places.sql": "CONSTANT (name price_places, value 2);\n",
                "sql/macros/sql/x.sql": _X_HEADER.format(
                    args="v BIGINT", returns=", returns DECIMAL(18, 2)"
                )
                + 'ROUND(@v / 100, @const("price_places"))\n',
                **orders_model('SELECT @sql.x(amount_cents) AS a FROM __ref("stg_orders")\n'),
            },
            expected_code="S026",
            expected_fragments=("constant price_places is not visible from sql/macros/sql/x.sql",),
        ),
        SqlMacroErrorTestCase(
            description="scoped_macro_not_visible_from_caller",
            files={
                "sql/models/staging/_sqlbuild/_macros/sql/x.sql": _X_HEADER.format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "@v + 1\n",
                **orders_model('SELECT @sql.x(amount_cents) AS a FROM __ref("stg_orders")\n'),
            },
            expected_code="S006",
            expected_fragments=(
                "SQL macro x() defined at sql/models/staging/_sqlbuild/_macros/sql/x.sql is not visible from",
            ),
        ),
        SqlMacroErrorTestCase(
            description="folder_local_macro_called_from_a_subfolder_model",
            files={
                "sql/models/marts/_sqlbuild/_macros/sql/doubled_inner.sql": _X_HEADER.format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "@v * 2\n",
                "sql/models/marts/daily/orders.sql": (
                    'MODEL (description "Orders");\n\nSELECT @sql.doubled_inner(order_id) AS d '
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="S006",
            expected_fragments=(
                "SQL macro doubled_inner() defined at sql/models/marts/_sqlbuild/_macros/sql/doubled_inner.sql is not visible from",
            ),
        ),
        SqlMacroErrorTestCase(
            description="sql_and_python_macro_sharing_a_name",
            files={
                "sql/macros/sql/same.sql": _X_HEADER.format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "@v + 1\n",
                "sql/macros/python/same.py": "def same(v):\n    return v\n",
                **orders_model('SELECT @sql.same(amount_cents) AS a FROM __ref("stg_orders")\n'),
            },
            expected_code="P001",
            expected_fragments=(
                "Macro name collision for 'same'",
                "sql/macros/python/same.py",
                "sql/macros/sql/same.sql",
            ),
        ),
        SqlMacroErrorTestCase(
            description="call_after_interpolation_is_located_at_its_own_span",
            files={
                **CENTS_TO_DOLLARS,
                "sqlbuild_project.toml": BASE_FILES["sqlbuild_project.toml"]
                + '\n[vars]\nfactor = "10"\n',
                **orders_model(
                    "SELECT @@factor AS factor,\n"
                    "  @sql.cents_to_dollars(COALESCE(fee_cents, 0)) AS a,\n"
                    "  @sql.cents_to_dollars(COALESCE(fee_cents, 0), places := fee_cents) AS b\n"
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="P017",
            expected_fragments=("expects INT CONST", "sql/models/marts/orders.sql:5:3"),
        ),
        SqlMacroErrorTestCase(
            description="relation_missing_a_declared_column",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    'WITH orders_raw AS (SELECT order_id, updated_at FROM __ref("stg_orders")),\n@sql.clean_orders(orders_raw)\nSELECT * FROM latest\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "argument src of macro clean_orders() needs column is_deleted BOOLEAN; orders_raw has no column is_deleted",
                "sql/models/marts/orders.sql:4:1",
                "sql/macros/sql/clean_orders.sql:3:",
                "pass a relation with these columns: order_id BIGINT, updated_at TIMESTAMP, is_deleted BOOLEAN",
            ),
        ),
        SqlMacroErrorTestCase(
            description="relation_missing_a_column_behind_a_star_over_typed_sources",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    "WITH orders_raw AS (\n"
                    '  SELECT o.order_id, o.updated_at, t.* FROM __ref("stg_orders") AS o\n'
                    "  CROSS JOIN (SELECT 1 AS flag) AS t\n),\n"
                    "@sql.clean_orders(orders_raw)\nSELECT order_id FROM latest\n"
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "argument src of macro clean_orders() needs column is_deleted BOOLEAN; orders_raw has no column is_deleted",
            ),
        ),
        SqlMacroErrorTestCase(
            description="relation_column_of_an_incompatible_type",
            files={
                **CLEAN_ORDERS,
                **orders_model(
                    'WITH orders_raw AS (SELECT order_id, updated_at, status AS is_deleted FROM __ref("stg_orders")),\n@sql.clean_orders(orders_raw)\nSELECT * FROM latest\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "argument src of macro clean_orders() needs column is_deleted BOOLEAN; orders_raw.is_deleted is VARCHAR",
                "sql/models/marts/orders.sql:4:1",
            ),
        ),
        SqlMacroErrorTestCase(
            description="columns_slot_naming_a_missing_column",
            files={
                **LATEST_PER_KEY,
                **orders_model(
                    'WITH orders_raw AS (SELECT * FROM __ref("stg_orders")),\n@sql.latest_per_key(orders_raw, src.keys := (status, email_typo))\nSELECT * FROM latest_rows\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "slot src.keys groups column email_typo; orders_raw has no column email_typo",
                "sql/models/marts/orders.sql:4:1",
                "sql/macros/sql/latest_per_key.sql:",
            ),
        ),
        SqlMacroErrorTestCase(
            description="argument_of_the_wrong_type",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    'SELECT o.order_id,\n  @sql.cents_to_dollars(o.status) AS d\nFROM __ref("stg_orders") AS o\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "argument amount_cents of macro cents_to_dollars() expects BIGINT; o.status is VARCHAR",
                "sql/models/marts/orders.sql:4:3",
                "sql/macros/sql/cents_to_dollars.sql:3:",
            ),
        ),
        SqlMacroErrorTestCase(
            description="argument_of_the_wrong_type_inside_a_cte",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    'WITH o AS (SELECT * FROM __ref("stg_orders")),\nfinal AS (SELECT @sql.cents_to_dollars(o.created_at) AS d FROM o)\nSELECT * FROM final\n'
                ),
            },
            expected_code="P017",
            expected_fragments=(
                "argument amount_cents of macro cents_to_dollars() expects BIGINT; o.created_at is TIMESTAMP",
            ),
        ),
        SqlMacroErrorTestCase(
            description="column_list_call_after_distinct_on",
            files={
                **AUDIT_COLUMNS,
                **orders_model(
                    "SELECT DISTINCT ON (o.order_id) @sql.audit_columns(o), o.order_id "
                    'FROM __ref("stg_orders") AS o\n'
                ),
            },
            expected_code="P021",
            expected_fragments=(
                "column-list macro audit_columns() must be called as an item of a select list",
            ),
        ),
        SqlMacroErrorTestCase(
            description="same_named_folder_local_macros_in_two_folders",
            files={
                "sql/models/a/_sqlbuild/_macros/sql/helper.sql": _X_HEADER.format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "@v * 2\n",
                "sql/models/b/_sqlbuild/_macros/sql/helper.sql": _X_HEADER.format(
                    args="v BIGINT", returns=", returns BIGINT"
                )
                + "@v * 3\n",
                "sql/models/a/model_a.sql": (
                    'MODEL (description "A");\n\nSELECT @sql.helper(order_id) AS x '
                    'FROM __ref("stg_orders")\n'
                ),
            },
            expected_code="P001",
            expected_fragments=("Macro name collision for 'helper'",),
        ),
        SqlMacroErrorTestCase(
            description="alias_after_a_column_list_call",
            files={
                **AUDIT_COLUMNS,
                **orders_model(
                    'SELECT o.order_id, @sql.audit_columns(o) AS audit FROM __ref("stg_orders") AS o\n'
                ),
            },
            expected_code="P021",
            expected_fragments=(
                "column-list macro audit_columns() cannot take an alias",
                "sql/models/marts/orders.sql:3:20",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_an_invalid_sql_macro_project_when_compiling_then_the_error_names_the_problem(
    test_case: SqlMacroErrorTestCase, tmp_path: Path
) -> None:
    _, result = run_project(tmp_path=tmp_path, files=test_case.files, command=("compile",))
    printed: str = output(result)

    assert result.returncode == 1, printed
    assert f"error[{test_case.expected_code}]" in printed, printed
    for fragment in test_case.expected_fragments:
        assert fragment in printed, printed


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroRecompileTestCase(
            description="parameter_type_changed_without_changing_the_expansion",
            files={
                **CENTS_TO_DOLLARS,
                **orders_model(
                    'SELECT o.order_id, @sql.cents_to_dollars(o.amount_cents) AS d FROM __ref("stg_orders") AS o\n'
                ),
            },
            edited_path="sql/macros/sql/cents_to_dollars.sql",
            replaced_text=("amount_cents BIGINT", "amount_cents BOOLEAN"),
            expected_code="P017",
            expected_fragment=(
                "argument amount_cents of macro cents_to_dollars() expects BOOLEAN; "
                "o.amount_cents is BIGINT"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_a_compiled_project_when_a_macro_signature_changes_then_the_next_compile_checks_it(
    test_case: SqlMacroRecompileTestCase, tmp_path: Path
) -> None:
    project_dir, first = run_project(tmp_path=tmp_path, files=test_case.files, command=("compile",))
    assert first.returncode == 0, output(first)
    macro: Path = project_dir / test_case.edited_path
    macro.write_text(macro.read_text().replace(*test_case.replaced_text), encoding="utf-8")

    second: subprocess.CompletedProcess[str] = run_sqb(
        command=("--no-color", "compile"), project_dir=project_dir
    )
    printed: str = output(second)

    assert second.returncode == 1, printed
    assert f"error[{test_case.expected_code}]" in printed, printed
    assert test_case.expected_fragment in printed, printed


@pytest.mark.parametrize(
    "test_case",
    [
        SqlMacroWaveTestCase(
            description="six_waves_each_with_one_wrong_typed_call",
            files={**CENTS_TO_DOLLARS, **wave_models(6)},
            wave_count=6,
            expected_finding=(
                "argument amount_cents of macro cents_to_dollars() expects BIGINT; "
                "o.status is VARCHAR"
            ),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_typed_calls_in_every_wave_when_compiling_then_each_model_reports_its_own(
    test_case: SqlMacroWaveTestCase, tmp_path: Path
) -> None:
    _, result = run_project(
        tmp_path=tmp_path, files=test_case.files, command=("compile", "--no-cache")
    )
    printed: str = output(result)

    assert result.returncode == 1, printed
    assert printed.count(test_case.expected_finding) == test_case.wave_count, printed
    for level in range(1, test_case.wave_count + 1):
        assert f"sql/models/marts/level_{level}.sql:5:3" in printed, printed
