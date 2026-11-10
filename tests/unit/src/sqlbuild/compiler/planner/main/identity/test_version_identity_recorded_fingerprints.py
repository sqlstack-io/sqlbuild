import hashlib
import json
from dataclasses import replace

import pytest

from sqlbuild._native import (
    CompiledFunction,
    DirectModelVersionIdentities,
    WarehouseFingerprints,
    build_compiled_function_fingerprint_sql,
    compute_query_hash,
)
from tests.unit.src.sqlbuild.compiler.planner._helpers.helpers import build_compiled_function
from tests.unit.src.sqlbuild.compiler.planner.main.identity._test_types import (
    RecordedFingerprintGraphTestCase,
    RecordedFunctionHashTestCase,
)
from tests.unit.src.sqlbuild.compiler.planner.main.identity.helpers import (
    build_function_identities,
    build_two_model_graph_identities,
    recorded_fingerprint,
)

_ORDERS_SQL: str = "SELECT 1 AS order_id"
_SUMMARY_SQL: str = 'SELECT count(*) AS orders FROM __ref("orders")'


@pytest.mark.parametrize(
    "test_case",
    [
        RecordedFingerprintGraphTestCase(
            description="unchanged upstream keeps its recorded fingerprint under an edited downstream",
            orders_sql=_ORDERS_SQL,
            summary_sql=_SUMMARY_SQL,
            recorded_definitions={
                "orders": "select 1  as order_id -- staged",
                "orders_summary": 'SELECT count(*) AS order_count FROM __ref("orders")',
            },
            expected_kept=frozenset({"orders"}),
        ),
        RecordedFingerprintGraphTestCase(
            description="unchanged downstream keeps its recorded fingerprint under an edited upstream",
            orders_sql=_ORDERS_SQL,
            summary_sql=_SUMMARY_SQL,
            recorded_definitions={
                "orders": "SELECT 2 AS order_id",
                "orders_summary": _SUMMARY_SQL,
            },
            expected_kept=frozenset({"orders_summary"}),
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_old_scheme_records_when_hashing_graph_then_only_unchanged_models_keep_them(
    test_case: RecordedFingerprintGraphTestCase,
) -> None:
    recorded: WarehouseFingerprints = WarehouseFingerprints(
        models={
            name: recorded_fingerprint(
                name=name,
                node_type="model",
                definition=definition,
                definition_hash=f"old_scheme_{name}",
            )
            for name, definition in test_case.recorded_definitions.items()
        }
    )
    current: DirectModelVersionIdentities = build_two_model_graph_identities(
        orders_sql=test_case.orders_sql, summary_sql=test_case.summary_sql, recorded=None
    )

    baselined: DirectModelVersionIdentities = build_two_model_graph_identities(
        orders_sql=test_case.orders_sql, summary_sql=test_case.summary_sql, recorded=recorded
    )

    queries: dict[str, str] = {
        "orders": test_case.orders_sql,
        "orders_summary": test_case.summary_sql,
    }
    for name, query_sql in queries.items():
        query_fingerprint: str = {
            True: f"old_scheme_{name}",
            False: compute_query_hash(query_sql=query_sql, dialect=None),
        }[name in test_case.expected_kept]
        expected: str = hashlib.sha256(
            f"{query_fingerprint}\n{baselined.model_metadata_jsons[name]}".encode()
        ).hexdigest()
        assert baselined.model_local_hashes[name] == expected, name
    assert (
        baselined.model_version_hashes["orders_summary"]
        != current.model_version_hashes["orders_summary"]
    )


@pytest.mark.parametrize(
    "test_case",
    [
        RecordedFunctionHashTestCase(
            description="unchanged function keeps the local hash its callers recorded",
            body_sql="order_status = 'completed'",
            recorded_body_sql="order_status  =  'completed'  -- final",
            expected_recorded_local_hash=True,
        ),
        RecordedFunctionHashTestCase(
            description="edited function gets its current local hash",
            body_sql="order_status = 'completed'",
            recorded_body_sql="order_status = 'shipped'",
            expected_recorded_local_hash=False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_old_scheme_function_record_when_hashing_then_unchanged_function_keeps_local_hash(
    test_case: RecordedFunctionHashTestCase,
) -> None:
    recorded_function: CompiledFunction = replace(
        build_compiled_function(body_sql=test_case.recorded_body_sql), name="add_one"
    )
    recorded: WarehouseFingerprints = WarehouseFingerprints(
        models={
            "orders": recorded_fingerprint(
                name="orders",
                node_type="model",
                definition="SELECT add_one(1) AS order_id",
                definition_hash="old_scheme_orders",
                metadata_json=json.dumps(
                    {"local_function_hashes": {"add_one": "old_local_add_one"}}
                ),
            )
        },
        functions={
            "add_one": recorded_fingerprint(
                name="add_one",
                node_type="udf",
                definition=build_compiled_function_fingerprint_sql(recorded_function),
                definition_hash="old_scheme_add_one",
            )
        },
    )
    current: DirectModelVersionIdentities = build_function_identities(
        body_sql=test_case.body_sql, recorded=None
    )

    baselined: DirectModelVersionIdentities = build_function_identities(
        body_sql=test_case.body_sql, recorded=recorded
    )

    expected: str = {
        True: "old_local_add_one",
        False: current.function_local_hashes["add_one"],
    }[test_case.expected_recorded_local_hash]
    assert baselined.function_local_hashes["add_one"] == expected
