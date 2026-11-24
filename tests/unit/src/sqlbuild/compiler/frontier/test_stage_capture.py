"""Stage captures render frontier objects as deterministic, comparable JSON."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

import pytest

from scripts.compiler_differential._helpers.comparing.capture_text import expand_capture_text
from sqlbuild._native import (
    STAGE_CAPTURE_DIR_ENV_VAR,
    CompilerStage,
    compile_frontier,
    render_stage_capture,
)
from tests.unit.src.sqlbuild.compiler.frontier._test_types import (
    FrontierCaptureTestCase,
    OmittedFieldTestCase,
    SharedCaptureTestCase,
    StageCaptureOrderTestCase,
    StageCaptureTestCase,
)
from tests.unit.src.sqlbuild.compiler.frontier.helpers import (
    HELPERS_MODULE,
    declaration_scope_build,
    linked_customer,
    memo_catalog,
    order_line,
    order_total,
    orders_binding_catalog,
    orders_lineage,
    repeated_orders,
)

STAGE_CAPTURE_SHARED_NODES_KEY: str = "__shared_nodes__"

_COMPILE_MODELS: str = "sqlbuild.compiler.compile.models"
_COMPILE_TYPES: str = "sqlbuild.compiler.compile.types"
_LINEAGE_TYPES: str = "sqlbuild.compiler.lineage.types"


@pytest.mark.parametrize(
    "test_case",
    [
        StageCaptureTestCase(
            description="dataclass_enum_and_path",
            value=order_line,
            expected_capture={
                "__type__": f"{HELPERS_MODULE}:OrderLine",
                "order_id": 7,
                "status": {"__enum__": f"{HELPERS_MODULE}:OrderStatus.PLACED"},
                "path": {"__path__": "sql/models/a.sql"},
            },
        ),
        StageCaptureTestCase(
            description="callable_becomes_qualified_name",
            value=lambda: {"total": order_total},
            expected_capture={"total": {"__callable__": f"{HELPERS_MODULE}:order_total"}},
        ),
        StageCaptureTestCase(
            description="sets_are_sorted",
            value=lambda: frozenset({"orders", "customers", "products"}),
            expected_capture={"__set__": ["customers", "orders", "products"]},
        ),
        StageCaptureTestCase(
            description="non_string_keys_are_pairs_in_insertion_order",
            value=lambda: {2: "second", 1: "first"},
            expected_capture={"__mapping__": [[2, "second"], [1, "first"]]},
        ),
        StageCaptureTestCase(
            description="cycles_are_cut",
            value=linked_customer,
            expected_capture={
                "__type__": f"{HELPERS_MODULE}:LinkedCustomer",
                "name": "customer",
                "peer": {"__cycle__": f"{HELPERS_MODULE}:LinkedCustomer"},
            },
        ),
        StageCaptureTestCase(
            description="compact_lineage_is_captured_as_decoded_facts",
            value=lambda: orders_lineage(
                string_pool=("customers", "order_id", "source", "orders"), source_index=3
            ),
            expected_capture=[
                {
                    "__type__": f"{_COMPILE_MODELS}:CompiledLineageColumnFact",
                    "output_column": "order_id",
                    "upstream_columns": [
                        {
                            "__type__": f"{_COMPILE_MODELS}:CompiledLineageSourceFact",
                            "resource_type": {
                                "__enum__": f"{_COMPILE_TYPES}:CompiledResourceType.SOURCE"
                            },
                            "resource_name": "orders",
                            "column_name": "order_id",
                        }
                    ],
                    "transform_kind": {"__enum__": f"{_LINEAGE_TYPES}:ColumnTransformKind.DIRECT"},
                    "confidence": {"__enum__": f"{_LINEAGE_TYPES}:ColumnLineageConfidence.HIGH"},
                }
            ],
        ),
        StageCaptureTestCase(
            description="mapping_with_a_reserved_key_is_encoded_as_pairs",
            value=lambda: {"__shared__": "orders", "status": "placed"},
            expected_capture={"__mapping__": [["__shared__", "orders"], ["status", "placed"]]},
        ),
        StageCaptureTestCase(
            description="non_finite_float_and_bytes",
            value=lambda: (float("inf"), b"orders"),
            expected_capture=[
                {"__float__": "inf"},
                {
                    "__bytes__": 6,
                    "sha256": "1c168adb00d208e42f93314529f1fa9c0427eb63233ceda95a5db52b7012a719",
                },
            ],
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_frontier_value_when_rendering_capture_then_json_is_canonical(
    test_case: StageCaptureTestCase,
) -> None:
    assert json.loads(render_stage_capture(test_case.value())) == test_case.expected_capture


@pytest.mark.parametrize(
    "test_case",
    [
        StageCaptureOrderTestCase(
            description="mapping_insertion_order_is_visible",
            first=lambda: {"orders": 1, "customers": 2},
            second=lambda: {"customers": 2, "orders": 1},
            expected_identical=False,
        ),
        StageCaptureOrderTestCase(
            description="non_string_key_insertion_order_is_visible",
            first=lambda: {1: "orders", 2: "customers"},
            second=lambda: {2: "customers", 1: "orders"},
            expected_identical=False,
        ),
        StageCaptureOrderTestCase(
            description="tuple_order_is_visible",
            first=lambda: ("orders", "customers"),
            second=lambda: ("customers", "orders"),
            expected_identical=False,
        ),
        StageCaptureOrderTestCase(
            description="set_order_is_not_visible",
            first=lambda: frozenset(("orders", "customers", "products")),
            second=lambda: frozenset(("products", "customers", "orders")),
            expected_identical=True,
        ),
        StageCaptureOrderTestCase(
            description="batch_string_pool_layout_is_not_visible",
            first=lambda: orders_lineage(
                string_pool=("order_id", "source", "orders"), source_index=2
            ),
            second=lambda: orders_lineage(
                string_pool=("customers", "source", "orders", "amount", "order_id"),
                source_index=2,
            ),
            expected_identical=True,
        ),
        StageCaptureOrderTestCase(
            description="decoded_lineage_source_is_visible",
            first=lambda: orders_lineage(
                string_pool=("order_id", "source", "orders", "customers"), source_index=2
            ),
            second=lambda: orders_lineage(
                string_pool=("order_id", "source", "orders", "customers"), source_index=3
            ),
            expected_identical=False,
        ),
        StageCaptureOrderTestCase(
            description="binding_catalog_relation_order_is_not_visible",
            first=lambda: orders_binding_catalog(relations=("orders", "customers")),
            second=lambda: orders_binding_catalog(relations=("customers", "orders")),
            expected_identical=True,
        ),
        StageCaptureOrderTestCase(
            description="undeclared_attribute_map_order_stays_visible",
            first=lambda: memo_catalog(memo_keys=("orders", "customers"), ordered_keys=()),
            second=lambda: memo_catalog(memo_keys=("customers", "orders"), ordered_keys=()),
            expected_identical=False,
        ),
        StageCaptureOrderTestCase(
            description="binding_catalog_relations_stay_visible",
            first=lambda: orders_binding_catalog(relations=("orders", "customers")),
            second=lambda: orders_binding_catalog(relations=("orders",)),
            expected_identical=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_reordered_value_when_rendering_capture_then_only_unordered_collections_match(
    test_case: StageCaptureOrderTestCase,
) -> None:
    assert (
        render_stage_capture(test_case.first()) == render_stage_capture(test_case.second())
    ) is test_case.expected_identical


@pytest.mark.parametrize(
    "test_case",
    [
        OmittedFieldTestCase(
            description="declared_process_handle_field_is_left_out",
            value=declaration_scope_build,
            expected_capture={
                "__type__": f"{_COMPILE_MODELS}:DeclarationScopeBuild",
                "loaded_macros": {},
                "index": "orders_index",
                "resolver": "orders_resolver",
            },
        ),
        OmittedFieldTestCase(
            description="undeclared_dataclass_keeps_every_field",
            value=order_line,
            expected_capture={
                "__type__": f"{HELPERS_MODULE}:OrderLine",
                "order_id": 7,
                "status": {"__enum__": f"{HELPERS_MODULE}:OrderStatus.PLACED"},
                "path": {"__path__": "sql/models/a.sql"},
            },
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_declared_omitted_field_when_rendering_dataclass_capture_then_it_is_left_out(
    test_case: OmittedFieldTestCase,
) -> None:
    capture: object = json.loads(render_stage_capture(test_case.value()))

    assert capture == test_case.expected_capture


@pytest.mark.parametrize(
    "test_case",
    [
        SharedCaptureTestCase(
            description="repeated_lines_and_sql_are_stored_once",
            value=repeated_orders,
            expected_shared_nodes=2,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_repeated_large_subtrees_when_rendering_capture_then_each_is_stored_once(
    test_case: SharedCaptureTestCase,
) -> None:
    text: str = render_stage_capture(test_case.value())

    shared: dict[str, object] = json.loads(text)[STAGE_CAPTURE_SHARED_NODES_KEY]
    assert len(shared) == test_case.expected_shared_nodes
    assert expand_capture_text(text) == test_case.value()
    assert {
        hashlib.sha256(
            json.dumps(node, ensure_ascii=False, separators=(",", ":")).encode()
        ).hexdigest()
        for node in shared.values()
    } == set(shared)
    assert len(text.splitlines()) == test_case.expected_shared_nodes + 4


@pytest.mark.parametrize(
    "test_case",
    [
        FrontierCaptureTestCase(
            description="discovery and compiled project",
            stages=(CompilerStage.DISCOVERED_PROJECT_INPUTS, CompilerStage.COMPILED_PROJECT),
            expected_results=(
                "discovered_project_inputs-result",
                "compiled_project-result",
            ),
            expected_files=("discovered_project_inputs.json", "compiled_project.json"),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_capture_directory_when_compiling_frontier_then_each_stage_is_captured_in_order(
    test_case: FrontierCaptureTestCase, tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setenv(STAGE_CAPTURE_DIR_ENV_VAR, str(tmp_path))

    results: tuple[object, ...] = tuple(
        compile_frontier(until=stage, python_stage=lambda stage=stage: f"{stage.value}-result")
        for stage in test_case.stages
    )

    captures: list[Path] = sorted(tmp_path.iterdir())
    assert results == test_case.expected_results
    assert tuple(path.name.split("-", 1)[1] for path in captures) == test_case.expected_files
    assert tuple(json.loads(path.read_text(encoding="utf-8")) for path in captures) == (
        test_case.expected_results
    )


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
