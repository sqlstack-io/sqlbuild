from __future__ import annotations

import pytest

from sqlbuild._native import QueryFingerprintError, compute_query_hash, normalize_query_sql
from tests.unit.src.sqlbuild.compiler.fingerprints.main._test_types import (
    ComputeQueryHashStabilityTestCase,
    ComputeQueryHashTestCase,
    NormalizeQuerySqlTestCase,
    QueryFingerprintFailureTestCase,
)


@pytest.mark.parametrize(
    "test_case",
    [
        NormalizeQuerySqlTestCase(
            description="strips leading and trailing whitespace",
            query_sql="  SELECT id FROM orders  ",
            expected_normalized="SELECT id FROM orders",
        ),
        NormalizeQuerySqlTestCase(
            description="collapses internal whitespace runs to single spaces",
            query_sql="SELECT  id,  name\n  FROM\n    orders",
            expected_normalized="SELECT id, name FROM orders",
        ),
        NormalizeQuerySqlTestCase(
            description="preserves casing exactly",
            query_sql="SELECT Id FROM Orders WHERE Status = 'Active'",
            expected_normalized="SELECT Id FROM Orders WHERE Status = 'Active'",
        ),
        NormalizeQuerySqlTestCase(
            description="handles tabs and carriage returns",
            query_sql="\tSELECT\t\tid\r\nFROM orders\t",
            expected_normalized="SELECT id FROM orders",
        ),
        NormalizeQuerySqlTestCase(
            description="returns empty string for whitespace-only input",
            query_sql="   \n\t  ",
            expected_normalized="",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_raw_query_when_normalizing_then_returns_expected_text(
    test_case: NormalizeQuerySqlTestCase,
) -> None:
    result: str = normalize_query_sql(test_case.query_sql)

    assert result == test_case.expected_normalized


@pytest.mark.parametrize(
    "test_case",
    [
        ComputeQueryHashTestCase(
            "generic token stream",
            None,
            "SELECT id, name FROM orders",
            "264bcb1f81ce5ba25394a052016edc265aefba85c0698f616d3037f9d872d2bd",
        ),
        ComputeQueryHashTestCase(
            "DuckDB dollar quote and cast",
            "duckdb",
            "SELECT o.id, $$Customer's note$$ AS note, amount::DECIMAL(10, 2) AS amount "
            "FROM orders AS o",
            "07786643d9593498ac2b2394e6801cb578b23ba0f9a0e70dbcdaf0d3a3cbf9db",
        ),
        ComputeQueryHashTestCase(
            "PostgreSQL tagged dollar quote",
            "postgres",
            "SELECT o.id, $tag$it's$tag$ AS note, amount::NUMERIC AS amount FROM orders AS o",
            "8db865c6f239b990c917754df176bd3c9b2c0c322b9af1437298ec98b67241f9",
        ),
        ComputeQueryHashTestCase(
            "Snowflake path access and QUALIFY",
            "snowflake",
            "SELECT o.id, payload:customer.name::STRING AS customer FROM orders AS o "
            "QUALIFY ROW_NUMBER() OVER (PARTITION BY o.id ORDER BY o.id) = 1",
            "c75f7b5931a18e1a074ffa7ab113c256fc838285af75a3f9feb62d61cc76d94d",
        ),
        ComputeQueryHashTestCase(
            "DuckDB lower-case keywords and SQLBuild placeholder",
            "duckdb",
            'select o.id from __ref("orders") as o where o.id > 0',
            "fda88b9b0f26486bdd25b8e3bab5a982fbb387901ab229091b17772d044aebd0",
        ),
        ComputeQueryHashTestCase(
            "BigQuery raw string and backtick path",
            "bigquery",
            "SELECT o.id, r'raw\\d' AS pattern FROM `project.dataset.orders` AS o",
            "ce893f2fd9ab11ba453fc3c02e77b6690df6c3d1d8bc141fc19cf3891d9a61b4",
        ),
        ComputeQueryHashTestCase(
            "Databricks optimizer hint",
            "databricks",
            "SELECT /*+ BROADCAST(o) */ o.id FROM `catalog`.`orders` AS o",
            "7cd96f42bcb764136a7f423181302ba13cead02bb6cfb7382502a3a2aa4107c3",
        ),
        ComputeQueryHashTestCase(
            "SQL Server bracket identifiers",
            "sqlserver",
            "SELECT TOP 10 [o].[id], N'order' AS kind FROM [dbo].[orders] AS o",
            "f4fac3edb7682bbeddf4f73cc199493d45f1419bc8e2cd1d972fead2d67e6b71",
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_dialect_query_when_computing_hash_then_golden_fingerprint_is_stable(
    test_case: ComputeQueryHashTestCase,
) -> None:
    result: str = compute_query_hash(query_sql=test_case.query_sql, dialect=test_case.dialect)

    assert result == test_case.expected_hash


@pytest.mark.parametrize(
    "test_case",
    [
        ComputeQueryHashStabilityTestCase(
            "whitespace and line breaks are ignored",
            "duckdb",
            "SELECT  id  FROM  orders",
            "select\n  id\nfrom orders",
            True,
        ),
        ComputeQueryHashStabilityTestCase(
            "comments are ignored",
            "snowflake",
            "SELECT id -- order key\nFROM orders /* all */",
            "SELECT id FROM orders",
            True,
        ),
        ComputeQueryHashStabilityTestCase(
            "keyword and built-in function case is ignored",
            "duckdb",
            "SELECT COUNT(id) FROM orders WHERE id > 0 LIMIT 1",
            "select count(id) from orders where id > 0 limit 1",
            True,
        ),
        ComputeQueryHashStabilityTestCase(
            "unquoted identifier case is significant",
            "duckdb",
            "SELECT Id FROM Orders",
            "SELECT id FROM orders",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "alias case is significant",
            "duckdb",
            "SELECT 1 AS Foo",
            "SELECT 1 AS foo",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "Snowflake path key case is significant",
            "snowflake",
            "SELECT payload:customerId FROM orders",
            "SELECT payload:customerid FROM orders",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "BigQuery qualified table case is significant",
            "bigquery",
            "SELECT * FROM ds.Orders",
            "SELECT * FROM ds.orders",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "unquoted call name case is ignored",
            "duckdb",
            "SELECT MyFunc(id) FROM orders",
            "SELECT myfunc(id) FROM orders",
            True,
        ),
        ComputeQueryHashStabilityTestCase(
            "CTE column-list name case is significant",
            "duckdb",
            "WITH Tree(id) AS (SELECT 1) SELECT id FROM orders",
            "WITH tree(id) AS (SELECT 1) SELECT id FROM orders",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "SQLBuild placeholder case is significant",
            "duckdb",
            'SELECT id FROM __ref("orders")',
            'SELECT id FROM __REF("orders")',
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "PostgreSQL comments, layout and keyword case are ignored",
            "postgres",
            "SELECT o.id -- key\nFROM orders AS o WHERE o.id > 0",
            "select o.id from orders as o /* all */ where o.id > 0",
            True,
        ),
        ComputeQueryHashStabilityTestCase(
            "PostgreSQL dollar-quoted body is significant",
            "postgres",
            "SELECT $tag$it's$tag$ AS note",
            "SELECT $tag$it s$tag$ AS note",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "SQLite comments and keyword case are ignored",
            "sqlite",
            "SELECT id FROM orders -- trailing",
            "select id\nfrom orders",
            True,
        ),
        ComputeQueryHashStabilityTestCase(
            "SQLite identifier case is significant",
            "sqlite",
            "SELECT Id FROM orders",
            "SELECT id FROM orders",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "Snowflake double-slash comments are ignored",
            "snowflake",
            "SELECT id // order key\nFROM orders",
            "SELECT id FROM orders",
            True,
        ),
        ComputeQueryHashStabilityTestCase(
            "generic keyword case is ignored and identifier case is not",
            None,
            "select Id from orders",
            "SELECT id FROM orders",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "string literal whitespace is significant",
            "duckdb",
            "SELECT 'a  b' AS code",
            "SELECT 'a b' AS code",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "quoted identifier case is significant",
            "snowflake",
            'SELECT "Id" FROM orders',
            'SELECT "ID" FROM orders',
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "number spelling is significant",
            "duckdb",
            "SELECT 1.0 AS amount",
            "SELECT 1.00 AS amount",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "optimizer hints are significant",
            "databricks",
            "SELECT /*+ BROADCAST(o) */ o.id FROM orders o",
            "SELECT /*+ REPARTITION(3) */ o.id FROM orders o",
            False,
        ),
        ComputeQueryHashStabilityTestCase(
            "different queries produce different hashes",
            None,
            "SELECT id FROM orders",
            "SELECT id FROM customers",
            False,
        ),
    ],
    ids=lambda case: case.description,
)
def test_given_two_queries_when_computing_hashes_then_stability_matches_expected(
    test_case: ComputeQueryHashStabilityTestCase,
) -> None:
    hash_a: str = compute_query_hash(query_sql=test_case.query_a, dialect=test_case.dialect)
    hash_b: str = compute_query_hash(query_sql=test_case.query_b, dialect=test_case.dialect)

    assert (hash_a == hash_b) == test_case.expected_same_hash


@pytest.mark.parametrize(
    "test_case",
    [
        QueryFingerprintFailureTestCase(
            "unterminated string literal",
            "duckdb",
            "SELECT 'open FROM orders",
            "Unterminated string",
        )
    ],
    ids=lambda case: case.description,
)
def test_given_untokenizable_sql_when_computing_hash_then_raises_fingerprint_error(
    test_case: QueryFingerprintFailureTestCase,
) -> None:
    with pytest.raises(QueryFingerprintError, match=test_case.expected_message):
        _ = compute_query_hash(query_sql=test_case.query_sql, dialect=test_case.dialect)
