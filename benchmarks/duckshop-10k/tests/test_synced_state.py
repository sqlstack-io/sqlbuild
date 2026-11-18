"""Offline checks of the synced warehouse state the plan preview guard mirrors."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any

import pytest

HERE = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(HERE))

from synced_state import inputs, mirror_batches, planned_identities  # noqa: E402

_METADATA: str = '{"declared_columns_hash":"columns","model_name":"orders"}'

NODES: dict[str, dict[str, Any]] = {
    "seed:countries": {"deps": [], "columns": [{"name": "code", "type": "VARCHAR"}]},
    "source:raw_orders": {
        "deps": [],
        "database": None,
        "schema": "RAW",
        "table": "orders",
        "columns": [{"name": "id", "type": "NUMBER"}],
    },
    "udf:to_cents": {"deps": ["seed:countries"]},
    "model:stg_orders": {
        "deps": ["source:raw_orders"],
        "materialization": "view",
        "config": {},
        "columns": [{"name": "id", "type": "NUMBER"}],
    },
    "model:orders": {
        "deps": ["model:stg_orders", "udf:to_cents"],
        "materialization": "incremental",
        "config": {"cursor_type": "timestamp"},
        "columns": [{"name": "id", "type": "NUMBER"}],
    },
}


def identity(name: str) -> dict[str, str]:
    return {
        "definition_hash": f"definition-{name}",
        "version_hash": f"version-{name}",
        "metadata_json": "{}",
        "definition": f"SELECT 1 AS {name}",
        "local_hash": f"local-{name}",
    }


IDENTITIES: dict[str, dict[str, str]] = {
    key: identity(key.split(":", 1)[1])
    for key in ("seed:countries", "model:stg_orders", "model:orders")
}


def test_given_a_capture_when_reading_identities_then_a_model_hashes_metadata_without_its_columns(
    tmp_path: Path,
) -> None:
    capture: Path = tmp_path / "001-plan_output.json"
    capture.write_text(
        json.dumps(
            {
                "__shared_nodes__": {"metadata": _METADATA},
                "__capture__": [
                    {
                        "__type__": "sqlbuild.compiler.planner.models:ModelPlanEntry",
                        "name": "orders",
                        "fingerprint_metadata_json": {"__shared__": "metadata"},
                        "fingerprint_definition_hash": "query",
                        "fingerprint_version_hash": "version",
                        "fingerprint_query_sql": "SELECT 1",
                    }
                ],
            }
        )
    )

    found: dict[str, dict[str, str]] = planned_identities(capture)

    assert (
        found["model:orders"]["local_hash"]
        == hashlib.sha256(b'query\n{"model_name":"orders"}').hexdigest()
    )
    assert found["model:orders"]["metadata_json"] == _METADATA


@pytest.mark.parametrize(
    ("description", "node", "expected_inputs"),
    [
        ("a source and an upstream model", "model:stg_orders", [("source", "raw_orders")]),
        (
            "a function's own inputs, not the function",
            "model:orders",
            [("node", "model:stg_orders"), ("node", "seed:countries")],
        ),
    ],
)
def test_given_a_node_when_listing_its_inputs_then_functions_expand_to_what_they_read(
    description: str, node: str, expected_inputs: list[tuple[str, str]]
) -> None:
    assert inputs(NODES[node], NODES) == expected_inputs, description


@pytest.mark.parametrize(
    ("description", "built_share", "expected_fingerprints", "expected_relations"),
    [
        ("every model built", 1.0, ["countries", "orders", "stg_orders"], 159),
        ("no model built", 0.0, ["countries"], 157),
    ],
)
def test_given_built_models_when_mirroring_then_each_executes_after_what_it_reads_with_its_hashes(
    description: str, built_share: float, expected_fingerprints: list[str], expected_relations: int
) -> None:
    batches = mirror_batches(NODES, IDENTITIES, built_share=built_share, changed_share=0.0, seed=1)

    succeeded: list[dict[str, Any]] = [
        row for row in batches["consumption"] if row["phase"] == "succeeded"
    ]
    executions: dict[str, str] = {row["node_name"]: row["execution_id"] for row in succeeded}
    reads: dict[tuple[str, str | None], str | None] = {
        (row["node_name"], row["input_name"]): row["input_version"] for row in succeeded
    }
    assert sorted(row["node_name"] for row in batches["fingerprints"]) == expected_fingerprints, (
        description
    )
    assert len(batches["catalog"]) == expected_relations, description
    assert {row["version_hash"] for row in succeeded} == {
        f"local-{name}" for name in expected_fingerprints
    }
    if built_share:
        assert reads[("orders", "model:stg_orders")] == executions["stg_orders"], description
        assert reads[("orders", "seed:countries")] == executions["countries"], description
