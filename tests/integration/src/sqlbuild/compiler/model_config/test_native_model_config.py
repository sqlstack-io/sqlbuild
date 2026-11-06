"""The native model config stage parses headers like the YAML schema parsers."""

from __future__ import annotations

import random
from pathlib import Path
from typing import TYPE_CHECKING

import pytest

from sqlbuild._native import assemble_project
from sqlbuild.adapter.contract.models import ExpressionInferenceProfile
from sqlbuild.compiler.compile.types import CompiledProjectView
from tests.integration.src.sqlbuild.compiler.model_config._test_types import (
    ColumnMetadataExpansionTestCase,
    HeaderAuditArgumentsTestCase,
    HeaderMetadataParityTestCase,
    ModelConfigTierTestCase,
)
from tests.integration.src.sqlbuild.compiler.model_config.helpers import (
    HeaderMetadataParity,
    column_metadata_outcome,
    config_value_inputs,
    generated_header_metadata,
    header_metadata_parity,
    model_config_outcome,
)

if TYPE_CHECKING:
    from sqlbuild._native import CompileInputsView


@pytest.mark.parametrize(
    "test_case",
    [
        HeaderMetadataParityTestCase(
            description="seeded columns and audits, valid and invalid",
            seed=20261007,
            count=4000,
            expected_minimum_parsed=400,
            expected_minimum_rejected=2000,
            expected_maximum_later_validated=60,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_generated_header_metadata_when_attaching_then_model_loop_matches_yaml_parsers(
    test_case: HeaderMetadataParityTestCase, tmp_path: Path
) -> None:
    parity: HeaderMetadataParity = header_metadata_parity(
        project_dir=tmp_path,
        headers=generated_header_metadata(rng=random.Random(test_case.seed), count=test_case.count),
    )

    assert (
        parity.mismatches,
        parity.parsed >= test_case.expected_minimum_parsed,
        parity.rejected >= test_case.expected_minimum_rejected,
        parity.later_validated <= test_case.expected_maximum_later_validated,
    ) == ([], True, True, True), (parity.parsed, parity.rejected, parity.later_validated)


@pytest.mark.parametrize(
    "test_case",
    [
        HeaderAuditArgumentsTestCase(
            description="numeric keys on model and column audits are ignored during compilation",
            header={
                "description": "One row per order.",
                "columns": {"order_id": {"audits": [{"unique": {1: "ignored"}}]}},
                "audits": [{"unique": {2: "also ignored", "column": "order_id"}}],
            },
            expected_audits=[("unique", "order_id"), ("unique", "order_id")],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_non_string_audit_arguments_when_attaching_then_keys_remain_ignored(
    test_case: HeaderAuditArgumentsTestCase,
    tmp_path: Path,
) -> None:
    inputs: CompileInputsView = config_value_inputs(project_dir=tmp_path, header=test_case.header)
    project: CompiledProjectView = assemble_project(
        inputs=inputs, inference_profile=ExpressionInferenceProfile(sql_analysis_dialect="duckdb")
    )

    assert [
        (audit.definition_name, audit.attached_column_name) for audit in project.audits
    ] == test_case.expected_audits
    assert project.diagnostics == ()


@pytest.mark.parametrize(
    "test_case",
    [
        ColumnMetadataExpansionTestCase(
            description="a project variable template expands in a column type",
            header_column="amount (type '${column_type}')",
            expected_type="BIGINT",
            expected_error=None,
        ),
        ColumnMetadataExpansionTestCase(
            description="an unknown template variable in a column type is rejected",
            header_column="amount (type '${target.schema}')",
            expected_type=None,
            expected_error="model config references unknown variable 'target.schema'",
        ),
        ColumnMetadataExpansionTestCase(
            description="a macro call in a column description is rejected",
            header_column="amount (description '@py.cents(amount)')",
            expected_type=None,
            expected_error="model config field 'columns.amount.description' does not allow macros",
        ),
        ColumnMetadataExpansionTestCase(
            description="a macro marker after a space in a column type is rejected",
            header_column="amount (type '@cents\u2003(amount)')",
            expected_type=None,
            expected_error="model config field 'columns.amount.type' does not allow macros",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_template_or_macro_column_metadata_when_attaching_then_loop_expands_or_rejects(
    test_case: ColumnMetadataExpansionTestCase, tmp_path: Path
) -> None:
    column_type, error = column_metadata_outcome(
        project_dir=tmp_path, header_column=test_case.header_column
    )

    assert (column_type, error) == (test_case.expected_type, test_case.expected_error)


@pytest.mark.parametrize(
    "test_case",
    [
        ModelConfigTierTestCase(
            description="native model config",
            expected_native_calls={
                "expand_template_data": 0,
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_project_when_building_model_inputs_then_native_config_entries_are_called(
    test_case: ModelConfigTierTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    calls, _ = model_config_outcome(project_dir=tmp_path / "orders", monkeypatch=monkeypatch)

    assert calls == test_case.expected_native_calls


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
