"""Run-time warnings for project relation names hard-coded in Python SQL."""

from __future__ import annotations

import pytest

from sqlbuild._native import HardCodedRelationOwnerKind
from sqlbuild.executor.python_nodes.classes.runtime_relation_guard import RuntimeRelationGuard
from sqlbuild.python_nodes.models import SqlResourceRef
from sqlbuild.refs import model, source
from tests.unit.src.sqlbuild.executor.python_nodes.classes._test_types import (
    RuntimeRelationGuardTestCase,
)

_PROJECT_RELATIONS: dict[SqlResourceRef, str] = {
    model("customers"): "main.customers",
    model("orders"): "main.orders",
    source("raw_payments"): "raw.payments",
}


@pytest.mark.parametrize(
    "test_case",
    (
        RuntimeRelationGuardTestCase(
            description="unqualified model name resolved against the current schema warns once",
            statements=("SELECT * FROM customers", "SELECT count(*) FROM main.customers"),
            expected_warning_fragments=(
                "[P008] task 'export_orders' named model:customers as 'customers' in SQL; "
                'declare it with depends_on=model("customers") and use '
                'ctx.relation(model("customers")) instead of the relation name',
            ),
            expected_warning_count=1,
        ),
        RuntimeRelationGuardTestCase(
            description="check naming an undeclared model warns with the depends_on remedy",
            statements=("SELECT count(*) FROM orders",),
            expected_warning_fragments=(
                "[P008] check 'orders_present' named model:orders as 'orders'",
                'declare it with depends_on=model("orders")',
            ),
            expected_warning_count=1,
            owner_label="check 'orders_present'",
        ),
        RuntimeRelationGuardTestCase(
            description="loader naming a model warns that loaders run before models",
            statements=("SELECT * FROM customers",),
            expected_warning_fragments=(
                "[P008] loader 'raw_orders' named model:customers as 'customers'",
                "loaders run before models and seeds and cannot read them",
            ),
            expected_warning_count=1,
            owner_label="loader 'raw_orders'",
            owner_kind=HardCodedRelationOwnerKind.LOADER,
        ),
        RuntimeRelationGuardTestCase(
            description="loader naming a source warns with the ctx.source remedy",
            statements=("SELECT * FROM raw.payments",),
            expected_warning_fragments=('use ctx.source("raw_payments") instead',),
            expected_warning_count=1,
            owner_label="loader 'raw_orders'",
            owner_kind=HardCodedRelationOwnerKind.LOADER,
        ),
        RuntimeRelationGuardTestCase(
            description="loader naming an upstream loader's source warns with the ctx.loader remedy",
            statements=("SELECT * FROM raw.payments",),
            expected_warning_fragments=("use ctx.loader(raw_payments) instead",),
            expected_warning_count=1,
            owner_label="loader 'raw_orders'",
            owner_kind=HardCodedRelationOwnerKind.LOADER,
            upstream_loader_by_source={"raw_payments": "raw_payments"},
        ),
        RuntimeRelationGuardTestCase(
            description="loader source resolved through ctx.source does not warn",
            statements=("SELECT * FROM raw.payments",),
            expected_warning_fragments=(),
            expected_warning_count=0,
            resolved_refs=(source("raw_payments"),),
            owner_label="loader 'raw_orders'",
            owner_kind=HardCodedRelationOwnerKind.LOADER,
        ),
        RuntimeRelationGuardTestCase(
            description="qualified source table warns",
            statements=("DELETE FROM raw.payments WHERE 1 = 0",),
            expected_warning_fragments=("named source:raw_payments as 'raw.payments'",),
            expected_warning_count=1,
        ),
        RuntimeRelationGuardTestCase(
            description="relation resolved through ctx.relation does not warn",
            statements=("SELECT * FROM main.customers",),
            expected_warning_fragments=(),
            expected_warning_count=0,
            resolved_refs=(model("customers"),),
        ),
        RuntimeRelationGuardTestCase(
            description="own destination does not warn",
            statements=("INSERT INTO orders SELECT 1",),
            expected_warning_fragments=(),
            expected_warning_count=0,
            own_refs=frozenset({model("orders")}),
        ),
        RuntimeRelationGuardTestCase(
            description="temporary tables and non-project relations do not warn",
            statements=(
                "CREATE TEMP TABLE customers AS SELECT 1 AS id",
                "SELECT * FROM customers JOIN information_schema.tables ON 1 = 0",
            ),
            expected_warning_fragments=(),
            expected_warning_count=0,
        ),
        RuntimeRelationGuardTestCase(
            description="ClickHouse TRUNCATE, ALTER DELETE and INTO OUTFILE warn",
            statements=(
                "TRUNCATE TABLE main.orders",
                "ALTER TABLE main.orders DELETE WHERE order_id = 1",
                "SELECT * FROM main.customers INTO OUTFILE 'customers.csv'",
            ),
            expected_warning_fragments=("named model:orders", "named model:customers"),
            expected_warning_count=2,
            dialect="clickhouse",
        ),
        RuntimeRelationGuardTestCase(
            description="ClickHouse temporary table, OPTIMIZE and ALTER UPDATE do not warn",
            statements=(
                "CREATE TEMPORARY TABLE customers AS SELECT 1 AS id",
                "SELECT * FROM customers",
                "OPTIMIZE TABLE main.orders FINAL",
                "ALTER TABLE main.orders UPDATE status = 'x' WHERE 1",
            ),
            expected_warning_fragments=(),
            expected_warning_count=0,
            dialect="clickhouse",
        ),
        RuntimeRelationGuardTestCase(
            description="unparseable SQL is not blocked and does not warn",
            statements=("VACUUM ((( customers",),
            expected_warning_fragments=(),
            expected_warning_count=0,
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_python_sql_when_checking_then_warns_for_hard_coded_project_relations(
    test_case: RuntimeRelationGuardTestCase,
) -> None:
    warnings: list[str] = []
    guard: RuntimeRelationGuard = RuntimeRelationGuard(
        owner_label=test_case.owner_label,
        owner_kind=test_case.owner_kind,
        project_relations=_PROJECT_RELATIONS,
        dialect=test_case.dialect,
        default_database=None,
        default_schema="main",
        warnings=warnings,
        own_refs=test_case.own_refs,
        upstream_loader_by_source=test_case.upstream_loader_by_source,
    )
    for ref in test_case.resolved_refs:
        guard.record_resolved(ref)

    for statement in test_case.statements:
        guard.check(statement)

    assert len(warnings) == test_case.expected_warning_count
    assert all(fragment in "\n".join(warnings) for fragment in test_case.expected_warning_fragments)


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
