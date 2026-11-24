from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    Model,
    project_with,
)
from sqlbuild.compiler.compile.types import CompiledAuditView, CompiledProjectView
from sqlbuild.rule_engine.classes.audit_facts import AuditFacts
from tests.unit.src.sqlbuild.rule_engine.classes.audit_facts._test_types import (
    AuditFactsTestCase,
)
from tests.unit.src.sqlbuild.rule_engine.classes.audit_facts.helpers import build_audit
from tests.unit.src.sqlbuild.rule_engine.main.evaluate.helpers import build_project


@pytest.mark.parametrize(
    "test_case",
    (
        AuditFactsTestCase(
            description="attached and project audits",
            attached_name="order_amount",
            other_model_name="customer_status",
            project_audit_name="inventory_balance",
            expected_attached_names=("order_amount",),
            expected_all_names=("order_amount", "customer_status", "inventory_balance"),
        ),
    ),
    ids=lambda case: case.description,
)
def test_given_attached_and_project_audits_when_querying_model_then_returns_only_attached_audits(
    test_case: AuditFactsTestCase,
) -> None:
    attached: CompiledAuditView = build_audit(
        name=test_case.attached_name, attached_target_name="orders"
    )
    other_model: CompiledAuditView = build_audit(
        name=test_case.other_model_name, attached_target_name="customers"
    )
    project_audit: CompiledAuditView = build_audit(
        name=test_case.project_audit_name, attached_target_name=None
    )
    project: CompiledProjectView = project_with(
        project=build_project(
            name="orders",
            relative_path="sql/models/orders.sql",
            sql="SELECT 1 AS order_id",
            config_values={},
        ),
        audits=(attached, other_model, project_audit),
    )

    facts: AuditFacts = AuditFacts(project=project)

    attached_audits: tuple[CompiledAuditView, ...] = facts.for_model(
        Model(name="orders", path=Path("sql/models/orders.sql"), materialization="view")
    )
    assert tuple(audit.name for audit in attached_audits) == test_case.expected_attached_names
    assert tuple(audit.name for audit in facts.all()) == test_case.expected_all_names


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
