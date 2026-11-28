from __future__ import annotations

from typing import Any

from sqlbuild.adapters.sqlite.classes.sqlite_adapter import SqliteAdapter
from tests.integration.src.sqlbuild.adapters.sqlite.helpers import fetch_rows, run_sql


def _load(*, adapter: SqliteAdapter, connection: Any, rows: str) -> None:
    run_sql(
        adapter=adapter,
        connection=connection,
        statements=(
            "DROP TABLE IF EXISTS customers",
            "CREATE TABLE customers AS SELECT column1 AS customer_id, column2 AS plan, "
            f"column3 AS updated_at FROM (VALUES {rows})",
        ),
    )


def _apply_changes(*, adapter: SqliteAdapter, connection: Any) -> None:
    run_sql(
        adapter=adapter,
        connection=connection,
        statements=adapter.render_apply_timestamp_snapshot_changes(
            destination="customer_plans",
            origin="customers",
            unique_key=("customer_id",),
            updated_at_column="updated_at",
            observed_at_column=None,
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            initial_valid_from=None,
            output_columns=("customer_id", "plan", "updated_at"),
            invalidate_hard_deletes=True,
        ),
    )


def test_given_iso_text_updated_at_when_a_deleted_key_returns_the_same_day_then_it_opens_after_its_close(
    adapter: SqliteAdapter, connection: Any
) -> None:
    today: str = fetch_rows(
        adapter=adapter, connection=connection, sql="SELECT strftime('%Y-%m-%d', 'now')"
    )[0][0]
    _load(
        adapter=adapter,
        connection=connection,
        rows="(1, 'basic', '2026-01-01T10:00:00'), (2, 'basic', '2026-01-01T10:00:00')",
    )
    run_sql(
        adapter=adapter,
        connection=connection,
        statements=adapter.render_create_initial_snapshot_destination(
            table_type="table",
            destination="customer_plans",
            origin="customers",
            snapshot_strategy="timestamp",
            updated_at_column="updated_at",
            observed_at_column=None,
            valid_from_column="valid_from",
            valid_to_column="valid_to",
            initial_valid_from=None,
        ),
    )
    _load(adapter=adapter, connection=connection, rows="(1, 'basic', '2026-01-01T10:00:00')")
    _apply_changes(adapter=adapter, connection=connection)
    _load(
        adapter=adapter,
        connection=connection,
        rows=f"(1, 'basic', '2026-01-01T10:00:00'), (2, 'pro', '{today}T00:00:01')",
    )
    _apply_changes(adapter=adapter, connection=connection)

    versions: tuple[tuple[object, ...], ...] = fetch_rows(
        adapter=adapter,
        connection=connection,
        sql=(
            "SELECT plan, julianday(valid_from) >= julianday(LAG(valid_to) OVER "
            "(ORDER BY valid_from)), valid_from = updated_at, valid_to IS NULL "
            "FROM customer_plans WHERE customer_id = 2 ORDER BY valid_from"
        ),
    )
    assert versions == (("basic", None, 1, 0), ("pro", 1, 0, 1))
