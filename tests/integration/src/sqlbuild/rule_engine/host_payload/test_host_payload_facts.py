"""Every Rule fact reads the same values from the native host payload as from the full project."""

from pathlib import Path

import pytest

from sqlbuild._native import RuleFactViews, RulesConfig, custom_host_payload
from sqlbuild.compiler.compile.types import CompiledProjectView
from sqlbuild.rule_engine._helpers.host.host_payload import decode_host_payload
from sqlbuild.rule_engine.types import RuleFactProject
from tests.integration.src.sqlbuild.rule_engine.host_payload._test_types import (
    HostPayloadFactsTestCase,
    HostPayloadLazyReadTestCase,
)
from tests.integration.src.sqlbuild.rule_engine.host_payload.helpers import (
    ReferenceProject,
    compile_files,
    every_fact_value,
    fact_views,
    native_project,
    record_project,
)

_PROJECT_FILES: dict[str, str] = {
    "sqlbuild_project.toml": 'name = "orders"\nadapter = "duckdb"\n',
    "sql/sources/raw.yml": (
        "sources:\n"
        "  - name: raw_orders\n    description: Raw orders.\n"
        '    expression: "(SELECT 1 AS id, CAST(10.5 AS DOUBLE) AS amount)"\n'
        "    columns:\n      - name: id\n        type: INTEGER\n"
        "  - name: raw_customers\n    description: Raw customers.\n"
        '    expression: "(SELECT 1 AS customer_id)"\n'
    ),
    "sql/seeds/regions.csv": "region_id,name\n1,north\n",
    "sql/seeds/regions.yml": (
        "seeds:\n  - name: regions\n    description: Regions.\n    columns:\n"
        "      - name: region_id\n        type: INTEGER\n"
        "      - name: name\n        type: VARCHAR\n"
    ),
    "sql/functions/sql/is_paid.sql": (
        "FUNCTION (\n  description 'Paid status.',\n  arguments (status STRING),\n"
        "  returns BOOLEAN,\n);\n\nstatus = 'PAID'\n"
    ),
    "sql/constants/policy.sql": "CONSTANT (name minimum_amount, value 7);\n",
    "sql/enums/order_status.sql": "ENUM (name order_status, members [PAID, OPEN]);\n",
    "sql/models/marts/_sqlbuild/_macros/python/amounts.py": (
        'def doubled(amount: str) -> str:\n    return f"({amount}) * 2"\n'
    ),
    "sql/models/staging/stg_orders.sql": (
        "MODEL (description 'Staged orders.',\n  contract enforced,\n"
        "  columns (\n"
        "    order_id (type INTEGER, nullable false, description 'Order key',\n"
        "      audits [not_null, accepted_values (values [1, 2], severity warn)]),\n"
        "    amount (type DOUBLE),\n    status (type order_status),\n  ),\n);\n\n"
        "SELECT CAST(id AS INTEGER) AS order_id, amount, 'PAID' AS status\n"
        'FROM __source("raw_orders")\n'
    ),
    "sql/models/marts/order_totals.sql": (
        'MODEL (description "Order totals", materialized table);\n\n'
        'SELECT o.order_id, @py.doubled("o.amount") AS doubled_amount, o.status\n'
        'FROM __ref("stg_orders") AS o WHERE o.amount > 7\n'
    ),
    "sql/models/marts/order_flags.sql": (
        'MODEL (description "Order flags");\n\n'
        'SELECT o.order_id, o.missing_flag AS flag FROM __ref("stg_orders") AS o\n'
    ),
    "sql/models/marts/flag_counts.sql": (
        'MODEL (description "Flag counts");\n\nSELECT f.flag FROM __ref("order_flags") AS f\n'
    ),
    "tests/unit/test_order_totals.sql": (
        "TEST ();\n\nWITH\n__ref__stg_orders AS (\n"
        "  SELECT 1 AS order_id, 10.0 AS amount, 'PAID' AS status\n),\n"
        "__expected__order_totals AS (\n"
        "  SELECT 1 AS order_id, 20.0 AS doubled_amount, 'PAID' AS status\n)\n"
        "SELECT 1\n"
    ),
}


_STAGING: str = "sql/models/staging/stg_orders.sql"
_POPULATED_FACTS: tuple[str, ...] = (
    "project.sources",
    "project.seeds",
    "project.functions",
    "tests.all",
    "audits.all",
    "declarations.enums",
    "declarations.constants",
    f"columns.declared:{_STAGING}",
    f"columns.inferred:{_STAGING}",
    f"columns.enum_types:{_STAGING}",
    f"audits.for_model:{_STAGING}",
    "tests.for_model:sql/models/marts/order_totals.sql",
    "graph.dependents:sql/models/marts/order_flags.sql",
)


@pytest.mark.parametrize(
    "test_case",
    (
        HostPayloadFactsTestCase(
            description="native compiled project",
            project_files=_PROJECT_FILES,
            payload_source=native_project,
            expected_populated_facts=_POPULATED_FACTS,
        ),
        HostPayloadFactsTestCase(
            description="compiled project and model records",
            project_files=_PROJECT_FILES,
            payload_source=record_project,
            expected_populated_facts=_POPULATED_FACTS,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_compiled_project_when_decoding_host_payload_then_every_fact_is_unchanged(
    tmp_path: Path, test_case: HostPayloadFactsTestCase
) -> None:
    project: CompiledProjectView = compile_files(root=tmp_path, files=test_case.project_files)
    config: RulesConfig = RulesConfig()

    decoded, decoded_config = decode_host_payload(
        custom_host_payload(test_case.payload_source(project), config)
    )
    expected: dict[str, object] = every_fact_value(
        fact_views(project=ReferenceProject(project), root=tmp_path)
    )
    actual: dict[str, object] = every_fact_value(fact_views(project=decoded, root=tmp_path))

    assert decoded_config == config
    assert actual == expected
    assert repr(actual) == repr(expected)
    assert all(expected[key] for key in test_case.expected_populated_facts)
    assert decoded.effective_target_name == project.effective_target_name


@pytest.mark.parametrize(
    "test_case",
    (
        HostPayloadLazyReadTestCase(
            description="contract reads leave SQL, columns, tests and audits encoded",
            project_files=_PROJECT_FILES,
            expected_enforced=(False, False, False, True),
            expected_encoded_model_facts=frozenset(
                {"authored_sql", "query_sql", "declared_columns", "inferred_columns"}
            ),
            expected_encoded_project_facts=frozenset({"sql_tests", "audits", "sources"}),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_rule_reading_one_fact_per_model_when_decoding_then_other_facts_stay_encoded(
    tmp_path: Path, test_case: HostPayloadLazyReadTestCase
) -> None:
    project: CompiledProjectView = compile_files(root=tmp_path, files=test_case.project_files)

    decoded: RuleFactProject = decode_host_payload(custom_host_payload(project, RulesConfig()))[0]
    views: RuleFactViews = fact_views(project=decoded, root=tmp_path)
    enforced: tuple[bool, ...] = tuple(map(views.contracts.enforced, views.project.models))

    assert enforced == test_case.expected_enforced
    assert all(
        test_case.expected_encoded_model_facts.isdisjoint(vars(model)) for model in decoded.models
    )
    assert test_case.expected_encoded_project_facts.isdisjoint(vars(decoded))
