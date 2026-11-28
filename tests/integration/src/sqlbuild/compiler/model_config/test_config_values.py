"""Config values the native build gives models: builtin, subclassed and opaque Python values."""

from __future__ import annotations

from datetime import date
from pathlib import Path

import pytest

from tests.integration.src.sqlbuild.compiler.model_config._test_types import (
    ConfigValueBuildTestCase,
    ConfigValueRejectionTestCase,
    OpaqueConfigValueTestCase,
)
from tests.integration.src.sqlbuild.compiler.model_config.helpers import (
    MARKER,
    SettingMap,
    Severity,
    TagList,
    config_value_inputs,
    config_value_outcome,
    marker_kept,
)


@pytest.mark.parametrize(
    "test_case",
    [
        ConfigValueBuildTestCase(
            description="builtin values, unions and templates",
            header={
                "description": "Orders.",
                "tags": ["${region}-${ratio}", "core"],
                "row_diff_exclude_columns": ["updated_at"],
                "table_settings": {"owner": "${CTX:model.name}", "weight": 10**30, "enabled": True},
            },
            expected_values=(
                ("'materialized'", "str", "'table'"),
                ("'contract'", "str", "'enforced'"),
                ("'tags'", "list", "['core', 'mart', 'north-0.5']"),
                ("'row_diff_exclude_columns'", "tuple", "('loaded_at', 'updated_at')"),
                (
                    "'table_settings'",
                    "dict",
                    "{'owner': 'orders', 'weight': 1000000000000000000000000000000, 'enabled': True}",
                ),
                ("'schema'", "str", "'north_orders'"),
            ),
            expected_header_keys=(
                "description",
                "row_diff_exclude_columns",
                "table_settings",
                "tags",
            ),
        ),
        ConfigValueBuildTestCase(
            description="subclasses, str enums, dates and opaque values",
            header={
                "description": "Orders.",
                "tags": TagList(["${region}", Severity.WARN]),
                "table_settings": SettingMap(owner="${CTX:model.name}", level=Severity.WARN),
                "row_diff_tolerances": {"by_type": {"DOUBLE": 0.5}, "default": date(2024, 1, 2)},
                Severity.WARN: MARKER,
            },
            expected_values=(
                ("'materialized'", "str", "'table'"),
                ("'contract'", "str", "'enforced'"),
                ("'tags'", "list", "['core', 'mart', 'north', 'warn']"),
                ("'row_diff_exclude_columns'", "list", "['loaded_at']"),
                (
                    "'table_settings'",
                    "dict",
                    "{'owner': 'orders', 'level': <Severity.WARN: 'warn'>}",
                ),
                (
                    "'row_diff_tolerances'",
                    "dict",
                    "{'by_type': {'DOUBLE': 0.5}, 'default': datetime.date(2024, 1, 2)}",
                ),
                ("<Severity.WARN: 'warn'>", "Marker", "Marker()"),
                ("'schema'", "str", "'north_orders'"),
            ),
            expected_header_keys=(
                "description",
                "row_diff_tolerances",
                "table_settings",
                "tags",
                "warn",
            ),
        ),
        ConfigValueBuildTestCase(
            description="target context and whole-value templates",
            header={
                "database": "db",
                "alias": "${CTX:destination.qualified}",
                "row_diff_sample_seed": "${ratio}",
            },
            expected_values=(
                ("'materialized'", "str", "'table'"),
                ("'contract'", "str", "'enforced'"),
                ("'tags'", "list", "['core', 'mart']"),
                ("'row_diff_exclude_columns'", "list", "['loaded_at']"),
                ("'database'", "str", "'db'"),
                ("'alias'", "str", "'db.north_orders.${CTX:destination.qualified}'"),
                ("'row_diff_sample_seed'", "str", "'0.5'"),
                ("'schema'", "str", "'north_orders'"),
            ),
            expected_header_keys=("alias", "database", "row_diff_sample_seed"),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_given_header_when_building_config_then_values_keep_types_and_order(
    test_case: ConfigValueBuildTestCase, tmp_path: Path
) -> None:
    outcome: tuple[tuple[tuple[str, str, str], ...], tuple[str, ...]] = config_value_outcome(
        project_dir=tmp_path / "orders", header=test_case.header
    )

    assert outcome == (test_case.expected_values, test_case.expected_header_keys)


@pytest.mark.parametrize(
    "test_case",
    [
        ConfigValueRejectionTestCase(
            description="a lone surrogate in a templated config",
            header={"description": "x\udc80", "alias": "${region}"},
            expected_error=(
                "UnicodeEncodeError",
                "'utf-8' codec can't encode character '\\udc80' in position 1: surrogates not allowed",
            ),
        ),
        ConfigValueRejectionTestCase(
            description="a hook list of plain strings",
            header={"pre_hooks": ["select 1"]},
            expected_error=(
                "CompileInputError",
                "model 'orders' pre_hooks entries must use typed inline_sql(...), sql(...), or python(...) hook syntax",
            ),
        ),
        ConfigValueRejectionTestCase(
            description="a missing hook list",
            header={"post_hooks": None},
            expected_error=(
                "CompileInputError",
                "model 'orders' post_hooks must be a list of typed hook entries",
            ),
        ),
        ConfigValueRejectionTestCase(
            description="a non-string tag",
            header={"tags": ["a", 1]},
            expected_error=("CompileInputError", "model 'orders' tags entries must be strings"),
        ),
        ConfigValueRejectionTestCase(
            description="a non-string header key",
            header={1: "x"},
            expected_error=("TypeError", "'int' object is not an instance of 'str'"),
        ),
        ConfigValueRejectionTestCase(
            description="a nested macro call",
            header={"table_settings": {"a": ["@py.x ("]}},
            expected_error=(
                "CompileInputError",
                "model config field 'table_settings.a' does not allow macros",
            ),
        ),
        ConfigValueRejectionTestCase(
            description="an unknown variable",
            header={"alias": "${missing}"},
            expected_error=(
                "CompileInputError",
                "model config references unknown variable 'missing'",
            ),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_invalid_header_when_building_config_then_the_build_raises_as_before(
    test_case: ConfigValueRejectionTestCase, tmp_path: Path
) -> None:
    with pytest.raises(Exception) as raised:
        config_value_inputs(project_dir=tmp_path / "orders", header=test_case.header)

    assert (type(raised.value).__name__, str(raised.value)) == test_case.expected_error


@pytest.mark.parametrize(
    "test_case",
    [
        OpaqueConfigValueTestCase(
            description="an object under a str-enum key",
            header={"description": "x", Severity.WARN: MARKER},
            key=Severity.WARN,
            expected_same_object=True,
        ),
        OpaqueConfigValueTestCase(
            description="an object beside templated values",
            header={"alias": "${region}", "owner": MARKER},
            key="owner",
            expected_same_object=True,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_opaque_config_value_when_building_config_then_the_same_object_reaches_the_model(
    test_case: OpaqueConfigValueTestCase, tmp_path: Path
) -> None:
    kept: bool = marker_kept(
        project_dir=tmp_path / "orders", header=test_case.header, key=test_case.key
    )

    assert kept == test_case.expected_same_object
