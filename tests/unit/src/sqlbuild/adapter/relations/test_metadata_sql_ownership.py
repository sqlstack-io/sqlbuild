"""Architecture guard: warehouse catalog SQL lives only in adapter inspection modules.

Planner and executor code must reach metadata through adapter methods and the shared inspection
catalog, so query shapes, IN-list caps, and per-schema read budgets stay enforceable in one place.
"""

from __future__ import annotations

import pytest

from tests.unit.src.sqlbuild.adapter.relations._test_types import MetadataSqlOwnershipTestCase
from tests.unit.src.sqlbuild.adapter.relations.helpers import metadata_sql_modules

_ALLOWED_METADATA_SQL_MODULES: frozenset[str] = frozenset(
    {
        "adapter/contract/classes/base_adapter.py",
        "adapter/contract/classes/duckdb_backed_adapter.py",
        "adapters/bigquery/_helpers/relation_usage.py",
        "adapters/bigquery/classes/bigquery_adapter.py",
        "adapters/databricks/classes/databricks_adapter.py",
        "adapters/databricks/constants.py",
        "adapters/postgres/classes/postgres_adapter.py",
        "adapters/snowflake/_helpers/metadata_types.py",
        "adapters/snowflake/_helpers/relation_changes.py",
        "adapters/snowflake/_helpers/show_metadata.py",
        "adapters/snowflake/classes/snowflake_adapter.py",
        "adapters/sqlserver/classes/sqlserver_adapter.py",
        "adapters/sqlserver/constants.py",
        "cost/models.py",
    }
)


@pytest.mark.parametrize(
    "test_case",
    [
        MetadataSqlOwnershipTestCase(
            description="catalog SQL appears only in adapter inspection modules",
            allowed_modules=_ALLOWED_METADATA_SQL_MODULES,
            expected_unlisted_modules=(),
        )
    ],
    ids=lambda case: case.description,
)
def test_given_source_tree_when_scanning_then_metadata_sql_stays_in_adapter_modules(
    test_case: MetadataSqlOwnershipTestCase,
) -> None:
    found: frozenset[str] = metadata_sql_modules()

    assert tuple(sorted(found - test_case.allowed_modules)) == test_case.expected_unlisted_modules
    assert tuple(sorted(test_case.allowed_modules - found)) == test_case.expected_unlisted_modules


if __name__ == "__main__":
    pytest.main([__file__, "-vv"])
