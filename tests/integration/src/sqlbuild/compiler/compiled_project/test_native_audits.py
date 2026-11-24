"""Audits attached by the native model loop and assembled into the native compiled project."""

from __future__ import annotations

from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from sqlbuild.compiler.compile.exceptions import CompileInputError
from sqlbuild.compiler.compile.types import CompiledAuditView
from tests.integration.src.sqlbuild.compiler.compiled_project._test_types import (
    NativeAuditAttachmentTestCase,
    NativeAuditErrorTestCase,
    NativeMeasurementPolicyTestCase,
)
from tests.integration.src.sqlbuild.compiler.compiled_project.helpers import (
    AUDIT_PROJECT_FILES,
    audit_summary,
    compile_function_project,
    measurement_policy,
)

if TYPE_CHECKING:
    from sqlbuild.compiler.compile.types import AuditInput


@pytest.mark.parametrize(
    "test_case",
    [
        NativeAuditAttachmentTestCase(
            description="singular audit reading two models under its own run scope",
            audit_name="orders_have_customers",
            expected_summary=(
                "NativeCompiledAudit",
                "orders_have_customers",
                ("orders", "customers"),
                "warn",
                "final",
                (None, None),
                'SELECT * FROM __ref("orders") o LEFT JOIN __ref("customers") c'
                " ON o.order_id = c.customer_id WHERE c.customer_id IS NULL",
            ),
            expected_usages=(
                "singular_audit:orders_have_customers:customers",
                "singular_audit:orders_have_customers:orders",
            ),
        ),
        NativeAuditAttachmentTestCase(
            description="built-in column audit under the project default severity",
            audit_name="not_null",
            expected_summary=(
                "NativeCompiledAudit",
                "not_null",
                ("orders",),
                "warn",
                "delta_and_final",
                ("orders", "order_id"),
                'SELECT order_id\nFROM __ref("orders")\nWHERE order_id IS NULL',
            ),
            expected_usages=(),
        ),
        NativeAuditAttachmentTestCase(
            description="project generic audit with an argument and its own severity",
            audit_name="at_least",
            expected_summary=(
                "NativeCompiledAudit",
                "at_least",
                ("orders",),
                "error",
                "delta_and_final",
                ("orders", "order_id"),
                'SELECT * FROM __ref("orders") WHERE order_id < 1',
            ),
            expected_usages=("audit:at_least:orders",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_audit_when_compiling_then_native_project_holds_its_attachment(
    test_case: NativeAuditAttachmentTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    project, inputs = compile_function_project(
        project_dir=tmp_path, files=AUDIT_PROJECT_FILES, monkeypatch=monkeypatch
    )

    audits: dict[str, CompiledAuditView] = {audit.name: audit for audit in project.audits}
    audit_inputs: dict[str, AuditInput] = {
        audit.definition_name: audit_input
        for audit, audit_input in zip(project.audits, inputs.audit_inputs, strict=True)
    }
    assert audit_summary(audits[test_case.audit_name]) == test_case.expected_summary
    assert (
        tuple(
            f"{usage.declaration.kind.value}:{usage.declaration.name}:{usage.consumer.name}"
            for usage in audit_inputs[test_case.audit_name].declaration_usages
        )
        == test_case.expected_usages
    )


@pytest.mark.parametrize(
    "test_case",
    [
        NativeAuditErrorTestCase(
            description="attachment of an unknown generic audit",
            files={
                "sql/models/customers.sql": (
                    "MODEL (description 'Customers.', columns (customer_id (type INTEGER,"
                    " audits [at_most(maximum 3)])));\nSELECT 1 AS customer_id\n"
                ),
            },
            expected_message="references unknown generic audit 'at_most'",
        ),
        NativeAuditErrorTestCase(
            description="singular audit reading an unknown model",
            files={
                "sql/audits/singular/orders_have_customers.sql": (
                    "AUDIT (name orders_have_customers);\n"
                    'SELECT * FROM __ref("orders") JOIN __ref("missing_customers") ON TRUE\n'
                ),
            },
            expected_message=(
                "Audit file sql/audits/singular/orders_have_customers.sql references unknown model"
                " 'missing_customers'"
            ),
        ),
        NativeAuditErrorTestCase(
            description="unknown project default run scope",
            files={
                "sqlbuild_project.toml": (
                    'name = "orders"\nadapter = "duckdb"\n\n[settings]\n'
                    'default_audit_run_scope = "always"\n'
                ),
            },
            expected_message=(
                "settings.default_audit_run_scope in sqlbuild_project.toml: unknown value 'always'"
            ),
        ),
        NativeAuditErrorTestCase(
            description="unknown singular audit run scope",
            files={
                "sql/audits/singular/orders_have_customers.sql": (
                    "AUDIT (name orders_have_customers, run_scope always);\n"
                    'SELECT * FROM __ref("orders") JOIN __ref("customers") ON TRUE\n'
                ),
            },
            expected_message="unknown audit run_scope 'always'; valid values: delta_and_final, final",
        ),
        NativeAuditErrorTestCase(
            description="model-attached audit that does not read its model",
            files={
                "sql/audits/generic/at_least.sql": (
                    'AUDIT (name at_least);\nSELECT * FROM __ref("customers") WHERE @minimum > 0\n'
                ),
            },
            expected_message=(
                "sql/models/orders.sql audit 'at_least': model-attached audit must reference the"
                " attached model 'orders' via __ref()"
            ),
        ),
        NativeAuditErrorTestCase(
            description="singular audit reading a dbt model",
            files={
                "sql/audits/singular/orders_have_customers.sql": (
                    "AUDIT (name orders_have_customers);\n"
                    'SELECT * FROM __ref("orders") JOIN __dbt_ref("customers") ON TRUE\n'
                ),
            },
            expected_message=(
                'may not use __dbt_ref("customers"); audit dbt model checks belong in dbt'
            ),
        ),
        NativeAuditErrorTestCase(
            description="thresholds on a violation audit attachment",
            files={
                "sql/models/orders.sql": (
                    "MODEL (description 'Orders.', columns (order_id (type INTEGER,"
                    " audits [at_least(minimum 1, thresholds (warn (above 1)))])));\n"
                    "SELECT 1 AS order_id\n"
                ),
            },
            expected_message="thresholds and minimum_samples are only valid for measurement audits",
        ),
        NativeAuditErrorTestCase(
            description="standalone measurement audit with a non-numeric threshold",
            files={
                "sql/audits/singular/coverage.sql": 'AUDIT (name coverage, evaluation measurement, value rate, thresholds (warn (above \'x\')));\nMEASURE (SELECT COUNT(*) AS rate FROM __ref("orders") JOIN __ref("customers") ON TRUE);\n'
            },
            expected_message=(
                "sql/audits/singular/coverage.sql standalone measurement audit invalid thresholds:"
                " above threshold requires one numeric value"
            ),
        ),
        NativeAuditErrorTestCase(
            description="standalone measurement audit with negative minimum samples",
            files={
                "sql/audits/singular/coverage.sql": 'AUDIT (name coverage, evaluation measurement, value rate, thresholds (warn (above 1)), minimum_samples -1);\nMEASURE (SELECT COUNT(*) AS rate FROM __ref("orders") JOIN __ref("customers") ON TRUE);\n'
            },
            expected_message=(
                "sql/audits/singular/coverage.sql standalone measurement audit 'minimum_samples'"
                " must be a non-negative integer"
            ),
        ),
        NativeAuditErrorTestCase(
            description="generic audit attachment missing an argument",
            files={
                "sql/models/orders.sql": (
                    "MODEL (description 'Orders.', columns (order_id (type INTEGER, audits [at_least])));\n"
                    "SELECT 1 AS order_id\n"
                )
            },
            expected_message="sql/models/orders.sql is missing argument 'minimum' for generic audit 'at_least'",
        ),
        NativeAuditErrorTestCase(
            description="generic audit attachment overriding its implicit column",
            files={
                "sql/models/orders.sql": (
                    "MODEL (description 'Orders.', columns (order_id (type INTEGER, audits [at_least(minimum 1, column other_id)])));\n"
                    "SELECT 1 AS order_id\n"
                )
            },
            expected_message=(
                "sql/models/orders.sql audit 'at_least' must not override implicit column from"
                " attached context"
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_audit_when_compiling_then_native_attachment_rejects_it(
    test_case: NativeAuditErrorTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    with pytest.raises(CompileInputError) as raised:
        _ = compile_function_project(
            project_dir=tmp_path,
            files={**AUDIT_PROJECT_FILES, **test_case.files},
            monkeypatch=monkeypatch,
        )

    assert test_case.expected_message in str(raised.value)


@pytest.mark.parametrize(
    "test_case",
    [
        NativeMeasurementPolicyTestCase(
            description="error threshold makes the standalone audit an error",
            files={
                "sql/audits/singular/coverage.sql": 'AUDIT (name coverage, evaluation measurement, value rate, thresholds (warn (above 1), error (above 5)), minimum_samples 3);\nMEASURE (SELECT COUNT(*) AS rate FROM __ref("orders") JOIN __ref("customers") ON TRUE);\n'
            },
            expected_policy=("above", 1.0, "above", 5.0, 3, "error"),
        ),
        NativeMeasurementPolicyTestCase(
            description="warn-only threshold keeps an unbounded authored sample count",
            files={
                "sql/audits/singular/coverage.sql": 'AUDIT (name coverage, evaluation measurement, value rate, thresholds (warn (above 1)), minimum_samples 99999999999999999999);\nMEASURE (SELECT COUNT(*) AS rate FROM __ref("orders") JOIN __ref("customers") ON TRUE);\n'
            },
            expected_policy=("above", 1.0, None, None, 99999999999999999999, "warn"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_standalone_measurement_audit_when_compiling_then_header_policy_is_parsed(
    test_case: NativeMeasurementPolicyTestCase,
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    project, _ = compile_function_project(
        project_dir=tmp_path,
        files={**AUDIT_PROJECT_FILES, **test_case.files},
        monkeypatch=monkeypatch,
    )

    audit: CompiledAuditView = {audit.name: audit for audit in project.audits}["coverage"]
    assert measurement_policy(audit) == test_case.expected_policy
