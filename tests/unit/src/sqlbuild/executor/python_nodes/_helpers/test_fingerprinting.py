"""Tests for Python-node identity fingerprint writes."""

from __future__ import annotations

import pytest

from sqlbuild._native import PythonNodeIdentity
from sqlbuild.executor.python_nodes._helpers.fingerprinting import (
    try_write_python_node_identity_fingerprint,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers._test_types import (
    PythonIdentityFingerprintWriteTestCase,
)
from tests.unit.src.sqlbuild.executor.python_nodes._helpers.helpers import (
    PythonNodeContextTestAdapter,
)


@pytest.mark.parametrize(
    "test_case",
    [
        PythonIdentityFingerprintWriteTestCase(
            description="writes identity payloads into fingerprint state",
            schema="analytics",
            expected_sql_count=2,
            expected_fragments=(
                "_sqlbuild_fingerprints",
                "asset",
                "build_orders",
                "definition_b64",
                "metadata_json_b64",
                "definition-hash",
                "version-hash",
            ),
        ),
        PythonIdentityFingerprintWriteTestCase(
            description="skips write without target schema",
            schema=None,
            expected_sql_count=0,
            expected_fragments=(),
            unexpected_fragments=("_sqlbuild_fingerprints",),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_python_identity_when_writing_fingerprint_then_uses_fingerprint_state(
    test_case: PythonIdentityFingerprintWriteTestCase,
) -> None:
    adapter: PythonNodeContextTestAdapter = PythonNodeContextTestAdapter()

    try_write_python_node_identity_fingerprint(
        identity=PythonNodeIdentity(
            node_type="asset",
            node_name="build_orders",
            source_path="python/tasks/orders.py",
            source_hash="source-hash",
            definition_hash="definition-hash",
            version_hash="version-hash",
            definition_json='{"source_text":"node source"}',
            metadata_json='{"dependencies":[{"source_text":"helper source"}]}',
        ),
        adapter=adapter,
        connection=object(),
        run_id="run_1",
        database="warehouse",
        schema=test_case.schema,
    )

    rendered_sql: str = "\n".join(adapter.executed_sql)
    assert len(adapter.executed_sql) == test_case.expected_sql_count
    for fragment in test_case.expected_fragments:
        assert fragment in rendered_sql
    for fragment in test_case.unexpected_fragments:
        assert fragment not in rendered_sql
