"""Tests for canonical authored resource identities."""

from __future__ import annotations

from pathlib import Path

import pytest

from sqlbuild._native import (
    ResourceIdentityError,
    discover_project_inputs,
    validate_resource_identity,
)
from tests.unit.src.sqlbuild.compiler.discovery._helpers._test_types import (
    CanonicalResourceIdentityTestCase,
    DiscoveredResourceIdentityErrorTestCase,
    InvalidResourceIdentityTestCase,
)

_RESOURCE_PATH: Path = Path("models/example.sql")


@pytest.mark.parametrize(
    "test_case",
    [
        CanonicalResourceIdentityTestCase(
            description="ordinary snake case",
            name="daily_orders",
            private_identity=False,
            expected_name="daily_orders",
        ),
        CanonicalResourceIdentityTestCase(
            description="established double underscore separators",
            name="commerce__mart_v_order",
            private_identity=False,
            expected_name="commerce__mart_v_order",
        ),
        CanonicalResourceIdentityTestCase(
            description="private scoped declaration",
            name="_country_codes",
            private_identity=True,
            expected_name="_country_codes",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_canonical_name_when_validating_resource_identity_then_name_is_accepted(
    test_case: CanonicalResourceIdentityTestCase,
) -> None:
    validate_resource_identity(
        name=test_case.name,
        kind="model",
        path=_RESOURCE_PATH,
        private_identity=test_case.private_identity,
    )

    assert test_case.name == test_case.expected_name


@pytest.mark.parametrize(
    "test_case",
    [
        InvalidResourceIdentityTestCase(
            description="upper camel case",
            name="DailyHTTPEvents",
            expected_corrected_name="daily_http_events",
        ),
        InvalidResourceIdentityTestCase(
            description="hyphenated name",
            name="daily-orders",
            expected_corrected_name="daily_orders",
        ),
        InvalidResourceIdentityTestCase(
            description="trailing separator",
            name="daily_orders_",
            expected_corrected_name="daily_orders",
        ),
        InvalidResourceIdentityTestCase(
            description="private declaration with uppercase words",
            name="_DailyOrders",
            expected_corrected_name="_daily_orders",
            private_identity=True,
        ),
        InvalidResourceIdentityTestCase(
            description="private declaration without leading underscore",
            name="daily_orders",
            expected_corrected_name="_daily_orders",
            private_identity=True,
        ),
        InvalidResourceIdentityTestCase(
            description="public declaration with leading underscore",
            name="_daily_orders",
            expected_corrected_name="daily_orders",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_noncanonical_name_when_validating_resource_identity_then_coded_error_suggests_rename(
    test_case: InvalidResourceIdentityTestCase,
) -> None:
    with pytest.raises(
        ResourceIdentityError,
        match=(
            rf"Invalid model identity '{test_case.name}'.*"
            rf"use snake_case '{test_case.expected_corrected_name}'"
        ),
    ) as error_info:
        validate_resource_identity(
            name=test_case.name,
            kind="model",
            path=_RESOURCE_PATH,
            private_identity=test_case.private_identity,
        )

    assert error_info.value.code == "D016"


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoveredResourceIdentityErrorTestCase(
            description="model filename",
            project_files={"models/DailyOrders.sql": "MODEL ();\n\nSELECT 1\n"},
            expected_error_fragment="Invalid model identity 'DailyOrders'",
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="source declaration",
            project_files={
                "sources/raw.yml": (
                    "sources:\n"
                    "  - name: raw-Orders\n"
                    "    database: analytics\n"
                    "    schema: raw\n"
                    "    table: orders\n"
                )
            },
            expected_error_fragment="Invalid source identity 'raw-Orders'",
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="SQL function filename",
            project_files={"functions/sql/CalculateTax.sql": "FUNCTION (returns INTEGER);\n\n1\n"},
            expected_error_fragment="Invalid function identity 'CalculateTax'",
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="SQL test name",
            project_files={"tests/unit/orders.sql": "TEST (name OrderBehavior);\n\nSELECT 1\n"},
            expected_error_fragment="Invalid SQL test identity 'OrderBehavior'",
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="standalone audit name",
            project_files={"audits/singular/orders.sql": "AUDIT (name OrderRule);\n\nSELECT 1\n"},
            expected_error_fragment="Invalid audit identity 'OrderRule'",
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="generic audit filename",
            project_files={
                "audits/generic/OrderRule.sql": (
                    "AUDIT (name valid_block_name);\n\nSELECT * FROM @relation\n"
                )
            },
            expected_error_fragment="Invalid audit identity 'OrderRule'",
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="generic audit explicit instance name",
            project_files={
                "audits/generic/order_rule.sql": (
                    "AUDIT (name OrderRule);\n\nSELECT * FROM @relation\n"
                ),
                "models/orders.sql": "MODEL (audits [order_rule]);\n\nSELECT 1\n",
            },
            expected_error_fragment="Invalid generic audit instance identity 'OrderRule'",
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="public declaration with private spelling",
            project_files={"constants/orders.sql": "CONSTANT (name _tax_rate, value 20);\n"},
            expected_error_fragment="Invalid public constant identity '_tax_rate'",
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="private model declaration without private spelling",
            project_files={
                "models/orders.sql": (
                    'MODEL (constants (tax_rate 20));\n\nSELECT @const("tax_rate")\n'
                )
            },
            expected_error_fragment=("Invalid model-local constant identity 'tax_rate'"),
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="SQL hook filename",
            project_files={
                "hooks/sql/GrantAccess.sql": "HOOK ();\n\nGRANT SELECT ON orders TO analyst\n"
            },
            expected_error_fragment="Invalid SQL hook identity 'GrantAccess'",
        ),
        DiscoveredResourceIdentityErrorTestCase(
            description="Python asset name",
            project_files={
                "python/tasks/orders.py": (
                    "from sqlbuild.assets import asset\n\n"
                    "@asset\n"
                    "def DailyTask(ctx):\n"
                    "    return None\n"
                )
            },
            expected_error_fragment="Invalid asset identity 'DailyTask'",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_noncanonical_discovered_resource_when_discovering_project_then_d016_blocks_identity(
    tmp_path: Path,
    test_case: DiscoveredResourceIdentityErrorTestCase,
) -> None:
    project_files: dict[str, str] = {
        "sqlbuild_project.toml": 'name = "identity_test"\nadapter = "duckdb"\n',
        **test_case.project_files,
    }
    for relative_path, contents in project_files.items():
        path: Path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")

    with pytest.raises(
        ResourceIdentityError,
        match=test_case.expected_error_fragment,
    ) as error_info:
        discover_project_inputs(project_dir=tmp_path)

    assert error_info.value.code == "D016"
