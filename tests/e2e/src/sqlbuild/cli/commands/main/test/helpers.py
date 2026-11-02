"""Helpers for sqb test command e2e tests."""

from __future__ import annotations

import json
import os
import re
import subprocess
import sys
from collections.abc import Mapping
from pathlib import Path
from typing import Any

import duckdb

from tests.e2e.src.sqlbuild.cli.commands.shared.helpers import (
    execute_duckdb,
    prepare_inline_project,
)

_OPTIONAL_CEREMONY_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": (
        'name = "optional_ceremony"\nadapter = "duckdb"\n\n'
        '[connection]\ndatabase = "optional_ceremony.duckdb"\n\n'
        '[defaults]\nmaterialized = "table"\n'
    ),
    "macros/python/status.py": (
        'def normalize_status(value: str) -> str:\n    return f"LOWER(TRIM({value}))"\n'
    ),
    "sources/raw_orders.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n"
        "    table: raw_orders\n"
        "    columns:\n"
        "      - name: order_id\n        type: INTEGER\n"
        "      - name: status\n        type: VARCHAR\n"
    ),
    "models/orders.sql": (
        "MODEL (description 'Test model orders.');\n\n"
        'SELECT order_id, @py.normalize_status("status") AS status FROM __source("raw_orders")\n'
    ),
}


def build_dynamic_pivot_test_project_files() -> dict[str, str]:
    """Build a Snowflake dynamic-pivot project for offline test-plan inspection."""

    return {
        "sqlbuild_project.toml": (
            'name = "dynamic_pivot_test_plan"\n'
            'adapter = "snowflake"\n\n'
            "[defaults]\n"
            'contract = "enforced"\n'
        ),
        "models/stg_order_amounts.sql": (
            "MODEL (description 'Test model stg_order_amounts.',\n"
            "  database analytics,\n"
            "  schema staging,\n"
            "  columns (\n"
            "    customer_id (type INTEGER),\n"
            "    category (type VARCHAR),\n"
            '    amount (type "DECIMAL(12,2)"),\n'
            "    country (type VARCHAR),\n"
            "  ),\n"
            ");\n"
            "SELECT CAST(1 AS INTEGER) AS customer_id, "
            "CAST('books' AS VARCHAR) AS category, "
            "CAST(10.25 AS DECIMAL(12,2)) AS amount, "
            "CAST('US' AS VARCHAR) AS country\n"
        ),
        "models/customer_category_amounts.sql": (
            "MODEL (description 'Test model customer_category_amounts.',\n"
            "  database analytics,\n"
            "  schema mart,\n"
            "  columns (\n"
            "    customer_id (type INTEGER),\n"
            "    country (type VARCHAR),\n"
            "  ),\n"
            "  dynamic_columns (\n"
            "    category_amounts (\n"
            "      pivot_column category,\n"
            "      value_column amount,\n"
            "      aggregate MAX,\n"
            '      type "DECIMAL(12,2)"\n'
            "    )\n"
            "  ),\n"
            ");\n"
            "WITH pivot_input AS (\n"
            "  SELECT customer_id, category, amount, country\n"
            '  FROM __ref("stg_order_amounts")\n'
            "), final AS (\n"
            "  SELECT *\n"
            "  FROM pivot_input PIVOT(MAX(amount) FOR category IN (ANY ORDER BY category))\n"
            ")\n"
            "SELECT * FROM final\n"
        ),
        "tests/unit/test_customer_category_amounts.sql": (
            'TEST (name "customer_category_amounts__empty_input");\n\n'
            "WITH __ref__stg_order_amounts AS (\n"
            "  SELECT\n"
            "    CAST(NULL AS INTEGER) AS customer_id,\n"
            "    CAST(NULL AS VARCHAR) AS category,\n"
            "    CAST(NULL AS DECIMAL(12,2)) AS amount,\n"
            "    CAST(NULL AS VARCHAR) AS country\n"
            "  WHERE FALSE\n"
            "), __assert__empty AS (\n"
            '  SELECT 1 AS unexpected_row FROM __ref("customer_category_amounts")\n'
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_chain_test_project_files(*, sql_analysis_enabled: bool) -> dict[str, str]:
    """Build an inline project with a two-model SQL unit-test chain."""

    sql_analysis_value: str = {False: "false", True: "true"}[sql_analysis_enabled]
    return {
        "sqlbuild_project.toml": (
            'name = "demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "demo.duckdb"\n\n'
            "[settings]\n"
            f"sql_analysis = {sql_analysis_value}\n"
        ),
        "models/stg_orders.sql": (
            "MODEL (description 'Test model stg_orders.', materialized table);\n\n"
            "SELECT\n"
            "  id,\n"
            "  @py.mocked_amount() AS amount,\n"
            "  @py.mocked_country() AS country,\n"
            "  @py.mocked_literal_text() AS literal_text,\n"
            "  @py.real_status() AS status\n"
            'FROM __source("raw")'
        ),
        "models/fact_orders.sql": (
            "MODEL (description 'Test model fact_orders.', materialized table);\n\n"
            "SELECT\n"
            "  id,\n"
            "  amount + 1 AS adjusted,\n"
            "  country,\n"
            "  literal_text,\n"
            "  status\n"
            'FROM __ref("stg_orders")'
        ),
        "models/_macros/python/test_macros.py": (
            "def mocked_amount() -> str:\n"
            '    return "0"\n\n'
            "def mocked_country() -> str:\n"
            "    return \"'CA'\"\n\n"
            "def mocked_literal_text() -> str:\n"
            "    return \"'real'\"\n\n"
            "def real_status() -> str:\n"
            "    return \"'active'\"\n"
        ),
        "sources/raw.yml": "sources:\n  - name: raw\n    description: Test source raw.\n"
        "    schema: main\n    table: raw\n",
        "tests/unit/test_chain.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__macro__mocked_amount AS (SELECT '100'),\n"
            "__macro__mocked_country AS (SELECT '''US'''),\n"
            "__macro__mocked_literal_text AS (SELECT ''' + x + '''),\n"
            "__source__raw AS (SELECT 1 AS id),\n"
            "__expected__stg_orders AS (\n"
            "  SELECT 1 AS id, 100 AS amount, 'US' AS country, ' + x + ' AS literal_text, "
            "'active' AS status\n"
            "),\n"
            "__expected__fact_orders AS (\n"
            "  SELECT 1 AS id, 101 AS adjusted, 'US' AS country, ' + x + ' AS "
            "literal_text, 'active' AS status\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_parameterized_test_project_files() -> dict[str, str]:
    """Build an inline project with two independently selectable test cases."""

    return {
        "sqlbuild_project.toml": (
            'name = "parameter_case_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "parameter_case_demo.duckdb"\n'
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.");\n\n'
            'SELECT status FROM __source("raw_orders")\n'
        ),
        "models/customers.sql": (
            'MODEL (description "Test model customers.");\n\n'
            'SELECT status FROM __source("raw_orders")\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "tests/unit/test_orders.sql": (
            'TEST (name "order_status", parameters (status string), cases ('
            'open_case (status "open"), closed_case (status "closed")));\n\n'
            "WITH\n"
            '__source__raw_orders AS (SELECT @param("status") AS status),\n'
            '__expected__orders AS (SELECT @param("status") AS status)\n'
            "SELECT 1\n"
        ),
        "tests/unit/test_customers.sql": (
            'TEST (name "customer_status", parameters (status string), '
            'cases (customer_only (status "active")));\n\n'
            "WITH\n"
            '__source__raw_orders AS (SELECT @param("status") AS status),\n'
            '__expected__customers AS (SELECT @param("status") AS status)\n'
            "SELECT 1\n"
        ),
    }


def build_table_function_fixture_project_files() -> dict[str, str]:
    """Build a model test whose table function is replaced by a relation fixture."""

    return {
        "sqlbuild_project.toml": (
            'name = "table_function_fixture_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "table_function_fixture_demo.duckdb"\n'
        ),
        "functions/sql/customer_orders.sql": (
            "FUNCTION (description 'Test function customer_orders.',\n"
            "  arguments (customer_id INTEGER),\n"
            "  returns table (order_id INTEGER)\n"
            ");\n\n"
            "SELECT customer_id * 100 AS order_id\n"
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.', columns (order_id (type INTEGER)));\n\n"
            'SELECT order_id FROM __table_fn("customer_orders")(7)\n'
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "fixture_rows AS (SELECT 7 AS order_id),\n"
            "__table_fn__customer_orders AS (SELECT order_id FROM fixture_rows),\n"
            "__expected__orders AS (SELECT 7 AS order_id)\n"
            "SELECT 1\n"
        ),
    }


def build_mock_boundary_test_project_files() -> dict[str, str]:
    """Build a test whose ref mock intentionally replaces one real model."""

    return {
        "sqlbuild_project.toml": (
            'name = "mock_boundary_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "mock_boundary_demo.duckdb"\n'
        ),
        "models/stg_orders.sql": "MODEL (description 'Test model stg_orders.');\n\n"
        "SELECT 1 AS order_id\n",
        "models/int_orders.sql": (
            'MODEL (description "Test model int_orders.");\n\n'
            'SELECT order_id FROM __ref("stg_orders")\n'
        ),
        "models/fact_orders.sql": (
            'MODEL (description "Test model fact_orders.");\n\n'
            'SELECT order_id FROM __ref("int_orders")\n'
        ),
        "tests/unit/test_fact_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id),\n"
            "__expected__fact_orders AS (SELECT 1 AS order_id)\n"
            "SELECT 1\n"
        ),
    }


MOCKED_UNSATISFIED_LEAF_TEST_SQL: str = (
    "TEST();\n\n"
    "WITH\n"
    "__source__raw AS (SELECT 1 AS id),\n"
    "__expected__fact_orders AS (\n"
    "  SELECT 1 AS id, 1 AS adjusted, 'CA' AS country, "
    "'real' AS literal_text, 'active' AS status\n"
    ")\n"
    "SELECT 1\n"
)


def build_unsatisfied_leaf_test_project_files() -> dict[str, str]:
    """Build a test that omits the source mock required by its real model chain."""

    files: dict[str, str] = build_chain_test_project_files(sql_analysis_enabled=True)
    files["sources/raw.yml"] = (
        "sources:\n"
        "  - name: raw\n    description: Test source raw.\n    schema: main\n    table: raw\n"
        "  - name: unused\n    description: Test source unused.\n"
        "    schema: main\n    table: unused\n"
    )
    files["tests/unit/test_chain.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__unused AS (SELECT 1 AS id),\n"
        "__expected__fact_orders AS (\n"
        "  SELECT 1 AS id, 101 AS adjusted, 'US' AS country, "
        "'literal' AS literal_text, 'active' AS status\n"
        ")\n"
        "SELECT 1\n"
    )
    return files


def build_missing_mock_columns_project_files() -> dict[str, str]:
    """Build a test whose source fixture omits two statically required columns."""

    return {
        "sqlbuild_project.toml": (
            'name = "missing_fixture_columns"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "missing_fixture_columns.duckdb"\n'
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.");\n\n'
            'SELECT customer_id, status FROM __source("raw_orders")\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: customer_id\n"
            "      - name: status\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT 1 AS order_id),\n"
            "__expected__orders AS (SELECT 100 AS customer_id, 'open' AS status)\n"
            "SELECT 1\n"
        ),
    }


def build_partial_source_fixture_project_files() -> dict[str, str]:
    """Build a source fixture with one required nullable column omitted."""

    return {
        "sqlbuild_project.toml": (
            'name = "partial_source_fixture"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "partial_source_fixture.duckdb"\n'
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.");\n\n'
            'SELECT order_id, status FROM __source("raw_orders")\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: order_id\n        type: INTEGER\n        nullable: false\n"
            "      - name: status\n        type: VARCHAR\n        nullable: true\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT 1 AS order_id),\n"
            "__expected__orders AS (\n"
            "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_unspecified_nullability_fixture_project_files() -> dict[str, str]:
    """Build a typed partial source fixture whose nullability is unspecified."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["sources/raw_orders.yml"] = files["sources/raw_orders.yml"].replace(
        "        type: VARCHAR\n        nullable: true\n",
        "        type: VARCHAR\n",
    )
    return files


def build_partial_ref_fixture_project_files() -> dict[str, str]:
    """Build a model-ref fixture with one required nullable column omitted."""

    return {
        "sqlbuild_project.toml": (
            'name = "partial_ref_fixture"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "partial_ref_fixture.duckdb"\n'
        ),
        "models/stg_orders.sql": (
            "MODEL (description 'Test model stg_orders.');\n\n"
            "SELECT CAST(1 AS INTEGER) AS order_id, CAST(NULL AS VARCHAR) AS status\n"
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.");\n\n'
            'SELECT order_id, status FROM __ref("stg_orders")\n'
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id),\n"
            "__expected__orders AS (\n"
            "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_open_schema_ref_fixture_project_files() -> dict[str, str]:
    """Build a fixture with a required column beyond a partial star-model schema."""

    return {
        "sqlbuild_project.toml": (
            'name = "open_schema_ref_fixture"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "open_schema_ref_fixture.duckdb"\n'
        ),
        "models/stg_orders.sql": (
            "MODEL (description 'Test model stg_orders.',\n"
            "  columns (\n"
            "    order_id (type INTEGER),\n"
            "  ),\n"
            ");\n\n"
            'SELECT * FROM __source("raw_orders")\n'
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.");\n\n'
            'SELECT order_id, status FROM __ref("stg_orders")\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status),\n"
            "__expected__orders AS (SELECT 1 AS order_id, 'open' AS status)\n"
            "SELECT 1\n"
        ),
    }


def build_recursive_ref_fixture_project_files() -> dict[str, str]:
    """Build a fixture for a recursive model with derived output columns."""

    return {
        "sqlbuild_project.toml": (
            'name = "recursive_ref_fixture"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "recursive_ref_fixture.duckdb"\n'
        ),
        "models/stg_order_links.sql": (
            "MODEL (description 'Test model stg_order_links.',\n"
            "  contract enforced,\n"
            "  columns (\n"
            "    order_id (type INTEGER),\n"
            "    parent_order_id (type INTEGER),\n"
            "  ),\n"
            ");\n\n"
            'SELECT order_id, parent_order_id FROM __source("raw_order_links")\n'
        ),
        "models/order_roots.sql": (
            "MODEL (description 'Test model order_roots.',\n"
            "  contract enforced,\n"
            "  columns (\n"
            "    order_id (type INTEGER),\n"
            "    root_order_id (type INTEGER),\n"
            "    link_depth (type INTEGER),\n"
            "  ),\n"
            ");\n\n"
            "WITH RECURSIVE links AS (\n"
            '  SELECT * FROM __ref("stg_order_links")\n'
            "), order_chain AS (\n"
            "  SELECT order_id, order_id AS root_order_id, 0 AS link_depth\n"
            "  FROM links\n"
            "  WHERE parent_order_id IS NULL\n"
            "  UNION ALL\n"
            "  SELECT child.order_id, parent.root_order_id, parent.link_depth + 1 AS link_depth\n"
            "  FROM links AS child\n"
            "  INNER JOIN order_chain AS parent ON child.parent_order_id = parent.order_id\n"
            ")\n"
            "SELECT order_id, root_order_id, link_depth FROM order_chain\n"
        ),
        "sources/raw_order_links.yml": (
            "sources:\n"
            "  - name: raw_order_links\n    description: Test source raw_order_links.\n"
            "    schema: main\n"
            "    table: raw_order_links\n"
            "    columns:\n"
            "      - name: order_id\n        type: INTEGER\n"
            "      - name: parent_order_id\n        type: INTEGER\n"
        ),
        "tests/unit/test_order_roots.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__ref__stg_order_links AS (\n"
            "  SELECT 1 AS order_id, CAST(NULL AS INTEGER) AS parent_order_id\n"
            "),\n"
            "__expected__order_roots AS (\n"
            "  SELECT 1 AS order_id, 1 AS root_order_id, 0 AS link_depth\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_partial_seed_fixture_project_files() -> dict[str, str]:
    """Build a seed fixture with one required nullable column omitted."""

    return {
        "sqlbuild_project.toml": (
            'name = "partial_seed_fixture"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "partial_seed_fixture.duckdb"\n'
        ),
        "models/countries.sql": (
            'MODEL (description "Test model countries.");\n\n'
            'SELECT country_code, country_name FROM __seed("country_codes")\n'
        ),
        "seeds/country_codes.csv": "country_code,country_name\nUS,United States\n",
        "seeds/schema.yml": (
            "seeds:\n"
            "  - name: country_codes\n    description: Test seed country_codes.\n"
            "    columns:\n"
            "      - name: country_code\n        type: VARCHAR\n        nullable: false\n"
            "      - name: country_name\n        type: VARCHAR\n        nullable: true\n"
        ),
        "tests/unit/test_countries.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__seed__country_codes AS (SELECT 'US' AS country_code),\n"
            "__expected__countries AS (\n"
            "  SELECT 'US' AS country_code, CAST(NULL AS VARCHAR) AS country_name\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_irrelevant_omitted_column_project_files() -> dict[str, str]:
    """Build a fixture omitting a known nullable column outside the compiled closure."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["models/orders.sql"] = (
        'MODEL (description "Test model orders.");\n\nSELECT order_id FROM __source("raw_orders")\n'
    )
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (SELECT 1 AS order_id),\n"
        "__expected__orders AS (SELECT 1 AS order_id)\n"
        "SELECT 1\n"
    )
    return files


def build_cte_partial_source_fixture_project_files() -> dict[str, str]:
    """Build a partial source fixture read through an import CTE."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["models/orders.sql"] = (
        "MODEL (description 'Test model orders.');\n\n"
        "WITH staged AS (\n"
        '  SELECT order_id, status FROM __source("raw_orders")\n'
        ")\n"
        "SELECT order_id, status FROM staged\n"
    )
    return files


def build_clause_partial_source_fixture_project_files() -> dict[str, str]:
    """Build a partial source fixture whose omitted column is read only by a filter."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["models/orders.sql"] = (
        'MODEL (description "Test model orders.");\n\n'
        'SELECT order_id FROM __source("raw_orders") WHERE status IS NULL\n'
    )
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (SELECT 1 AS order_id),\n"
        "__expected__orders AS (SELECT 1 AS order_id)\n"
        "SELECT 1\n"
    )
    return files


def build_star_partial_fixture_project_files() -> dict[str, str]:
    """Build a contracted star model requiring the complete known source shape."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["sources/raw_orders.yml"] = files["sources/raw_orders.yml"].replace(
        "    columns:\n", "    contract: enforced\n    columns:\n"
    )
    files["models/orders.sql"] = (
        'MODEL (description "Test model orders.");\n\nSELECT * FROM __source("raw_orders")\n'
    )
    return files


def build_empty_partial_fixture_project_files() -> dict[str, str]:
    """Build a zero-row fixture that still needs a nullable typed column."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (SELECT 1 AS order_id WHERE FALSE),\n"
        "__expected__orders AS (\n"
        "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status WHERE FALSE\n"
        ")\n"
        "SELECT 1\n"
    )
    return files


def build_explicit_typed_null_fixture_project_files() -> dict[str, str]:
    """Build a complete fixture with an explicit typed null supplied by the author."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (\n"
        "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status\n"
        "),\n"
        "__expected__orders AS (\n"
        "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status\n"
        ")\n"
        "SELECT 1\n"
    )
    return files


def build_untyped_null_fixture_project_files() -> dict[str, str]:
    """Build a complete empty fixture whose bare null needs its contract type."""

    return {
        "sqlbuild_project.toml": (
            'name = "untyped_null_fixture"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "untyped_null_fixture.duckdb"\n'
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.',\n"
            "  contract enforced,\n"
            "  columns (order_year (type BIGINT)),\n"
            ");\n\n"
            "SELECT EXTRACT(YEAR FROM ordered_at) AS order_year\n"
            'FROM __source("raw_orders")\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    contract: enforced\n"
            "    columns:\n"
            "      - name: ordered_at\n        type: DATE\n        nullable: true\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT NULL AS ordered_at WHERE FALSE),\n"
            "__expected__orders AS (\n"
            "  SELECT NULL AS order_year WHERE FALSE\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_contract_empty_fixture_project_files() -> dict[str, str]:
    """Build empty input and expected fixtures whose shapes come from contracts."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["models/orders.sql"] = (
        "MODEL (description 'Test model orders.',\n"
        "  contract enforced,\n"
        "  columns (\n"
        "    order_id (type INTEGER),\n"
        "    status (type VARCHAR),\n"
        "  ),\n"
        ");\n\n"
        'SELECT order_id, status FROM __source("raw_orders")\n'
    )
    files["sources/raw_orders.yml"] = files["sources/raw_orders.yml"].replace(
        "    table: raw_orders\n",
        "    table: raw_orders\n    contract: enforced\n",
    )
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (SELECT * FROM __empty_fixture()),\n"
        "__expected__orders AS (SELECT * FROM __empty_fixture())\n"
        "SELECT 1\n"
    )
    return files


def build_open_source_empty_fixture_project_files() -> dict[str, str]:
    """Build an empty source fixture without an authoritative source shape."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (SELECT * FROM __empty_fixture()),\n"
        "__expected__orders AS (\n"
        "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status\n"
        ")\n"
        "SELECT 1\n"
    )
    return files


def build_open_expected_empty_fixture_project_files() -> dict[str, str]:
    """Build an empty expected fixture without an authoritative model shape."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["models/orders.sql"] = (
        'MODEL (description "Test model orders.");\n\nSELECT * FROM __source("raw_orders")\n'
    )
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (\n"
        "  SELECT 1 AS order_id, CAST(NULL AS VARCHAR) AS status\n"
        "),\n"
        "__expected__orders AS (SELECT * FROM __empty_fixture())\n"
        "SELECT 1\n"
    )
    return files


def build_qualified_star_other_relation_project_files() -> dict[str, str]:
    """Build a qualified star that does not target the only mocked relation."""

    return {
        "sqlbuild_project.toml": (
            'name = "qualified_star_other_relation"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "qualified_star_other_relation.duckdb"\n'
        ),
        "models/stg_orders.sql": "MODEL (description 'Test model stg_orders.');\n\n"
        "SELECT 1 AS stg_id\n",
        "models/orders.sql": (
            "MODEL (description 'Test model orders.');\n\n"
            "SELECT stg.*, raw.status\n"
            'FROM __ref("stg_orders") stg\n'
            'JOIN __source("raw_orders") raw ON TRUE\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    contract: enforced\n"
            "    columns:\n"
            "      - name: order_id\n        type: INTEGER\n        nullable: false\n"
            "      - name: status\n        type: VARCHAR\n        nullable: false\n"
            "      - name: created_at\n        type: TIMESTAMP\n        nullable: false\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT 'open' AS status),\n"
            "__expected__orders AS (SELECT 1 AS stg_id, 'open' AS status)\n"
            "SELECT 1\n"
        ),
    }


def build_mixed_case_partial_fixture_project_files() -> dict[str, str]:
    """Build a completed fixture with an as-written mixed-case unquoted alias."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["models/orders.sql"] = (
        'MODEL (description "Test model orders.");\n\n'
        'SELECT status, note FROM __source("raw_orders")\n'
    )
    files["sources/raw_orders.yml"] = (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n"
        "    table: raw_orders\n"
        "    columns:\n"
        "      - name: status\n        type: VARCHAR\n        nullable: false\n"
        "      - name: note\n        type: VARCHAR\n        nullable: true\n"
    )
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (SELECT 'open' AS Status),\n"
        "__expected__orders AS (\n"
        "  SELECT 'open' AS status, CAST(NULL AS VARCHAR) AS note\n"
        ")\n"
        "SELECT 1\n"
    )
    return files


def build_upper_case_star_import_partial_fixture_project_files() -> dict[str, str]:
    """Build upper-case references to lower-case source columns through a star import CTE."""

    return {
        "sqlbuild_project.toml": (
            'name = "partial_source_fixture"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "partial_source_fixture.duckdb"\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw__orders\n    description: Raw order records\n"
            "    schema: raw\n"
            "    table: orders\n"
            "    columns:\n"
            "      - name: order_id\n        type: VARCHAR\n"
            "      - name: customer_id\n        type: VARCHAR\n"
            "      - name: status\n        type: VARCHAR\n"
        ),
        "models/orders.sql": (
            'MODEL (description "Orders with their status", materialized table);\n\n'
            "WITH raw__orders AS (\n"
            "  SELECT *\n"
            '  FROM __source("raw__orders")\n'
            "),\n\n"
            "orders AS (\n"
            "  SELECT\n"
            "    ORDER_ID,\n"
            "    STATUS\n"
            "  FROM raw__orders\n"
            ")\n\n"
            "SELECT\n"
            "  ORDER_ID AS order_id,\n"
            "  STATUS AS status\n"
            "FROM orders\n"
        ),
        "tests/unit/test_orders.sql": (
            'TEST (name "orders__keeps_status");\n\n'
            "WITH __source__raw__orders AS (\n"
            "  SELECT CAST(column1 AS VARCHAR) AS order_id\n"
            "  FROM VALUES ('o1') AS t(column1)\n"
            "),\n\n"
            "__expected__orders AS (\n"
            "  SELECT CAST(column1 AS VARCHAR) AS order_id\n"
            "  FROM VALUES ('o1') AS t(column1)\n"
            ")\n\n"
            "SELECT 1\n"
        ),
    }


def build_invalid_partial_fixture_project_files(
    *, status_column_attributes: str, supplied_column: str = "order_id"
) -> dict[str, str]:
    """Build one partial source fixture that cannot be completed safely."""

    files: dict[str, str] = build_partial_source_fixture_project_files()
    files["sources/raw_orders.yml"] = (
        "sources:\n"
        "  - name: raw_orders\n    description: Test source raw_orders.\n"
        "    schema: main\n"
        "    table: raw_orders\n"
        "    contract: enforced\n"
        "    columns:\n"
        "      - name: order_id\n        type: INTEGER\n        nullable: false\n"
        "      - name: status\n"
        f"{status_column_attributes}"
    )
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        f"__source__raw_orders AS (SELECT 1 AS {supplied_column}),\n"
        "__expected__orders AS (SELECT 1 AS order_id, 'open' AS status)\n"
        "SELECT 1\n"
    )
    return files


def build_incompatible_fixture_type_project_files(*, adapter_name: str) -> dict[str, str]:
    """Build a test whose expected scalar conflicts with an array model column."""

    array_expression: str = {
        "duckdb": "[1, 2]",
        "snowflake": "ARRAY_CONSTRUCT(1, 2)",
    }[adapter_name]
    return {
        "sqlbuild_project.toml": (
            'name = "incompatible_fixture_type"\n'
            f'adapter = "{adapter_name}"\n\n'
            "[connection]\n"
            'database = "incompatible_fixture_type.duckdb"\n'
            'schema = "main"\n'
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.", database "fixture_db", schema "main");\n\n'
            'SELECT item_ids FROM __source("raw_orders")\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: item_ids\n        type: ARRAY\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            f"__source__raw_orders AS (SELECT {array_expression} AS item_ids),\n"
            "__expected__orders AS (SELECT '1,2' AS item_ids)\n"
            "SELECT 1\n"
        ),
    }


def build_complex_values_fixture_project_files(*, adapter_name: str) -> dict[str, str]:
    """Build a minimal complex-string VALUES fixture for one adapter dialect."""

    return {
        "sqlbuild_project.toml": (
            'name = "complex_values_fixture"\n'
            f'adapter = "{adapter_name}"\n\n'
            "[connection]\n"
            'database = "complex_values_fixture.duckdb"\n'
            'schema = "main"\n\n'
            "[[rules.rule_ignores]]\n"
            'rules = ["SQLCLEAN039"]\n'
            'paths = ["tests/unit/test_orders.sql"]\n'
            'reason = "The fixtures cover complex strings in a VALUES relation on every dialect."\n'
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.", database "fixture_db", schema "main");\n\n'
            'SELECT order_id, mapping_text FROM __source("raw_orders")\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: order_id\n        type: INTEGER\n"
            "      - name: mapping_text\n        type: VARCHAR\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT order_id, mapping_text FROM (VALUES\n"
            "    (1, 'alpha,beta [one] (two)'),\n"
            "    (2, 'gamma(delta),[epsilon]')\n"
            ") AS fixture(order_id, mapping_text)\n"
            "),\n"
            "__expected__orders AS (\n"
            "  SELECT order_id, mapping_text FROM (VALUES\n"
            "    (1, 'alpha,beta [one] (two)'),\n"
            "    (2, 'gamma(delta),[epsilon]')\n"
            ") AS fixture(order_id, mapping_text)\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_star_mock_fixture_project_files() -> dict[str, str]:
    """Build a valid source fixture whose columns come from a star projection."""

    files: dict[str, str] = build_missing_mock_columns_project_files()
    files["tests/unit/test_orders.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (\n"
        "  SELECT * FROM (VALUES (100, 'open')) AS fixture(customer_id, status)\n"
        "),\n"
        "__expected__orders AS (SELECT 100 AS customer_id, 'open' AS status)\n"
        "SELECT 1\n"
    )
    return files


def build_union_distinct_expected_project_files() -> dict[str, str]:
    """Build a SQL test whose expected rows combine branches with UNION DISTINCT."""

    return {
        "sqlbuild_project.toml": (
            'name = "union_distinct_expected"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "union_distinct_expected.duckdb"\n'
        ),
        "models/orders.sql": (
            'MODEL (description "Test model orders.");\n\n'
            'SELECT DISTINCT order_id FROM __source("raw_orders")\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: order_id\n        type: INTEGER\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS order_id UNION ALL SELECT 1 AS order_id UNION ALL SELECT 2 AS order_id\n"
            "),\n"
            "__expected__orders AS (\n"
            "  SELECT 1 AS order_id\n"
            "  UNION DISTINCT\n"
            "  SELECT 2 AS order_id\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_transformed_collection_project_files() -> dict[str, str]:
    """Build a valid aggregation that changes a scalar input into a collection output."""

    return {
        "sqlbuild_project.toml": (
            'name = "transformed_collection"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "transformed_collection.duckdb"\n'
        ),
        "models/order_statuses.sql": (
            'MODEL (description "Test model order_statuses.");\n\n'
            'SELECT list(status) AS statuses FROM __source("raw_orders")\n'
        ),
        "sources/raw_orders.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n"
            "    table: raw_orders\n"
            "    columns:\n"
            "      - name: status\n        type: VARCHAR\n"
        ),
        "tests/unit/test_order_statuses.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT 'open' AS status),\n"
            "__expected__order_statuses AS (SELECT ['open'] AS statuses)\n"
            "SELECT 1\n"
        ),
    }


def build_cte_derived_output_fixture_project_files() -> dict[str, str]:
    """Build a fixture where CTE outputs are derived from fewer source columns."""

    return {
        "sqlbuild_project.toml": (
            'name = "cte_derived_output_fixture"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "cte_derived_output_fixture.duckdb"\n'
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.');\n\n"
            "WITH raw_orders AS (\n"
            '  SELECT * FROM __source("raw_orders")\n'
            "), derived AS (\n"
            "  SELECT\n"
            "    order_id,\n"
            "    UPPER(payload) AS normalized_status,\n"
            "    CURRENT_TIMESTAMP AS loaded_at\n"
            "  FROM raw_orders\n"
            "), final AS (\n"
            "  SELECT order_id, normalized_status, loaded_at FROM derived\n"
            ")\n"
            "SELECT order_id, normalized_status, loaded_at FROM final\n"
        ),
        "sources/raw_orders.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "tests/unit/test_orders.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__source__raw_orders AS (SELECT 1 AS order_id, 'ready' AS payload),\n"
            "__assert__orders AS (\n"
            "  SELECT 1 FROM __ref(\"orders\") WHERE normalized_status <> 'READY'\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_multiple_invalid_fixtures_project_files() -> dict[str, str]:
    """Build two invalid tests so planning must aggregate both diagnostics."""

    files: dict[str, str] = build_missing_mock_columns_project_files()
    del files["tests/unit/test_orders.sql"]
    files["tests/unit/test_orders_a.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (SELECT 'open' AS status),\n"
        "__expected__orders AS (SELECT 100 AS customer_id, 'open' AS status)\n"
        "SELECT 1\n"
    )
    files["tests/unit/test_orders_b.sql"] = (
        "TEST();\n\n"
        "WITH\n"
        "__source__raw_orders AS (SELECT 100 AS customer_id),\n"
        "__expected__orders AS (SELECT 100 AS customer_id, 'open' AS status)\n"
        "SELECT 1\n"
    )
    return files


def build_assertion_test_project_files(*, failing: bool) -> dict[str, str]:
    """Build an inline project with a SQL unit-test zero-row assertion."""

    amount: int = {False: 10, True: -10}[failing]
    return {
        "sqlbuild_project.toml": (
            'name = "assertion_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "assertion_demo.duckdb"\n\n'
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.', materialized table);\n\n"
            "SELECT\n"
            "  id AS order_id,\n"
            "  amount\n"
            'FROM __source("raw_orders")\n'
        ),
        "tests/unit/orders_assert.sql": (
            "TEST();\n\n"
            "WITH\n"
            f"__source__raw_orders AS (SELECT 1 AS id, {amount} AS amount),\n"
            "__assert__no_negative_orders AS (\n"
            '  SELECT * FROM __ref("orders") WHERE amount < 0\n'
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_macro_test_project_files() -> dict[str, str]:
    """Build an inline project with a macro unit test guarding one model."""

    return {
        "sqlbuild_project.toml": (
            'name = "macro_test_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "macro_test_demo.duckdb"\n\n'
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        "macros/python/status.py": (
            'def normalize_status(value: str) -> str:\n    return f"LOWER(TRIM({value}))"\n'
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.', "
            "materialized table);\n\nSELECT @py.normalize_status(\"'  PAID  '\") AS status\n"
        ),
        "tests/unit/test_normalize_status.sql": (
            'TEST (mode macro, name "normalizes_status");\n\n'
            "WITH\n"
            "input_values AS (SELECT '  PAID  ' AS raw_status),\n"
            "__macro_actual__ AS (\n"
            '  SELECT @py.normalize_status("raw_status") AS status FROM input_values\n'
            "),\n"
            "__macro_expected__ AS (SELECT 'paid' AS status)\n"
            "SELECT 1\n"
        ),
    }


def build_diamond_chain_test_project_files(*, sql_analysis_enabled: bool) -> dict[str, str]:
    """Build a shared (diamond) upstream graph with one passing and one failing chain test."""

    sql_analysis_value: str = {False: "false", True: "true"}[sql_analysis_enabled]
    return {
        "sqlbuild_project.toml": (
            'name = "diamond_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "diamond_demo.duckdb"\n\n'
            "[settings]\n"
            f"sql_analysis = {sql_analysis_value}\n\n"
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "models/stg_orders.sql": (
            "MODEL (description 'Test model stg_orders.', materialized table);\n\n"
            'SELECT id AS order_id, customer_id, amount FROM __source("raw_orders")\n'
        ),
        "models/large_orders.sql": (
            "MODEL (description 'Test model large_orders.', materialized table);\n\n"
            'SELECT order_id, customer_id, amount FROM __ref("stg_orders") WHERE amount >= 10\n'
        ),
        "models/small_orders.sql": (
            "MODEL (description 'Test model small_orders.', materialized table);\n\n"
            "WITH picked AS (\n"
            '  SELECT order_id, customer_id, amount FROM __ref("stg_orders") WHERE amount < 10\n'
            ")\n"
            "SELECT * FROM picked -- small orders only\n"
        ),
        "models/customer_order_mix.sql": (
            "MODEL (description 'Test model customer_order_mix.', materialized table);\n\n"
            'WITH customers AS (SELECT DISTINCT customer_id FROM __ref("stg_orders"))\n'
            "SELECT customer_id,\n"
            '  (SELECT COUNT(*) FROM __ref("large_orders") AS l\n'
            "    WHERE l.customer_id = c.customer_id) AS large_count,\n"
            '  (SELECT COUNT(*) FROM __ref("small_orders") AS s\n'
            "    WHERE s.customer_id = c.customer_id) AS small_count\n"
            "FROM customers AS c\n"
        ),
        "tests/unit/test_customer_order_mix.sql": (
            'TEST (name "customer_order_mix_matches");\n\n'
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS id, 1 AS customer_id, 5 AS amount\n"
            "  UNION ALL SELECT 2 AS id, 1 AS customer_id, 20 AS amount\n"
            "  UNION ALL SELECT 3 AS id, 2 AS customer_id, 30 AS amount\n"
            "),\n"
            "__expected__customer_order_mix AS (\n"
            "  SELECT 1 AS customer_id, 1 AS large_count, 1 AS small_count\n"
            "  UNION ALL SELECT 2 AS customer_id, 1 AS large_count, 0 AS small_count\n"
            "),\n"
            "__assert__no_negative_large_orders AS (\n"
            '  SELECT * FROM __ref("large_orders") WHERE amount < 0\n'
            ")\n"
            "SELECT 1\n"
        ),
        "tests/unit/test_customer_order_mix_wrong.sql": (
            'TEST (name "customer_order_mix_wrong");\n\n'
            "WITH\n"
            "__source__raw_orders AS (SELECT 1 AS id, 1 AS customer_id, 5 AS amount),\n"
            "__expected__customer_order_mix AS (\n"
            "  SELECT 1 AS customer_id, 3 AS large_count, 1 AS small_count\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_deep_shared_missing_mock_project_files(*, layers: int) -> dict[str, str]:
    """Build a deep diamond graph whose test omits one source mock reached on every path."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "deep_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "deep_demo.duckdb"\n\n'
            "[settings]\n"
            f"unit_test_max_real_models = {2 * layers + 2}\n\n"
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
            "  - name: raw_customers\n    description: Test source raw_customers.\n"
            "    schema: main\n    table: raw_customers\n"
        ),
        "models/orders_00_left.sql": (
            "MODEL (description 'Test model orders_00_left.', materialized table);\n\n"
            "SELECT o.order_id, o.amount\n"
            'FROM __source("raw_orders") AS o\n'
            'JOIN __source("raw_customers") AS c ON c.customer_id = o.customer_id\n'
        ),
        "models/orders_00_right.sql": (
            'MODEL (description "Test model orders_00_right.", '
            'materialized table);\n\nSELECT order_id, amount FROM __ref("orders_00_left")\n'
        ),
    }
    for layer in range(1, layers + 1):
        previous_left: str = f"orders_{layer - 1:02d}_left"
        previous_right: str = f"orders_{layer - 1:02d}_right"
        for side in ("left", "right"):
            files[f"models/orders_{layer:02d}_{side}.sql"] = (
                "MODEL (description 'Test model.', materialized table);\n\n"
                "SELECT a.order_id, a.amount + b.amount AS amount\n"
                f'FROM __ref("{previous_left}") AS a\n'
                f'JOIN __ref("{previous_right}") AS b ON a.order_id = b.order_id\n'
            )
    files["tests/unit/test_deep_orders.sql"] = (
        'TEST (name "deep_orders_missing_mock");\n\n'
        "WITH\n"
        "__source__raw_customers AS (SELECT 1 AS customer_id),\n"
        f"__expected__orders_{layers:02d}_left AS (SELECT 1 AS order_id, 1 AS amount)\n"
        "SELECT 1\n"
    )
    return files


def build_expected_column_subset_project_files(
    *, expected_tests: dict[str, str], sql_analysis_enabled: bool = True
) -> dict[str, str]:
    """Build an orders model whose SQL tests list a chosen subset of its output columns."""

    sql_analysis_value: str = {False: "false", True: "true"}[sql_analysis_enabled]
    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "expected_columns_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "expected_columns_demo.duckdb"\n\n'
            "[settings]\n"
            f"sql_analysis = {sql_analysis_value}\n\n"
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.', materialized table);\n\n"
            "SELECT id AS order_id, customer_id, status, amount * 2 AS amount\n"
            'FROM __source("raw_orders")\n'
        ),
    }
    for test_name, expected_sql in expected_tests.items():
        files[f"tests/unit/{test_name}.sql"] = (
            f'TEST (name "{test_name}");\n\n'
            "WITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS id, 100 AS customer_id, 'paid' AS status, 5 AS amount\n"
            "  UNION ALL SELECT 2 AS id, 200 AS customer_id, 'open' AS status, 7 AS amount\n"
            "),\n"
            f"__expected__orders AS (\n  {expected_sql}\n)\n"
            "SELECT 1\n"
        )
    return files


_HELPER_SCOPE_SOURCE_MOCK: str = (
    "__source__raw_orders AS (SELECT 1 AS id, 10 AS amount UNION ALL SELECT 2 AS id, 20 AS amount)"
)
_HELPER_SCOPE_TESTS: dict[str, str] = {
    "helper_in_assert_passes": (
        f"{_HELPER_SCOPE_SOURCE_MOCK},\n"
        "expected_rows AS (SELECT 1 AS order_id, 20 AS amount UNION ALL SELECT 2, 40),\n"
        "__assert__all_expected_rows_built AS (\n"
        "  SELECT order_id, amount FROM expected_rows\n"
        '  EXCEPT SELECT order_id, amount FROM __ref("orders")\n'
        ")\n"
    ),
    "helper_in_assert_fails": (
        f"{_HELPER_SCOPE_SOURCE_MOCK},\n"
        "expected_rows AS (SELECT 1 AS order_id, 20 AS amount UNION ALL SELECT 3, 60),\n"
        "__assert__all_expected_rows_built AS (\n"
        "  SELECT order_id, amount FROM expected_rows\n"
        '  EXCEPT SELECT order_id, amount FROM __ref("orders")\n'
        ")\n"
    ),
    "helper_in_expected_passes": (
        f"{_HELPER_SCOPE_SOURCE_MOCK},\n"
        "expected_rows AS (SELECT 1 AS order_id, 20 AS amount UNION ALL SELECT 2, 40),\n"
        "__expected__orders AS (SELECT order_id, amount FROM expected_rows)\n"
    ),
    "helper_in_expected_fails": (
        f"{_HELPER_SCOPE_SOURCE_MOCK},\n"
        "expected_rows AS (SELECT 1 AS order_id, 20 AS amount UNION ALL SELECT 2, 41),\n"
        "__expected__orders AS (SELECT order_id, amount FROM expected_rows)\n"
    ),
    "mock_reads_helper_reading_mock": (
        "__source__raw_orders AS (SELECT 1 AS id, 10 AS amount UNION ALL SELECT 2, 20),\n"
        "base_rows AS (SELECT id AS order_id, amount FROM __source__raw_orders),\n"
        "__ref__stg_orders AS (SELECT order_id, amount FROM base_rows),\n"
        "expected_rows AS (SELECT order_id, amount * 2 AS amount FROM __ref__stg_orders),\n"
        "__expected__orders AS (SELECT order_id, amount FROM expected_rows),\n"
        "__assert__every_order_built AS (\n"
        "  SELECT order_id, amount FROM expected_rows\n"
        '  EXCEPT SELECT order_id, amount FROM __ref("orders")\n'
        ")\n"
    ),
    "helper_reads_mock_and_helper": (
        "__ref__stg_orders AS (SELECT 1 AS order_id, 10 AS amount UNION ALL SELECT 2, 20),\n"
        "doubled AS (SELECT order_id, amount * 2 AS amount FROM __ref__stg_orders),\n"
        "expected_rows AS (SELECT order_id, amount FROM doubled),\n"
        "__expected__orders AS (SELECT order_id, amount FROM expected_rows),\n"
        "__assert__every_order_doubled AS (\n"
        "  SELECT order_id, amount FROM expected_rows\n"
        '  EXCEPT SELECT order_id, amount FROM __ref("orders")\n'
        ")\n"
    ),
}


def build_helper_scope_project_files() -> dict[str, str]:
    """Build an orders chain whose SQL tests read helper CTEs from expected and assertion SQL."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "helper_scope_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "helper_scope_demo.duckdb"\n\n'
            "[defaults]\n"
            'materialized = "table"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "models/stg_orders.sql": (
            "MODEL (description 'Test model stg_orders.', materialized table);\n\n"
            'SELECT id AS order_id, amount FROM __source("raw_orders")\n'
        ),
        "models/orders.sql": (
            "MODEL (description 'Test model orders.', materialized table);\n\n"
            'SELECT order_id, amount * 2 AS amount FROM __ref("stg_orders")\n'
        ),
    }
    for test_name, ctes in _HELPER_SCOPE_TESTS.items():
        files[f"tests/unit/{test_name}.sql"] = (
            f'TEST (name "{test_name}");\n\nWITH\n{ctes}SELECT 1\n'
        )
    return files


_CURSOR_ORDER_ROWS: str = (
    "__source__raw_orders AS (\n"
    "  SELECT TIMESTAMP '1990-06-01' AS order_date, 1 AS amount\n"
    "  UNION ALL SELECT TIMESTAMP '2026-01-31' AS order_date, 2 AS amount\n"
    "  UNION ALL SELECT TIMESTAMP '2026-02-01' AS order_date, 3 AS amount\n"
    "  UNION ALL SELECT TIMESTAMP '2026-02-02' AS order_date, 4 AS amount\n"
    "  UNION ALL SELECT TIMESTAMP '2026-02-03' AS order_date, 5 AS amount\n"
    ")"
)
_CURSOR_EVENT_ROWS: str = (
    "__source__raw_events AS (\n"
    "  SELECT 5 AS event_id, 1 AS amount\n"
    "  UNION ALL SELECT 10 AS event_id, 2 AS amount\n"
    "  UNION ALL SELECT 20 AS event_id, 3 AS amount\n"
    ")"
)
_ALL_ORDER_DATES: str = (
    "SELECT TIMESTAMP '1990-06-01' AS order_date, 1 AS amount\n"
    "  UNION ALL SELECT TIMESTAMP '2026-01-31' AS order_date, 2 AS amount\n"
    "  UNION ALL SELECT TIMESTAMP '2026-02-01' AS order_date, 3 AS amount\n"
    "  UNION ALL SELECT TIMESTAMP '2026-02-02' AS order_date, 4 AS amount\n"
    "  UNION ALL SELECT TIMESTAMP '2026-02-03' AS order_date, 5 AS amount"
)
CURSOR_WINDOW_PASSING_TESTS: dict[str, str] = {
    "default_window_keeps_every_row": (
        'TEST (name "default_window_keeps_every_row");\n\nWITH\n'
        f"{_CURSOR_ORDER_ROWS},\n"
        f"__expected__daily_orders AS (\n  {_ALL_ORDER_DATES}\n)\n"
        "SELECT 1\n"
    ),
    "default_window_supports_date_arithmetic": (
        'TEST (name "default_window_supports_date_arithmetic");\n\nWITH\n'
        f"{_CURSOR_ORDER_ROWS},\n"
        "__expected__order_lookback AS (\n"
        "  SELECT DATE '1899-12-25' AS scan_start, DATE '3000-01-01' AS scan_end, 15 AS amount\n"
        ")\n"
        "SELECT 1\n"
    ),
    "declared_window_excludes_rows_outside": (
        'TEST (name "declared_window_excludes_rows_outside", '
        'cursor_start "2026-02-01", cursor_end "2026-02-03");\n\nWITH\n'
        f"{_CURSOR_ORDER_ROWS},\n"
        "__expected__daily_orders AS (\n"
        "  SELECT TIMESTAMP '2026-02-01' AS order_date, 3 AS amount\n"
        "  UNION ALL SELECT TIMESTAMP '2026-02-02' AS order_date, 4 AS amount\n"
        ")\n"
        "SELECT 1\n"
    ),
    "integer_default_window_keeps_every_row": (
        'TEST (name "integer_default_window_keeps_every_row");\n\nWITH\n'
        f"{_CURSOR_EVENT_ROWS},\n"
        "__expected__event_totals AS (\n"
        "  SELECT 5 AS event_id, 1 AS amount\n"
        "  UNION ALL SELECT 10 AS event_id, 2 AS amount\n"
        "  UNION ALL SELECT 20 AS event_id, 3 AS amount\n"
        ")\n"
        "SELECT 1\n"
    ),
    "integer_declared_window_excludes_rows_outside": (
        'TEST (name "integer_declared_window_excludes_rows_outside", '
        "cursor_start 10, cursor_end 20);\n\nWITH\n"
        f"{_CURSOR_EVENT_ROWS},\n"
        "__expected__event_totals AS (SELECT 10 AS event_id, 2 AS amount)\n"
        "SELECT 1\n"
    ),
    "microbatch_runs_one_pass": (
        'TEST (name "microbatch_runs_one_pass");\n\nWITH\n'
        f"{_CURSOR_ORDER_ROWS},\n"
        "__expected__hourly_orders AS (SELECT 5 AS order_count)\n"
        "SELECT 1\n"
    ),
}


def build_cursor_window_project_files(*, tests: dict[str, str]) -> dict[str, str]:
    """Build intrinsic-using timestamp, integer and microbatch models plus one plain model."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "cursor_window_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "cursor_window_demo.duckdb"\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
            "  - name: raw_events\n    description: Test source raw_events.\n"
            "    schema: main\n    table: raw_events\n"
        ),
        "models/daily_orders.sql": (
            "MODEL (description 'Test model daily_orders.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy delete_insert,\n"
            "  cursor order_date,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain day,\n"
            '  cursor_start "2026-01-01",\n'
            "  cursor_inputs (raw_orders order_date,),\n"
            ");\n\n"
            "SELECT order_date, amount\n"
            'FROM __source("raw_orders")\n'
            "WHERE order_date >= __cursor_start() AND order_date < __cursor_end()\n"
        ),
        "models/order_lookback.sql": (
            "MODEL (description 'Test model order_lookback.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy delete_insert,\n"
            "  cursor scan_start,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain day,\n"
            "  cursor_inputs (raw_orders order_date,),\n"
            ");\n\n"
            "SELECT\n"
            "  CAST(__cursor_start() AS DATE) - INTERVAL 7 DAY AS scan_start,\n"
            "  CAST(__cursor_end() + INTERVAL 1 DAY AS DATE) AS scan_end,\n"
            "  SUM(amount) AS amount\n"
            'FROM __source("raw_orders")\n'
            "WHERE order_date >= CAST(__cursor_start() AS DATE) - INTERVAL 7 DAY\n"
        ),
        "models/event_totals.sql": (
            "MODEL (description 'Test model event_totals.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy delete_insert,\n"
            "  cursor event_id,\n"
            "  cursor_type integer,\n"
            "  cursor_inputs (raw_events event_id,),\n"
            ");\n\n"
            "SELECT event_id, amount\n"
            'FROM __source("raw_events")\n'
            "WHERE event_id >= __cursor_start() AND event_id < __cursor_end()\n"
            "  AND event_id - 1 < __cursor_end() + 1\n"
        ),
        "models/plain_orders.sql": (
            'MODEL (description "Test model plain_orders.", '
            'materialized table);\n\nSELECT amount FROM __source("raw_orders")\n'
        ),
        "models/hourly_orders.sql": (
            "MODEL (description 'Test model hourly_orders.',\n"
            "  materialized incremental,\n"
            "  incremental_strategy delete_insert,\n"
            "  incremental_mode microbatch,\n"
            "  microbatch_strategy watermark,\n"
            "  cursor_watermark_mode all,\n"
            "  cursor order_hour,\n"
            "  cursor_type timestamp,\n"
            "  cursor_grain hour,\n"
            "  cursor_inputs (raw_orders (column order_date, roles [filter, watermark]),),\n"
            "  batch_size 1h,\n"
            ");\n\n"
            "SELECT MAX(DATE_TRUNC('hour', order_date)) AS order_hour, COUNT(*) AS order_count\n"
            'FROM __source("raw_orders")\n'
            "WHERE order_date >= __cursor_start() AND order_date < __cursor_end()\n"
        ),
    }
    for test_name, sql in tests.items():
        files[f"tests/unit/{test_name}.sql"] = sql
    return files


def build_declared_window_test(*, window: str, model_name: str) -> dict[str, str]:
    """Build one SQL test declaring a cursor window and expecting rows from one model."""

    return {
        "declared_window_case": (
            f'TEST (name "declared_window_case", {window});\n\nWITH\n'
            f"{_CURSOR_ORDER_ROWS},\n"
            f"{_CURSOR_EVENT_ROWS},\n"
            f"__expected__{model_name} AS (SELECT 1 AS amount)\n"
            "SELECT 1\n"
        )
    }


def build_cte_scope_project_files(*, queries: tuple[str, ...], expected: str) -> dict[str, str]:
    """Build a chain with deliberately overlapping model-local CTE names."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "cte_scope"\nadapter = "duckdb"\n[connection]\ndatabase = "orders.duckdb"\n'
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "tests/unit/chain.sql": (
            'TEST (name "order_chain");\nWITH '
            "__source__raw_orders AS (SELECT 1 AS order_id), "
            f"__expected__orders_{len(queries) - 1} AS ({expected}) SELECT 1"
        ),
    }
    for index, query in enumerate(queries):
        files[f"models/orders_{index}.sql"] = f"MODEL (description 'Test model.');\n{query}"
    return files


ORDER_LINES_MODEL_SQL: str = (
    'WITH base_rows AS (SELECT order_id, amount FROM __source("raw_orders")),\n'
    "final AS (SELECT order_id, amount * 2 AS line_total FROM base_rows)\n"
    "SELECT order_id, line_total FROM final"
)
ORDER_TOTALS_MODEL_SQL: str = (
    "WITH final AS (\n"
    "  SELECT order_id, line_total + 1 AS order_total, 'paid' AS status\n"
    '  FROM __ref("order_lines")\n'
    ")\n"
    "SELECT order_id, order_total, status FROM final"
)
ORDER_TOTALS_EXPECTED_ROWS_SQL: str = "SELECT 1 AS order_id, 21 AS order_total"


def build_shared_cte_name_chain_project_files(
    *, sql_analysis_enabled: bool, expected_order_lines_sql: str
) -> dict[str, str]:
    """Build two chained models that both end in `final`, plus helpers shared by fixtures."""

    sql_analysis_value: str = {False: "false", True: "true"}[sql_analysis_enabled]
    return {
        "sqlbuild_project.toml": (
            'name = "model_inlining"\nadapter = "duckdb"\n\n[connection]\n'
            'database = "model_inlining.duckdb"\n\n[settings]\n'
            f"sql_analysis = {sql_analysis_value}\n"
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "models/order_lines.sql": (
            f"MODEL (description 'Order lines.');\n\n{ORDER_LINES_MODEL_SQL}\n"
        ),
        "models/order_totals.sql": (
            f"MODEL (description 'Order totals.');\n\n{ORDER_TOTALS_MODEL_SQL}\n"
        ),
        "tests/unit/order_chain.sql": (
            'TEST (name "order_chain");\n\n'
            "WITH\n"
            "base_rows AS (SELECT 1 AS order_id, 10 AS amount),\n"
            "__source__raw_orders AS (SELECT order_id, amount FROM base_rows),\n"
            f"expected_rows AS ({ORDER_TOTALS_EXPECTED_ROWS_SQL}),\n"
            f"__expected__order_lines AS ({expected_order_lines_sql}),\n"
            "__expected__order_totals AS (\n"
            "  SELECT order_id, order_total, 'paid' AS status FROM expected_rows\n"
            "),\n"
            "__assert__totals_match AS (\n"
            '  SELECT order_id, order_total FROM __ref("order_totals")\n'
            "  EXCEPT SELECT order_id, order_total FROM expected_rows\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


_CTE_ENTRY_KEY: re.Pattern[str] = re.compile(r'\{"key": "((?:[^"\\]|\\.)*)", "value": \{"aliases"')


def rendered_cte_names(sql: str) -> tuple[set[str], set[str]]:
    """Return a rendered test query's top-level CTE names and every CTE name nested below them."""

    row: tuple[Any, ...] | None = duckdb.execute(
        "SELECT json_serialize_sql($sql)", {"sql": sql}
    ).fetchone()
    assert row is not None
    entries: list[dict[str, Any]] = json.loads(row[0])["statements"][0]["node"]["cte_map"]["map"]
    top_level: set[str] = {entry["key"].lower() for entry in entries}
    nested_json: str = json.dumps([entry["value"] for entry in entries])
    nested: set[str] = {name.lower() for name in _CTE_ENTRY_KEY.findall(nested_json)}
    return top_level, nested


def assert_test_query_names_isolated(sql: str, helper_names: tuple[str, ...]) -> None:
    """Assert no nested CTE reuses a top-level name and no helper keeps its name at top level."""

    top_level, nested = rendered_cte_names(sql)
    assert not top_level & nested, (top_level & nested, sql)
    assert not top_level & {name.lower() for name in helper_names}, sql


def build_lookup_project_files(*, sql_analysis_enabled: bool) -> dict[str, str]:
    """Build models reading a physical table and a model CTE named like a fixture-only helper."""

    sql_analysis_value: str = {False: "false", True: "true"}[sql_analysis_enabled]
    return {
        "sqlbuild_project.toml": (
            'name = "lookup"\nadapter = "duckdb"\n\n[connection]\n'
            'database = "lookup.duckdb"\n\n[settings]\n'
            f"sql_analysis = {sql_analysis_value}\n"
        ),
        "sources/raw.yml": (
            "sources:\n  - name: raw_orders\n    description: Test source raw_orders.\n"
            "    schema: main\n    table: raw_orders\n"
        ),
        "models/matched_orders.sql": (
            "MODEL (description 'Orders found in the lookup table.');\n\n"
            "WITH lookup_rows AS (SELECT order_id FROM main.lookup_rows)\n"
            'SELECT o.order_id FROM __source("raw_orders") AS o\n'
            "INNER JOIN lookup_rows AS l ON l.order_id = o.order_id\n"
        ),
        "models/listed_orders.sql": (
            "MODEL (description 'Orders listed in the physical lookup table.');\n\n"
            'SELECT o.order_id FROM __source("raw_orders") AS o\n'
            "INNER JOIN lookup_rows AS l ON l.order_id = o.order_id\n"
        ),
        "tests/unit/lookup.sql": (
            'TEST (name "lookup");\n\n'
            "WITH\n"
            "lookup_rows AS (SELECT 99 AS order_id),\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS order_id UNION ALL SELECT order_id FROM lookup_rows\n"
            "),\n"
            "__expected__matched_orders AS (SELECT 1 AS order_id),\n"
            "__expected__listed_orders AS (SELECT 1 AS order_id)\n"
            "SELECT 1\n"
        ),
    }


def prepare_optional_ceremony_project(*, tmp_path: Path, test_files: dict[str, str]) -> Path:
    """Write the optional-ceremony DuckDB project with the given SQL test files."""

    project_dir: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="optional_ceremony",
        repo_files={**_OPTIONAL_CEREMONY_PROJECT_FILES, **test_files},
    )
    execute_duckdb(
        db_path=project_dir / "optional_ceremony.duckdb",
        sql="CREATE TABLE raw_orders AS SELECT 1 AS order_id, 'paid' AS status",
    )
    return project_dir


_HELPER_REFERENCE_TESTS: dict[str, str] = {
    "helper_and_assertion_read_model": (
        "__ref__items AS (\n  SELECT 1 AS item_id, 10 AS amount\n),\n"
        "doubled AS (\n"
        '  SELECT item_id, amount_doubled FROM __ref("item_totals")\n'
        "),\n"
        "__assert__doubles_amount AS (\n"
        "  SELECT item_id FROM doubled WHERE amount_doubled <> 20\n"
        "),\n"
        "__assert__has_one_row AS (\n"
        '  SELECT COUNT(*) AS row_count FROM __ref("item_totals") HAVING COUNT(*) <> 1\n'
        ")\n"
    ),
    "only_helper_reads_model": (
        "__ref__items AS (\n  SELECT 1 AS item_id, 10 AS amount\n),\n"
        "doubled AS (\n"
        '  SELECT item_id, amount_doubled FROM __ref("item_totals")\n'
        "),\n"
        "__assert__doubles_amount AS (\n"
        "  SELECT item_id FROM doubled WHERE amount_doubled <> 20\n"
        ")\n"
    ),
    "helper_reads_helper_and_mock": (
        "__ref__items AS (\n  SELECT 1 AS item_id, 10 AS amount\n),\n"
        "joined AS (\n"
        "  SELECT d.item_id, d.amount_doubled, i.amount FROM doubled AS d\n"
        '  JOIN __ref("items") AS i USING (item_id)\n'
        "),\n"
        "doubled AS (\n"
        '  SELECT item_id, amount_doubled FROM __ref("item_totals")\n'
        "),\n"
        "__expected__item_totals AS (\n"
        "  SELECT item_id, amount_doubled FROM joined\n"
        "),\n"
        "__assert__doubles_mocked_amount AS (\n"
        "  SELECT item_id FROM joined WHERE amount_doubled <> amount * 2\n"
        ")\n"
    ),
    "mocked_model_read_by_helper_and_assertion": (
        "__ref__items AS (\n  SELECT item_id, 10 AS amount FROM base_ids\n),\n"
        "base_ids AS (SELECT 1 AS item_id),\n"
        'doubled AS (SELECT t.item_id, t.amount_doubled FROM __ref("item_totals") AS t),\n'
        "__assert__doubles_amount AS (\n"
        '  SELECT item_id FROM doubled JOIN __ref("items") USING (item_id)\n'
        "  WHERE amount_doubled <> 20\n"
        ")\n"
    ),
    "unread_helpers_and_mocks": (
        "__ref__items AS (\n  SELECT 1 AS item_id, 10 AS amount\n),\n"
        'archived_rows AS (SELECT item_id FROM __ref("item_archive")),\n'
        'other_rows AS (SELECT item_id, amount FROM __ref("item_extras")),\n'
        "__ref__item_extras AS (SELECT item_id, amount FROM other_rows),\n"
        "__expected__item_totals AS (\n  SELECT 1 AS item_id, 20 AS amount_doubled\n)\n"
    ),
    "nested_cte_shadows_helper": (
        "__ref__items AS (\n  SELECT 1 AS item_id, 10 AS amount\n),\n"
        "expected_rows AS (\n"
        "  WITH doubled AS (SELECT 1 AS item_id, 20 AS amount_doubled)\n"
        "  SELECT item_id, amount_doubled FROM doubled\n"
        "),\n"
        "doubled AS (SELECT item_id, amount_doubled FROM expected_rows),\n"
        "__expected__item_totals AS (\n  SELECT item_id, amount_doubled FROM doubled\n)\n"
    ),
    "helper_cycle": (
        "__ref__items AS (\n  SELECT 1 AS item_id, 10 AS amount\n),\n"
        "first_rows AS (SELECT item_id FROM second_rows),\n"
        "second_rows AS (SELECT item_id FROM first_rows),\n"
        "__assert__no_rows AS (\n"
        '  SELECT item_id FROM __ref("item_totals") JOIN first_rows USING (item_id)\n'
        ")\n"
    ),
    "helper_reads_unknown_model": (
        "__ref__items AS (\n  SELECT 1 AS item_id, 10 AS amount\n),\n"
        'missing_rows AS (SELECT item_id FROM __ref("item_archive")),\n'
        "__assert__no_rows AS (\n"
        '  SELECT item_id FROM __ref("item_totals") JOIN missing_rows USING (item_id)\n'
        ")\n"
    ),
    "mocked_model_read_by_expected_and_assertion": (
        "__ref__items AS (\n  SELECT 7 AS item_id, 5 AS amount\n),\n"
        "__expected__item_totals AS (\n"
        '  SELECT item_id, amount * 2 AS amount_doubled FROM __ref("items")\n'
        "),\n"
        "__assert__reads_mocked_items AS (\n"
        '  SELECT item_id FROM __ref("items") WHERE item_id <> 7 OR amount <> 5\n'
        ")\n"
    ),
    "expected_reads_unmocked_model": (
        "__ref__items AS (\n  SELECT 3 AS item_id, 4 AS amount\n),\n"
        "__expected__item_totals AS (\n"
        '  SELECT item_id, amount * 2 AS amount_doubled FROM __ref("item_extras")\n'
        ")\n"
    ),
    "assertion_reads_unmocked_source": (
        "__ref__items AS (\n  SELECT 1 AS item_id, 10 AS amount\n),\n"
        "__expected__item_totals AS (\n  SELECT 1 AS item_id, 20 AS amount_doubled\n),\n"
        "__assert__no_returns AS (\n"
        '  SELECT item_id FROM __source("item_returns")\n'
        ")\n"
    ),
    "helper_named_like_column_reads_unknown_model": (
        "__ref__items AS (\n  SELECT 7 AS item_id, 5 AS amount\n),\n"
        'amount_doubled AS (SELECT item_id FROM __ref("item_archive")),\n'
        "__expected__item_totals AS (\n  SELECT 7 AS item_id, 10 AS amount_doubled\n)\n"
    ),
    "helper_named_like_column_reads_model": (
        "__ref__items AS (\n  SELECT 7 AS item_id, 5 AS amount\n),\n"
        'amount_doubled AS (SELECT item_id FROM __ref("item_extras")),\n'
        "__expected__item_totals AS (\n  SELECT 7 AS item_id, 10 AS amount_doubled\n)\n"
    ),
    "assertion_reads_only_mocks": (
        "__ref__items AS (\n  SELECT 7 AS item_id, 5 AS amount\n),\n"
        "__assert__only_mocked_items AS (\n"
        '  SELECT item_id FROM __ref("items") WHERE item_id <> 7\n'
        ")\n"
    ),
    "expected_reads_unknown_model": (
        "__ref__items AS (\n  SELECT 1 AS item_id, 10 AS amount\n),\n"
        "__expected__item_totals AS (\n"
        '  SELECT item_id, amount_doubled FROM __ref("item_archive")\n'
        ")\n"
    ),
    "mock_called_reads_referencing_helper": (
        'extras AS (SELECT item_id, amount FROM __ref("item_extras")),\n'
        "mid AS (SELECT item_id, amount FROM extras),\n"
        "__ref__items AS (SELECT 7 AS item_id, 5 AS amount),\n"
        "__source__item_returns AS (SELECT item_id FROM mid),\n"
        "__expected__item_totals AS (SELECT 7 AS item_id, 10 AS amount_doubled),\n"
        "__assert__no_returns AS (\n"
        '  SELECT item_id FROM __source("item_returns") WHERE item_id <> 7\n'
        ")\n"
    ),
    "mock_called_reads_referencing_helper_directly": (
        'extras AS (SELECT item_id, amount FROM __ref("item_extras")),\n'
        "__ref__items AS (SELECT 7 AS item_id, 5 AS amount),\n"
        "__source__item_returns AS (SELECT item_id FROM extras),\n"
        "__expected__item_totals AS (SELECT 7 AS item_id, 10 AS amount_doubled),\n"
        "__assert__no_returns AS (\n"
        '  SELECT item_id FROM __source("item_returns") WHERE item_id <> 7\n'
        ")\n"
    ),
    "assertion_reads_mocked_source_and_model": (
        "__ref__items AS (\n  SELECT 7 AS item_id, 5 AS amount\n),\n"
        "__source__item_returns AS (SELECT 7 AS item_id),\n"
        "__assert__returned_items AS (\n"
        '  SELECT r.item_id FROM __source("item_returns") AS r\n'
        '  JOIN __ref("items") AS i USING (item_id) WHERE i.amount <> 5\n'
        ")\n"
    ),
    "assertion_reads_mock_by_name": (
        "__ref__items AS (\n  SELECT 7 AS item_id, 5 AS amount\n),\n"
        "__assert__only_mocked_items AS (\n"
        "  SELECT item_id FROM __ref__items WHERE item_id <> 7\n"
        ")\n"
    ),
    "mock_reads_referencing_helper": (
        "__ref__items AS (\n  SELECT item_id, 10 AS amount FROM base_rows\n),\n"
        'base_rows AS (SELECT item_id FROM __ref("item_totals")),\n'
        "__assert__no_rows AS (\n"
        '  SELECT item_id FROM __ref("item_totals") WHERE item_id IS NULL\n'
        ")\n"
    ),
}


def build_helper_reference_project_files(
    *, tests: tuple[str, ...], sql_analysis: bool = True
) -> dict[str, str]:
    """Build an items project whose SQL tests read models through helper CTEs."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": (
            'name = "helper_reference_demo"\n'
            'adapter = "duckdb"\n\n'
            "[connection]\n"
            'database = "helper_reference_demo.duckdb"\n\n'
            "[settings]\n"
            f"sql_analysis = {str(sql_analysis).lower()}\n\n"
            "[[rules.rule_ignores]]\n"
            'rules = ["SQLCLEAN005"]\n'
            'paths = ["tests/unit/**"]\n'
            'reason = "These SQL tests leave helpers unread to cover reference detection."\n'
        ),
        "sources/item_returns.yml": (
            "sources:\n"
            "  - name: item_returns\n"
            "    description: Returned items\n"
            "    expression: SELECT 1 AS item_id\n"
        ),
        "models/item_extras.sql": (
            "MODEL (description 'Extra item amounts', materialized table);\n\n"
            'SELECT item_id, amount FROM __ref("items")\n'
        ),
        "models/items.sql": (
            "MODEL (description 'Base items', materialized table);\n\n"
            "SELECT 1 AS item_id, 10 AS amount\n"
        ),
        "models/item_totals.sql": (
            "MODEL (description 'Doubled item amounts', materialized table);\n\n"
            'SELECT item_id, amount * 2 AS amount_doubled FROM __ref("items")\n'
        ),
    }
    for test_name in tests:
        files[f"tests/unit/test_{test_name}.sql"] = (
            f'TEST (name "{test_name}");\n\nWITH\n{_HELPER_REFERENCE_TESTS[test_name]}\nSELECT 1\n'
        )
    return files


COST_GUARD_PROJECT_TOML: str = """
name = "shop"
adapter = "duckdb"
default_target = "dev"

[connection]
database = "warehouse.duckdb"

[targets.dev]
schema = "main"
""".strip()
COST_GUARD_RAW_ORDERS_SOURCE: str = (
    "sources:\n"
    "  - name: raw_orders\n    description: Raw orders.\n"
    "    schema: main\n"
    "    table: raw_orders\n"
    "    columns:\n"
    "      - name: order_id\n        type: INTEGER\n"
    "      - name: amount\n        type: INTEGER\n"
)
SLOW_SCALAR_SQL: str = "(SELECT COUNT(*) FROM range(100000000000))"


def build_order_chain_files(*, chain_length: int) -> dict[str, str]:
    """A source-fed chain of `chain_length` order models and one unit test of the last one."""

    files: dict[str, str] = {
        "sqlbuild_project.toml": COST_GUARD_PROJECT_TOML,
        "sources/raw_orders.yml": COST_GUARD_RAW_ORDERS_SOURCE,
        "models/order_step_1.sql": (
            "MODEL (description 'Order step 1.', materialized table);\n\n"
            'SELECT order_id, amount FROM __source("raw_orders")\n'
        ),
    }
    for step in range(2, chain_length + 1):
        files[f"models/order_step_{step}.sql"] = (
            f"MODEL (description 'Order step {step}.', materialized table);\n\n"
            f'SELECT order_id, amount + 1 AS amount FROM __ref("order_step_{step - 1}")\n'
        )
    files["tests/unit/order_chain_totals.sql"] = (
        "TEST();\n\nWITH\n__source__raw_orders AS (\n  SELECT 1 AS order_id, 5 AS amount\n),\n"
        f"__expected__order_step_{chain_length} AS (\n"
        f"  SELECT 1 AS order_id, {4 + chain_length} AS amount\n)\n"
    )
    return files


def stored_test_outputs(*, database_path: Path) -> list[str]:
    """Persistent relations named like a stored unit-test output."""

    with duckdb.connect(str(database_path), read_only=True) as connection:
        rows: list[tuple[str]] = connection.execute(
            "SELECT table_name FROM information_schema.tables "
            "WHERE table_name LIKE '__sqb_test_output%'"
        ).fetchall()
    return [row[0] for row in rows]


def slow_model_files(*, max_duration: str) -> dict[str, str]:
    """A slow model with `max_duration`, a model reading it and an independent model."""

    return {
        "sqlbuild_project.toml": COST_GUARD_PROJECT_TOML,
        "models/order_scans.sql": (
            "MODEL (description 'Order scans.', materialized table, "
            f'max_duration "{max_duration}");\n\n'
            f"SELECT 1 AS order_id, {SLOW_SCALAR_SQL} AS scanned\n"
        ),
        "models/order_scan_summary.sql": (
            "MODEL (description 'Order scan summary.', materialized table);\n\n"
            'SELECT COUNT(*) AS orders FROM __ref("order_scans")\n'
        ),
        "models/customers.sql": (
            "MODEL (description 'Customers.', materialized table);\n\nSELECT 100 AS customer_id\n"
        ),
    }


SLOW_SWAP_ADAPTER: str = (
    "from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter\n\n\n"
    "class DuckDbSlowSwapAdapter(DuckDbAdapter):\n"
    "    adapter_name = 'duckdb_slow_swap'\n\n"
    "    def render_swap(self, *, left, right):\n"
    "        first, *rest = super().render_swap(left=left, right=right)\n"
    "        if 'order_totals' not in left:\n"
    "            return (first, *rest)\n"
    "        return (first, 'SELECT COUNT(*) FROM range(100000000000)', *rest)\n"
)


def slow_promotion_files(
    *, database: Path, max_duration: str, order_total: int, customer_id: int
) -> dict[str, str]:
    """A model whose promotion swap is slow on a rebuild, and an independent model after it."""

    return {
        "sqlbuild_project.toml": COST_GUARD_PROJECT_TOML.replace(
            'adapter = "duckdb"', 'adapter = "duckdb_slow_swap"'
        ).replace('database = "warehouse.duckdb"', f'database = "{database}"'),
        "adapters/duckdb_slow_swap.py": SLOW_SWAP_ADAPTER,
        "models/order_totals.sql": (
            "MODEL (description 'Order totals.', materialized table, "
            f'max_duration "{max_duration}");\n\n'
            f"SELECT 1 AS order_id, {order_total} AS total\n"
        ),
        "models/z_customers.sql": (
            "MODEL (description 'Customers.', materialized table);\n\n"
            f"SELECT {customer_id} AS customer_id\n"
        ),
    }


def order_totals_test_files(total_sql: str) -> dict[str, str]:
    """Return an order_totals model computing ``total_sql`` and a test expecting both models."""

    return {
        "models/order_totals.sql": (
            "MODEL (description 'Test model order_totals.');\n\n"
            f'SELECT order_id, {total_sql} AS total FROM __ref("orders")\n'
        ),
        "tests/unit/test_orders.sql": (
            'TEST (name "orders_ids");\n\nWITH\n'
            "__source__raw_orders AS (\n  SELECT 1 AS order_id, '7' AS code\n),\n"
            "__expected__orders AS (\n  SELECT 1 AS order_id\n),\n"
            "__expected__order_totals AS (\n  SELECT 1 AS order_id\n)\nSELECT 1\n"
        ),
    }


def build_shared_partial_fixture_project_files(
    *, enriched_column: str, report_column: str
) -> dict[str, str]:
    """Build a test chain whose two real models read one partial mock."""

    return {
        "sqlbuild_project.toml": (
            'name = "shared_partial_fixture"\nadapter = "duckdb"\n\n'
            '[connection]\ndatabase = "shared_partial_fixture.duckdb"\n'
        ),
        "models/order_lines.sql": (
            'MODEL (description "Test model order_lines.");\n\n'
            "SELECT 1 AS order_id, CAST(1.5 AS DOUBLE) AS amount, CAST(2.5 AS DOUBLE) AS discount\n"
        ),
        "models/enriched_lines.sql": (
            'MODEL (description "Test model enriched_lines.");\n\n'
            f'SELECT order_id, amount, {enriched_column} FROM __ref("order_lines")\n'
        ),
        "models/line_report.sql": (
            'MODEL (description "Test model line_report.");\n\n'
            f"SELECT e.order_id, e.amount, l.{report_column} AS line_discount\n"
            'FROM __ref("enriched_lines") AS e\n'
            'JOIN __ref("order_lines") AS l ON e.order_id = l.order_id\n'
        ),
        "tests/unit/test_line_report.sql": (
            "TEST();\n\n"
            "WITH\n"
            "__ref__order_lines AS (SELECT 1 AS order_id, CAST(1.5 AS DOUBLE) AS amount),\n"
            "__expected__line_report AS (\n"
            "  SELECT 1 AS order_id, CAST(1.5 AS DOUBLE) AS amount,"
            " CAST(NULL AS DOUBLE) AS line_discount\n"
            ")\n"
            "SELECT 1\n"
        ),
    }


def build_since_project_files() -> dict[str, str]:
    """Build a DuckDB orders project whose tests and scenarios execute different nodes."""

    return {
        "sqlbuild_project.toml": (
            'name = "since_orders"\nadapter = "duckdb"\n\n'
            '[connection]\ndatabase = "since_orders.duckdb"\n\n'
            '[defaults]\nmaterialized = "table"\n'
        ),
        ".gitignore": "target/\n*.duckdb\n*.duckdb.wal\n",
        "models/_macros/python/money.py": (
            "def cents_to_dollars(expression: str) -> str:\n"
            '    return f"CAST({expression} AS DOUBLE) / 100"\n'
        ),
        "sources/raw.yml": (
            "sources:\n"
            "  - name: raw_orders\n    description: Raw orders.\n"
            "    schema: main\n    table: raw_orders\n"
            "    columns:\n"
            "      - name: order_id\n        type: INTEGER\n"
            "      - name: status\n        type: VARCHAR\n"
            "      - name: amount_cents\n        type: INTEGER\n"
        ),
        "models/stg_orders.sql": (
            "MODEL (description 'Cleaned orders.');\n\n"
            'SELECT order_id, status, amount_cents FROM __source("raw_orders")\n'
        ),
        "models/order_totals.sql": (
            "MODEL (description 'Order totals in dollars.');\n\n"
            'SELECT order_id, @py.cents_to_dollars("amount_cents") AS amount\n'
            'FROM __ref("stg_orders")\n'
        ),
        "models/open_orders.sql": (
            "MODEL (description 'Open orders.');\n\n"
            "SELECT order_id FROM __ref(\"stg_orders\") WHERE status = 'open'\n"
        ),
        "tests/unit/test_stg_orders.sql": (
            "TEST ();\n\nWITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents\n"
            "),\n"
            "__expected__stg_orders AS (SELECT 1 AS order_id, 'open' AS status)\nSELECT 1\n"
        ),
        "tests/unit/test_order_totals.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__expected__order_totals AS (SELECT 1 AS order_id, 2.5 AS amount)\nSELECT 1\n"
        ),
        "tests/unit/test_open_orders.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__expected__open_orders AS (SELECT 1 AS order_id)\nSELECT 1\n"
        ),
        "tests/scenarios/order_totals_end_to_end.sql": (
            "SCENARIO (description 'Order totals from raw orders');\n\nWITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents\n"
            "),\n"
            "__expected__order_totals AS (SELECT 1 AS order_id, 2.5 AS amount)\nSELECT 1\n"
        ),
        "tests/scenarios/open_orders_from_fixture.sql": (
            "SCENARIO (description 'Open orders from a staged fixture');\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__expected__open_orders AS (SELECT 1 AS order_id)\nSELECT 1\n"
        ),
    }


def build_since_sql_macro_files() -> dict[str, str]:
    """Build a model whose scoped SQL macro calls another scoped SQL macro, and its test."""

    return {
        "models/_macros/sql/round_cents.sql": (
            "MACRO (\n  description 'Cents rounded to whole dollars.',\n"
            "  arguments (cents INTEGER),\n  returns DOUBLE,\n);\n\n"
            "ROUND(CAST(@cents AS DOUBLE) / 100, 0)\n"
        ),
        "models/_macros/sql/net_dollars.sql": (
            "MACRO (\n  description 'Net dollars after a fixed fee.',\n"
            "  arguments (cents INTEGER),\n  returns DOUBLE,\n);\n\n"
            "@sql.round_cents(@cents) - 1\n"
        ),
        "models/order_net.sql": (
            "MODEL (description 'Net order amounts.');\n\n"
            "SELECT order_id, @sql.net_dollars(amount_cents) AS net\n"
            'FROM __ref("stg_orders")\n'
        ),
        "tests/unit/test_order_net.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__expected__order_net AS (SELECT 1 AS order_id, 2.0 AS net)\nSELECT 1\n"
        ),
    }


def build_since_indirect_files() -> dict[str, str]:
    """Build files that reach tests indirectly: variables, defaults, helpers, seeds, functions."""

    base: dict[str, str] = build_since_project_files()
    return {
        "sqlbuild_project.toml": (
            base["sqlbuild_project.toml"]
            + '\n[vars]\nmin_amount = "100"\n\n[scopes]\nenforce_placement = false\n'
        ),
        "python/_rates.py": 'DIVISOR = "100"\n',
        "models/_macros/python/currency.py": (
            'def whole_cents(expression: str) -> str:\n    return f"({expression} * 1)"\n'
        ),
        "models/_macros/python/money.py": (
            "from models._macros.python.currency import whole_cents\n"
            "from python._rates import DIVISOR\n\n\n"
            "def cents_to_dollars(expression: str) -> str:\n"
            '    return f"CAST({whole_cents(expression)} AS DOUBLE) / {DIVISOR}"\n'
        ),
        "macros/python/taxes.py": (
            'def with_tax(expression: str) -> str:\n    return f"({expression} + 0)"\n'
        ),
        "macros/python/pricing.py": (
            "from macros.python.taxes import with_tax\n\n\n"
            "def gross(expression: str) -> str:\n    return with_tax(expression)\n"
        ),
        "macros/python/fixture_values.py": "def suffix() -> str:\n    return \"''\"\n",
        "models/stg_orders.sql": (
            "MODEL (\n  description 'Cleaned orders.',\n"
            "  columns (order_id (type INTEGER), status (type VARCHAR), "
            "amount_cents (type INTEGER)),\n);\n\n"
            'SELECT order_id, status, amount_cents FROM __source("raw_orders")\n'
        ),
        "models/order_gross.sql": (
            "MODEL (description 'Gross order amounts.');\n\n"
            'SELECT order_id, @py.gross("amount_cents") AS gross_cents FROM __ref("stg_orders")\n'
        ),
        "models/order_day.sql": (
            "MODEL (description 'The UTC day of a fixed timestamp.');\n\n"
            "SELECT order_id, CAST(TIMESTAMPTZ '2024-01-01 23:30:00+00' AS DATE) AS day\n"
            'FROM __ref("stg_orders")\n'
        ),
        "tests/unit/test_order_gross.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__expected__order_gross AS (SELECT 1 AS order_id, 250 AS gross_cents)\nSELECT 1\n"
        ),
        "tests/unit/test_order_day.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__expected__order_day AS (SELECT 1 AS order_id, DATE '2024-01-01' AS day)\nSELECT 1\n"
        ),
        "tests/unit/test_status_cases.sql": (
            'TEST (name "status_cases", parameters (status string), '
            'cases (open_case (status "open"), closed_case (status "closed")));\n\n'
            "WITH\n__source__raw_orders AS (\n"
            '  SELECT 1 AS order_id, @param("status") || @py.suffix() AS status, '
            "250 AS amount_cents\n),\n"
            "__expected__stg_orders AS (\n"
            '  SELECT 1 AS order_id, @param("status") || @py.suffix() AS status\n)\nSELECT 1\n'
        ),
        "audits/generic/at_least.sql": (
            'AUDIT ();\n\nSELECT *\nFROM __ref("@model")\nWHERE @column < @minimum\n'
        ),
        "models/order_totals.sql": (
            "MODEL (\n  description 'Order totals in dollars.',\n"
            "  audits [at_least (column amount, minimum 0)],\n);\n\n"
            'SELECT order_id, @py.cents_to_dollars("amount_cents") AS amount\n'
            'FROM __ref("stg_orders")\n'
        ),
        "models/big_orders.sql": (
            "MODEL (description 'Large orders.');\n\n"
            'SELECT order_id FROM __ref("stg_orders") WHERE amount_cents >= @@min_amount\n'
        ),
        "models/marts/order_counts.sql": (
            "MODEL (description 'Order counts.');\n\n"
            'SELECT COUNT(*) AS order_count FROM __ref("stg_orders")\n'
        ),
        "seeds/regions.csv": "region_id,region_name\n1,north\n",
        "seeds/regions.yml": (
            "seeds:\n  - name: regions\n    description: Sales regions.\n    columns:\n"
            "      - name: region_id\n        type: INTEGER\n"
            "      - name: region_name\n        type: VARCHAR\n"
        ),
        "functions/sql/is_open.sql": (
            "FUNCTION (\n  description 'Whether an order is open.',\n"
            "  arguments (order_status VARCHAR),\n  returns BOOLEAN,\n);\n\n"
            "order_status = 'open'\n"
        ),
        "models/regional_orders.sql": (
            "MODEL (description 'Orders with their region.');\n\n"
            "SELECT o.order_id, r.region_id\n"
            'FROM __ref("stg_orders") o CROSS JOIN __seed("regions") r\n'
        ),
        "models/order_flags.sql": (
            "MODEL (description 'Whether each order is open.');\n\n"
            'SELECT order_id, __udf("is_open")(status) AS is_open FROM __ref("stg_orders")\n'
        ),
        "models/order_archive.sql": (
            "MODEL (description 'Archived orders.');\n\n"
            'SELECT order_id FROM __ref("stg_orders")\n'
        ),
        "models/archive_counts.sql": (
            "MODEL (description 'Archived order count.');\n\n"
            'SELECT COUNT(*) AS archived FROM __ref("order_archive")\n'
        ),
        "tests/unit/test_archive_counts.sql": (
            "TEST ();\n\nWITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents\n"
            "),\n"
            "__expected__archive_counts AS (SELECT 1 AS archived)\nSELECT 1\n"
        ),
        "python/tasks/report.py": (
            "from sqlbuild.tasks import task\n\n\n"
            "@task()\ndef report_orders(ctx):\n"
            '    """Report orders."""\n\n'
            "    return ctx.result(payload={})\n"
        ),
        "tests/unit/test_big_orders.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__expected__big_orders AS (SELECT 1 AS order_id)\nSELECT 1\n"
        ),
        "tests/unit/test_order_counts.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__expected__order_counts AS (SELECT 1 AS order_count)\nSELECT 1\n"
        ),
        "tests/unit/test_regional_orders.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__seed__regions AS (SELECT 1 AS region_id, 'north' AS region_name),\n"
            "__expected__regional_orders AS (SELECT 1 AS order_id, 1 AS region_id)\nSELECT 1\n"
        ),
        "tests/unit/test_order_flags.sql": (
            "TEST ();\n\nWITH\n"
            "__ref__stg_orders AS (SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents),\n"
            "__expected__order_flags AS (SELECT 1 AS order_id, TRUE AS is_open)\nSELECT 1\n"
        ),
        "tests/scenarios/regional_orders_end_to_end.sql": (
            "SCENARIO (description 'Regional orders from raw orders and the region seed');\n\n"
            "WITH\n__source__raw_orders AS (\n"
            "  SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents\n"
            "),\n"
            "__expected__regional_orders AS (SELECT 1 AS order_id, 1 AS region_id)\nSELECT 1\n"
        ),
        "tests/unit/test_order_totals_from_source.sql": (
            "TEST ();\n\nWITH\n"
            "__source__raw_orders AS (\n"
            "  SELECT 1 AS order_id, 'open' AS status, 250 AS amount_cents\n"
            "),\n"
            "__expected__order_totals AS (SELECT 1 AS order_id, 2.5 AS amount)\nSELECT 1\n"
        ),
    }


_GIT_IDENTITY: dict[str, str] = {
    "GIT_AUTHOR_NAME": "Example",
    "GIT_AUTHOR_EMAIL": "dev@example.com",
    "GIT_COMMITTER_NAME": "Example",
    "GIT_COMMITTER_EMAIL": "dev@example.com",
}


def run_git(*, repository: Path, arguments: tuple[str, ...]) -> str:
    """Run git in `repository` with a fixed identity and return its stdout."""

    result: subprocess.CompletedProcess[str] = subprocess.run(
        ("git", "-c", "commit.gpgsign=false", *arguments),
        cwd=repository,
        env={**os.environ, **_GIT_IDENTITY},
        capture_output=True,
        text=True,
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr
    return result.stdout.strip()


def commit_project_on_main(*, repository: Path) -> None:
    """Commit everything on `main`, then continue on a `feature` branch."""

    run_git(repository=repository, arguments=("init", "--quiet", "--initial-branch", "main"))
    run_git(repository=repository, arguments=("add", "--all"))
    run_git(repository=repository, arguments=("commit", "--quiet", "-m", "orders"))
    run_git(repository=repository, arguments=("checkout", "--quiet", "-b", "feature"))


def write_project_files(*, project_dir: Path, files: Mapping[str, str]) -> None:
    """Write each file below `project_dir`, creating parent directories."""

    relative_path: str
    contents: str
    for relative_path, contents in files.items():
        path: Path = project_dir / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")


def delete_project_files(*, project_dir: Path, relative_paths: tuple[str, ...]) -> None:
    """Delete each file below `project_dir`."""

    relative_path: str
    for relative_path in relative_paths:
        (project_dir / relative_path).unlink()


def prepare_since_repository(*, tmp_path: Path, extra_files: Mapping[str, str]) -> Path:
    """Write the since project with `extra_files`, commit it on `main` and branch to `feature`."""

    project: Path = prepare_inline_project(
        tmp_path=tmp_path,
        project_name="since_orders",
        repo_files={**build_since_project_files(), **extra_files},
    )
    commit_project_on_main(repository=project)
    return project


_FAKE_SQLQUACK: str = """
import json
import sys
from pathlib import Path

arguments = sys.argv[1:]
if arguments == ["--version"]:
    print("sqlquack VERSION")
    sys.exit(0)
if arguments[0] == "function":
    name = arguments[arguments.index("--name") + 1]
    schema = json.loads(Path(arguments[arguments.index("--schema") + 1]).read_text())
    body = Path(arguments[-1]).read_text(encoding="utf-8")
    with Path(__file__).with_name("functions.jsonl").open("a", encoding="utf-8") as calls:
        calls.write(json.dumps({"name": name, "schema": schema, "body": body}) + "\\n")
    if "UNCONVERTIBLE_FN" in body.upper():
        print("sqlquack function: refused: no rule converts fn:UNCONVERTIBLE_FN", file=sys.stderr)
        sys.exit(1)
    parameters = [argument[0] for argument in schema["functions"][name]["arguments"]]
    divergences = {"declared_offset": ["node:cast"]} if "ZONED_AT" in body.upper() else {}
    print(json.dumps({
        "parameters": parameters, "body": body, "provenance": {"divergences": divergences},
    }))
    sys.exit(0)
with Path(__file__).with_name("calls.jsonl").open("a", encoding="utf-8") as calls:
    calls.write(json.dumps(arguments) + "\\n")
text = Path(arguments[-1]).read_text(encoding="utf-8")
with Path(__file__).with_name("statements.jsonl").open("a", encoding="utf-8") as calls:
    schema_path = arguments[arguments.index("--schema") + 1] if "--schema" in arguments else None
    schema = json.loads(Path(schema_path).read_text()) if schema_path else {"functions": {}}
    calls.write(json.dumps({"sql": text, "declared": sorted(schema["functions"])}) + "\\n")
sql = text.upper()
report = {"converted": True, "sql": text, "refusals": [], "assumptions": [], "notes": []}
if "UNCONVERTIBLE_FN" in sql:
    report["converted"] = False
    report["sql"] = ""
    report["refusals"] = [
        {
            "construct": "fn:UNCONVERTIBLE_FN",
            "category": "missing rule",
            "reason": "no rule converts fn:UNCONVERTIBLE_FN",
            "line": 3,
            "column": 9,
        }
    ]
elif "GUARDED_VALUE" in sql:
    report["sql"] = "SELECT CAST('json_fidelity#0123456789ab: a guarded value' AS INTEGER)"
elif "ZONED_AT" in sql and "--strict" in arguments:
    report["converted"] = False
    report["sql"] = ""
    report["refusals"] = [
        {
            "construct": "node:cast",
            "category": "outside a rule's type conditions",
            "reason": "node:cast relies on assumption session_timezone_utc",
        }
    ]
elif "ZONED_AT" in sql:
    report["assumptions"] = ["session_timezone_utc"]
mode = Path(__file__).with_name("mode").read_text(encoding="utf-8").strip()
exit_code = 0 if report["converted"] else 1
if mode == "empty":
    report["sql"] = "SELECT 0, 'customer_orders', 0, 0, 0, 0 WHERE FALSE"
elif mode == "short":
    report["sql"] = "SELECT 0, 'customer_orders', 2, 2"
elif mode == "fractional":
    report["sql"] = "SELECT 0, 'customer_orders', 2.5, 2, 0, 0"
elif mode == "wrong_step":
    report["sql"] = "SELECT 7, 'customer_orders', 2, 2, 0, 0"
elif mode == "nonzero_exit":
    exit_code = 3
elif mode == "string_false":
    report["converted"] = "false"
elif mode == "refusals_with_converted":
    report["refusals"] = [{"construct": "fn:ANY", "category": "missing rule", "reason": "stand-in"}]
elif mode == "assumes_anyway":
    report["assumptions"] = ["session_timezone_utc"]
elif mode == "slow":
    import time
    time.sleep(3)
elif mode == "invalid_json":
    report = "not a report"
print(json.dumps(report))
sys.exit(exit_code)
"""


def write_fake_sqlquack(*, directory: Path, version: str, mode: str = "faithful") -> Path:
    """A stand-in `sqlquack` logging conversions; a `mode` other than faithful reports badly."""

    directory.mkdir(parents=True, exist_ok=True)
    executable: Path = directory / "sqlquack"
    executable.write_text(
        f"#!{sys.executable}\n" + _FAKE_SQLQUACK.replace("VERSION", version), encoding="utf-8"
    )
    executable.chmod(0o755)
    (directory / "mode").write_text(mode, encoding="utf-8")
    for log in ("calls.jsonl", "functions.jsonl", "statements.jsonl"):
        (directory / log).write_text("", encoding="utf-8")
    return executable


def read_fake_sqlquack_conversions(*, directory: Path) -> tuple[tuple[str, ...], ...]:
    """Each argument list the stand-in `sqlquack` converted with, in order."""

    lines: list[str] = (directory / "calls.jsonl").read_text(encoding="utf-8").splitlines()
    return tuple(tuple(json.loads(line)) for line in lines)


def read_fake_sqlquack_log(*, directory: Path, log: str) -> tuple[dict[str, Any], ...]:
    """Each record the stand-in `sqlquack` wrote to `log`: functions or statements it converted."""

    lines: list[str] = (directory / f"{log}.jsonl").read_text(encoding="utf-8").splitlines()
    return tuple(json.loads(line) for line in lines)


_LOCAL_PROJECT_NAME: str = 'name = "duckshop_local"\nadapter = "snowflake"\n\n'
_LOCAL_CONNECTION: str = (
    "[connection]\n"
    'account = "example-no-login.invalid"\nuser = "NO_LOGIN"\npassword = "not-a-credential"\n'
    "session_parameters = { QUOTED_IDENTIFIERS_IGNORE_CASE = true }\n"
)
_LOCAL_DEFAULTS: str = '\n[defaults]\ndatabase = "ANALYTICS"\nschema = "PUBLIC"\n'
_LOCAL_ORDERS: str = (
    "__source__raw_orders AS (\n"
    "  SELECT 1 AS order_id, 10 AS customer_id, 'completed' AS status\n"
    "  UNION ALL SELECT 2, 10, 'completed'\n"
    "  UNION ALL SELECT 3, 11, 'cancelled'\n"
    "),\n"
)
_REAL_LOCAL_ORDERS: str = (
    "__source__raw_orders AS (\n"
    "  SELECT CAST(column1 AS NUMBER(38,0)) AS order_id,\n"
    "    CAST(column2 AS NUMBER(38,0)) AS customer_id,\n"
    "    CAST(column3 AS NUMBER(12,2)) AS amount,\n"
    "    CAST(column4 AS VARCHAR) AS status\n"
    "  FROM VALUES (1, 10, 5.00, 'completed'), (2, 10, 7.50, 'completed'),\n"
    "    (3, 11, 3.25, 'cancelled')\n"
    "),\n"
)


def _local_source(*, columns: tuple[tuple[str, str], ...]) -> str:
    listed: str = "".join(f"      - name: {name}\n        type: {kind}\n" for name, kind in columns)
    return (
        "sources:\n  - name: raw_orders\n    description: Raw orders.\n"
        "    database: raw\n    schema: shop\n    table: orders\n    columns:\n" + listed
    )


def _local_model(*, description: str, select: str) -> str:
    return f"MODEL (description '{description}');\n\n{select}\nFROM __source(\"raw_orders\")\n"


def _local_test(*, name: str, orders: str, model: str, expected: str) -> str:
    return (
        f'TEST (name "{name}");\n\nWITH\n{orders}'
        f"__expected__{model} AS (\n  {expected}\n)\nSELECT 1\n"
    )


def build_local_engine_project_files(*, connection: str) -> dict[str, str]:
    """A Snowflake DuckShop project whose tests pass, fail, refuse, guard and assume."""

    orders: str = _LOCAL_ORDERS
    return {
        "sqlbuild_project.toml": _LOCAL_PROJECT_NAME + connection + _LOCAL_DEFAULTS,
        "sqlquack.toml": "schema_version = 1\n",
        "sources/raw_orders.yml": _local_source(
            columns=(("order_id", "INTEGER"), ("customer_id", "INTEGER"), ("status", "VARCHAR"))
        ),
        "models/customer_orders.sql": _local_model(
            description="Completed orders per customer.",
            select="SELECT customer_id, COUNT(*) AS order_count",
        )
        + "WHERE status = 'completed'\nGROUP BY customer_id\n",
        "models/order_labels.sql": _local_model(
            description="Orders with a label.",
            select="SELECT order_id, UNCONVERTIBLE_FN(status) AS label",
        ),
        "models/order_guards.sql": _local_model(
            description="Orders with a guarded value.",
            select="SELECT order_id, 'guarded_value' AS guarded",
        ),
        "models/order_zones.sql": _local_model(
            description="Orders with a zoned marker.",
            select="SELECT order_id, 'zoned_at' AS zoned_at",
        ),
        "tests/unit/test_customer_orders.sql": _local_test(
            name="customer_orders_count_completed",
            orders=orders,
            model="customer_orders",
            expected="SELECT 10 AS customer_id, 2 AS order_count",
        ),
        "tests/unit/test_customer_orders_wrong.sql": _local_test(
            name="customer_orders_wrong_count",
            orders=orders,
            model="customer_orders",
            expected="SELECT 10 AS customer_id, 3 AS order_count",
        ),
        "tests/unit/test_order_labels.sql": _local_test(
            name="order_labels_are_refused",
            orders=orders,
            model="order_labels",
            expected="SELECT 1 AS order_id, 'x' AS label",
        ),
        "tests/unit/test_order_guards.sql": _local_test(
            name="order_guards_raise",
            orders=orders,
            model="order_guards",
            expected="SELECT 1 AS order_id, 'guarded_value' AS guarded",
        ),
        "tests/unit/test_order_zones.sql": _local_test(
            name="order_zones_rely_on_utc",
            orders=orders,
            model="order_zones",
            expected=(
                "SELECT 1 AS order_id, 'zoned_at' AS zoned_at "
                "UNION ALL SELECT 2 AS order_id, 'zoned_at' AS zoned_at "
                "UNION ALL SELECT 3 AS order_id, 'zoned_at' AS zoned_at"
            ),
        ),
    }


def _real_expected_totals(*, total: str) -> str:
    return (
        "SELECT CAST(column1 AS NUMBER(38,0)) AS customer_id,\n"
        "    CAST(column2 AS NUMBER(18,0)) AS order_count,\n"
        "    CAST(column3 AS NUMBER(38,2)) AS total_amount\n"
        f"  FROM VALUES (10, 2, {total})"
    )


def build_real_local_engine_project_files(*, policy: str) -> dict[str, str]:
    """A Snowflake DuckShop project for the real SQLQuack: one pass, one failure, one refusal."""

    orders: str = _REAL_LOCAL_ORDERS
    return {
        "sqlbuild_project.toml": (_LOCAL_PROJECT_NAME + _LOCAL_CONNECTION + _LOCAL_DEFAULTS),
        "sqlquack.toml": policy,
        "sources/raw_orders.yml": _local_source(
            columns=(
                ("order_id", "NUMBER(38,0)"),
                ("customer_id", "NUMBER(38,0)"),
                ("amount", "NUMBER(12,2)"),
                ("status", "VARCHAR"),
            )
        ),
        "models/customer_totals.sql": _local_model(
            description="Completed order totals per customer.",
            select="SELECT customer_id, COUNT(*) AS order_count, SUM(amount) AS total_amount",
        )
        + "WHERE status = 'completed'\nGROUP BY customer_id\n",
        "models/order_labels.sql": _local_model(
            description="Orders with an unconvertible label.",
            select="SELECT order_id, UNCONVERTIBLE_FN(status) AS label",
        ),
        "tests/unit/test_customer_totals.sql": _local_test(
            name="customer_totals_sum_completed",
            orders=orders,
            model="customer_totals",
            expected=_real_expected_totals(total="12.50"),
        ),
        "tests/unit/test_customer_totals_wrong.sql": _local_test(
            name="customer_totals_wrong_sum",
            orders=orders,
            model="customer_totals",
            expected=_real_expected_totals(total="99.00"),
        ),
        "tests/unit/test_order_labels.sql": _local_test(
            name="order_labels_are_refused",
            orders=orders,
            model="order_labels",
            expected=(
                "SELECT CAST(column1 AS NUMBER(38,0)) AS order_id, "
                "CAST(column2 AS VARCHAR) AS label FROM VALUES (1, 'x')"
            ),
        ),
    }


_FUNCTION_ORDERS: str = (
    "__source__raw_orders AS (\n"
    "  SELECT 1 AS order_id, 'completed' AS status, 'gift' AS tag\n"
    "  UNION ALL SELECT 2, 'cancelled', 'bath'\n"
    "),\n"
)


def _local_function(*, arguments: str, returns: str, body: str) -> str:
    return (
        f"FUNCTION (\n  description 'A test function.',\n  arguments ({arguments}),\n"
        f"  returns {returns},\n);\n\n{body}\n"
    )


def build_local_function_project_files() -> dict[str, str]:
    """A Snowflake project whose tests call SQL and Python project functions."""

    return {
        "sqlbuild_project.toml": _LOCAL_PROJECT_NAME + _LOCAL_CONNECTION + _LOCAL_DEFAULTS,
        "sqlquack.toml": "schema_version = 1\n",
        "sources/raw_orders.yml": _local_source(
            columns=(("order_id", "INTEGER"), ("status", "VARCHAR"), ("tag", "VARCHAR"))
        ),
        "functions/sql/is_completed.sql": _local_function(
            arguments="order_status VARCHAR", returns="BOOLEAN", body="order_status = 'completed'"
        ),
        "functions/sql/is_open.sql": _local_function(
            arguments="order_status VARCHAR",
            returns="BOOLEAN",
            body='NOT __udf("is_completed")(order_status)',
        ),
        "functions/sql/odd_label.sql": _local_function(
            arguments="order_status VARCHAR",
            returns="VARCHAR",
            body="UNCONVERTIBLE_FN(order_status)",
        ),
        "functions/python/tag_label.py": (
            "from sqlbuild.functions import udf\n\n\n"
            '@udf(arguments={"tag": "VARCHAR"}, returns="VARCHAR", runtime_version="3.11")\n'
            "def main(tag: str | None) -> str | None:\n"
            '    """A tag as a display label."""\n'
            "    return None if tag is None else tag.capitalize()\n"
        ),
        "models/order_flags.sql": _local_model(
            description="Orders with whether each is completed or open.",
            select=(
                'SELECT order_id, __udf("is_completed")(status) AS is_completed, '
                '__udf("is_open")(status) AS is_open'
            ),
        ),
        "models/order_odd_labels.sql": _local_model(
            description="Orders with a label no rule converts.",
            select='SELECT order_id, __udf("odd_label")(status) AS label',
        ),
        "models/order_tags.sql": _local_model(
            description="Orders with a tag label.",
            select='SELECT order_id, __udf("tag_label")(tag) AS label',
        ),
        "tests/unit/test_order_flags.sql": _local_test(
            name="order_flags_from_functions",
            orders=_FUNCTION_ORDERS,
            model="order_flags",
            expected=(
                "SELECT 1 AS order_id, TRUE AS is_completed, FALSE AS is_open "
                "UNION ALL SELECT 2 AS order_id, FALSE AS is_completed, TRUE AS is_open"
            ),
        ),
        "tests/unit/test_order_odd_labels.sql": _local_test(
            name="order_odd_labels_are_refused",
            orders=_FUNCTION_ORDERS,
            model="order_odd_labels",
            expected="SELECT 1 AS order_id, 'x' AS label",
        ),
        "tests/unit/test_order_tags.sql": _local_test(
            name="order_tags_are_refused",
            orders=_FUNCTION_ORDERS,
            model="order_tags",
            expected="SELECT 1 AS order_id, 'Gift' AS label",
        ),
    }


def build_real_local_function_project_files(*, policy: str) -> dict[str, str]:
    """A Snowflake project for the real SQLQuack whose function-mode test calls a SQL function."""

    return {
        "sqlbuild_project.toml": _LOCAL_PROJECT_NAME + _LOCAL_CONNECTION + _LOCAL_DEFAULTS,
        "sqlquack.toml": policy,
        "functions/sql/sku_family.sql": _local_function(
            arguments="sku VARCHAR",
            returns="VARCHAR",
            body="UPPER(REGEXP_SUBSTR(sku, '^[A-Za-z]+'))",
        ),
        "tests/unit/functions/sql/test_sku_family.sql": (
            'TEST (mode udf, name "sku_family_extracts_leading_letters");\n\n'
            "WITH input_values AS (\n"
            "  SELECT v.sku FROM VALUES ('DUCK-MAL-01'), ('decoy-teal-1'), ('123-POND'), (NULL)"
            " AS v(sku)\n),\n"
            "__udf_actual__ AS (\n"
            '  SELECT i.sku, __udf("sku_family")(i.sku) AS sku_family FROM input_values AS i\n'
            "),\n"
            "__udf_expected__ AS (\n"
            "  SELECT v.sku::VARCHAR AS sku, v.sku_family::VARCHAR AS sku_family\n"
            "  FROM VALUES ('DUCK-MAL-01', 'DUCK'), ('decoy-teal-1', 'DECOY'),"
            " ('123-POND', NULL), (NULL, NULL) AS v(sku, sku_family)\n"
            ")\n"
        ),
    }
