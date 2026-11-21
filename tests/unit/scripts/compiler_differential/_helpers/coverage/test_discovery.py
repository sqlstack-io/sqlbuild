"""Discovery captures are classified into the input kinds the seed corpus must exercise."""

from __future__ import annotations

import dataclasses
import json
from pathlib import Path

import pytest

from scripts.compiler_differential._helpers.coverage.discovery import (
    discovered_input_kinds,
    discovery_capture_problems,
    project_discovery_kinds,
    required_discovery_kinds,
)
from scripts.compiler_differential.constants import DISCOVERY_NON_COLLECTION_FIELDS
from sqlbuild._native import (
    DiscoveredProjectInputs,
    LocalConfig,
    ProjectConfig,
    render_stage_capture,
)
from tests.unit.scripts.compiler_differential._helpers.coverage._test_types import (
    AuthoredByteKindsTestCase,
    CaptureProblemsTestCase,
    DiscoveryKindsTestCase,
    EncodedInputsKindsTestCase,
    RequiredKindsTestCase,
)
from tests.unit.scripts.compiler_differential._helpers.coverage.helpers import (
    constant_declaration,
    empty_inputs_capture,
)

_SCOPE: str = "sqlbuild.compiler.scopes.types:ScopeKind"


@pytest.mark.parametrize(
    "test_case",
    [
        DiscoveryKindsTestCase(
            description="collections_and_model_strategies",
            capture={
                "project_config": {"adapter": "duckdb", "path_defaults": {"staging": {}}},
                "local_config": {"target": None, "vars": {}},
                "model_files": [
                    {
                        "header_values": {
                            "materialized": "incremental",
                            "incremental_strategy": "merge",
                        }
                    },
                    {
                        "header_values": {
                            "materialized": "incremental",
                            "incremental_mode": "microbatch",
                            "microbatch_strategy": "watermark",
                        }
                    },
                    {"header_values": {"materialized": "snapshot", "snapshot_strategy": "check"}},
                    {"header_values": {"materialized": "copy_table"}, "contents": "-- café\n"},
                ],
                "materialization_files": [{"name": "copy_table"}],
                "adapter_file": {"relative_path": {"__path__": "adapter.py"}},
            },
            expected_present=frozenset(
                {
                    "model_files",
                    "materialization_files",
                    "adapter_file",
                    "incremental_merge",
                    "microbatch_watermark",
                    "snapshot_check",
                    "custom_materialized_model",
                    "path_defaults",
                    "non_ascii_comment",
                }
            ),
            expected_absent=frozenset(
                {
                    "incremental_delete_insert",
                    "local_config",
                    "project_adapter_config",
                    "enum_files",
                }
            ),
        ),
        DiscoveryKindsTestCase(
            description="declarations_tests_and_config",
            capture={
                "project_config": {
                    "adapter": "orders_duckdb",
                    "targets": {"dev": {}, "ci": {}},
                    "dbt": {"target_path": "dbt/artifacts"},
                },
                "local_config": {"target": "ci", "vars": {"region": "north"}},
                "constant_files": [
                    {
                        "scope_kind": {"__enum__": f"{_SCOPE}.INHERITED"},
                        "contents": "\ufeffCONSTANT (name labels, value ['東京']);\n",
                        "declarations": [
                            constant_declaration(kind="LIST", value=["東京"]),
                            constant_declaration(kind="FLOAT", value=0.5),
                        ],
                    }
                ],
                "enum_files": [{"declarations": [{"members": [{"name": "LOW", "value": 1}]}]}],
                "macro_files": [
                    {"contents": "from macros.python.rounding import round_amount\n"},
                ],
                "test_files": [
                    {
                        "contents": "TEST(\n\tmode macro);",
                        "blocks": [
                            {
                                "mode": {
                                    "__enum__": "sqlbuild.compiler.compile.types:SqlTestMode.MACRO"
                                },
                                "cases": [{"name": "placed"}],
                            }
                        ],
                    }
                ],
                "audit_files": [
                    {
                        "declaration_kind": {
                            "__enum__": "sqlbuild.compiler.scopes.types:DeclarationKind.SINGULAR_AUDIT"
                        }
                    }
                ],
                "source_files": [{"source_entries": [{"managed": True}, {"managed": False}]}],
            },
            expected_present=frozenset(
                {
                    "scope_inherited",
                    "constant_list",
                    "constant_float",
                    "non_ascii_constant",
                    "enum_integer",
                    "cross_file_macro_import",
                    "test_mode_macro",
                    "parameterized_test",
                    "singular_audit",
                    "managed_source",
                    "unmanaged_source",
                    "local_config",
                    "target_override",
                    "dbt_target_path_config",
                    "project_adapter_config",
                    "bom",
                    "tab",
                    "non_ascii_string",
                }
            ),
            expected_absent=frozenset({"generic_audit", "enum_string", "scope_global"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_discovery_capture_when_classifying_then_kinds_follow_discovered_facts(
    test_case: DiscoveryKindsTestCase,
) -> None:
    kinds: frozenset[str] = discovered_input_kinds(json.dumps(test_case.capture))

    assert test_case.expected_present <= kinds
    assert not test_case.expected_absent & kinds


@pytest.mark.parametrize(
    "test_case",
    [
        EncodedInputsKindsTestCase(
            description="project_without_any_optional_input",
            inputs=DiscoveredProjectInputs(
                project_config=ProjectConfig(name="orders", adapter="duckdb"),
                local_config=LocalConfig(),
            ),
            expected_kinds=frozenset(),
        ),
        EncodedInputsKindsTestCase(
            description="local_config_with_only_an_empty_override_set",
            inputs=DiscoveredProjectInputs(
                project_config=ProjectConfig(name="orders", adapter="duckdb"),
                local_config=LocalConfig(setting_overrides=frozenset()),
            ),
            expected_kinds=frozenset(),
        ),
        EncodedInputsKindsTestCase(
            description="local_config_with_a_setting_override",
            inputs=DiscoveredProjectInputs(
                project_config=ProjectConfig(name="orders", adapter="duckdb"),
                local_config=LocalConfig(setting_overrides=frozenset({"sql_analysis"})),
            ),
            expected_kinds=frozenset({"local_config"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_encoded_inputs_when_classifying_then_encoded_empty_values_prove_nothing(
    test_case: EncodedInputsKindsTestCase,
) -> None:
    assert discovered_input_kinds(render_stage_capture(test_case.inputs)) == (
        test_case.expected_kinds
    )


@pytest.mark.parametrize(
    "test_case",
    [
        AuthoredByteKindsTestCase(
            description="crlf_in_discovered_model",
            files={"models/orders.sql": b"MODEL ();\r\nSELECT 1\r\n"},
            read_paths=("models/orders.sql",),
            expected_crlf=True,
        ),
        AuthoredByteKindsTestCase(
            description="lf_only",
            files={"models/orders.sql": b"MODEL ();\nSELECT 1\n"},
            read_paths=("models/orders.sql",),
            expected_crlf=False,
        ),
        AuthoredByteKindsTestCase(
            description="crlf_only_in_a_file_discovery_did_not_read",
            files={
                "models/orders.sql": b"MODEL ();\nSELECT 1\n",
                "notes/readme.txt": b"orders\r\n",
            },
            read_paths=("models/orders.sql",),
            expected_crlf=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_authored_bytes_when_classifying_then_crlf_counts_only_for_discovered_files(
    test_case: AuthoredByteKindsTestCase, tmp_path: Path
) -> None:
    for relative_path, contents in test_case.files.items():
        path: Path = tmp_path / relative_path
        path.parent.mkdir(parents=True, exist_ok=True)
        _ = path.write_bytes(contents)
    capture: Path = tmp_path / "captures" / "001-discovered_project_inputs.json"
    capture.parent.mkdir()
    _ = capture.write_text(
        json.dumps(
            empty_inputs_capture(
                model_files=[{"relative_path": {"__path__": path}} for path in test_case.read_paths]
            )
        ),
        encoding="utf-8",
    )

    kinds: frozenset[str] = project_discovery_kinds(
        captures={"0-compile": {capture.name: capture}}, source_dir=tmp_path
    )

    assert ("crlf" in kinds) is test_case.expected_crlf


@pytest.mark.parametrize(
    "test_case",
    [
        RequiredKindsTestCase(
            description="every_discovered_collection",
            expected_required=frozenset(
                field.name for field in dataclasses.fields(DiscoveredProjectInputs)
            )
            - DISCOVERY_NON_COLLECTION_FIELDS,
        )
    ],
    ids=lambda case: case.description,
)
def test_given_discovered_project_inputs_when_listing_required_kinds_then_collections_are_required(
    test_case: RequiredKindsTestCase,
) -> None:
    assert test_case.expected_required <= set(required_discovery_kinds())


@pytest.mark.parametrize(
    "test_case",
    [
        CaptureProblemsTestCase(
            description="sound_capture",
            capture=empty_inputs_capture(
                asset_functions=[{"function": {"__callable__": "python.assets.orders:export"}}]
            ),
            expected_problem_count=0,
        ),
        CaptureProblemsTestCase(
            description="opaque_value_and_unnamed_callable",
            capture=empty_inputs_capture(
                asset_functions=[{"function": {"__opaque__": "orders:Exporter"}}]
            ),
            expected_problem_count=2,
        ),
        CaptureProblemsTestCase(
            description="missing_field",
            capture={"__type__": "DiscoveredProjectInputs", "model_files": []},
            expected_problem_count=1,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_discovery_capture_when_checking_then_incomplete_or_opaque_parts_are_reported(
    test_case: CaptureProblemsTestCase,
) -> None:
    problems: tuple[str, ...] = discovery_capture_problems(json.dumps(test_case.capture))

    assert len(problems) == test_case.expected_problem_count, problems


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
