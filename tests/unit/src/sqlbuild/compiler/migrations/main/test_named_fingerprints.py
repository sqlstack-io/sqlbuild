"""Unit coverage for finding a model's newest fingerprint by name across inspected schemas."""

from __future__ import annotations

import duckdb
import pytest

from sqlbuild._native import Fingerprint, MigrationStateInspection, live_reads, write_fingerprints
from sqlbuild.adapters.duckdb.classes.duckdb_adapter import DuckDbAdapter
from tests.unit.src.sqlbuild.compiler.migrations.main._test_types import NamedFingerprintTestCase
from tests.unit.src.sqlbuild.compiler.migrations.main.helpers import built_fingerprint

_FINGERPRINTS: dict[str, tuple[Fingerprint, ...]] = {
    "main": (
        built_fingerprint("model", "orders", "main", 2),
        built_fingerprint("model", "customers", "main", 5),
        built_fingerprint("seed", "currencies", "main", 3),
    ),
    "archive": (
        built_fingerprint("model", "orders", "archive", 4),
        built_fingerprint("model", "customers", "archive", 1),
    ),
}


@pytest.mark.parametrize(
    "test_case",
    [
        NamedFingerprintTestCase(
            description="a model built in two schemas has its newest build in the later schema",
            model_name="orders",
            expected_targets=("archive.orders",),
        ),
        NamedFingerprintTestCase(
            description="a model built in two schemas has its newest build in the earlier schema",
            model_name="customers",
            expected_targets=("main.customers",),
        ),
        NamedFingerprintTestCase(
            description="a name in another case is another model",
            model_name="Orders",
            expected_targets=(),
        ),
        NamedFingerprintTestCase(
            description="a seed's fingerprint is not a model's",
            model_name="currencies",
            expected_targets=(),
        ),
        NamedFingerprintTestCase(
            description="a model never built has none",
            model_name="refunds",
            expected_targets=(),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_inspected_schemas_when_a_model_is_named_then_its_newest_fingerprint_is_found(
    test_case: NamedFingerprintTestCase,
) -> None:
    adapter: DuckDbAdapter = DuckDbAdapter()
    connection: duckdb.DuckDBPyConnection = duckdb.connect(":memory:")
    _ = connection.execute("CREATE SCHEMA archive")
    schema: str
    fingerprints: tuple[Fingerprint, ...]
    for schema, fingerprints in _FINGERPRINTS.items():
        write_fingerprints(
            connection=connection,
            execute=adapter.execute,
            database=None,
            schema=schema,
            fingerprints=fingerprints,
            render_qualified_name=adapter.render_qualified_name,
            render_framework_type=adapter.render_framework_type,
        )
    inspection: MigrationStateInspection = MigrationStateInspection(
        reads=live_reads(adapter=adapter, connection=connection), database=None
    )

    inspection.inspect_schemas(schemas={"main"})
    inspection.inspect_schemas(schemas={"archive"})

    found: Fingerprint | None = inspection.named_fingerprint(model_name=test_case.model_name)
    assert (
        tuple(
            f"{fingerprint.target_schema}.{fingerprint.target_name}"
            for fingerprint in filter(None, (found,))
        )
        == test_case.expected_targets
    )
