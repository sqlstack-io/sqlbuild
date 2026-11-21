from pathlib import Path

import pytest

from sqlbuild._native import CostContext
from sqlbuild.cost.models import CostResourceContext
from tests.unit.src.sqlbuild.cost._test_types import CostResourceScopeTestCase


@pytest.mark.parametrize(
    "test_case",
    [
        CostResourceScopeTestCase(
            description="inherits run ledger while overriding resource phase and attempt",
            ledger_path=Path("target/executions/run-1/statements.jsonl"),
            expected_resource_type="asset",
            expected_resource_name="refresh_orders",
            expected_phase="execute",
            expected_attempt=3,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_parent_cost_context_when_scoping_resource_then_inherits_run_ledger(
    test_case: CostResourceScopeTestCase,
) -> None:
    with CostContext.scope(
        run_id="run-1",
        resource_type="run",
        resource_name="dev",
        ledger_path=test_case.ledger_path,
        phase="build",
    ):
        with CostContext.resource_scope(
            resource_type=test_case.expected_resource_type,
            resource_name=test_case.expected_resource_name,
            phase=test_case.expected_phase,
            attempt=test_case.expected_attempt,
        ):
            actual: CostResourceContext | None = CostContext.current()
        with CostContext.scope(
            run_id="child-run",
            resource_type="run",
            resource_name="audit",
        ):
            nested: CostResourceContext | None = CostContext.current()

        restored: CostResourceContext | None = CostContext.current()

    assert actual is not None
    assert actual.run_id == "run-1"
    assert actual.ledger_path == test_case.ledger_path
    assert actual.resource_type == test_case.expected_resource_type
    assert actual.resource_name == test_case.expected_resource_name
    assert actual.phase == test_case.expected_phase
    assert actual.attempt == test_case.expected_attempt
    assert restored is not None
    assert restored.resource_type == "run"
    assert nested is not None
    assert nested.ledger_path == test_case.ledger_path
    assert CostContext.current() is None


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
