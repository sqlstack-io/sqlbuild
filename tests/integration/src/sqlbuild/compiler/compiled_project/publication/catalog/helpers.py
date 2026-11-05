"""Observe the installed location's catalog projection and statistics samples."""

import json
import sqlite3
import time
from contextlib import closing
from pathlib import Path
from subprocess import Popen
from typing import Any


def wait_for_catalog(
    control: Path, expected: list[dict[str, Any]], process: Popen[bytes], log: Path
) -> None:
    """Wait for the exact synthetic relation, not a count that unrelated rows can satisfy."""
    deadline: float = time.monotonic() + 45
    observed: list[dict[str, Any]] = [{}]
    while observed != expected:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, (observed, expected, log.read_text())
        with closing(sqlite3.connect(control)) as connection:
            rows: list[tuple[str]] = connection.execute(
                "SELECT record_json FROM sqb_warehouse_catalog "
                "WHERE json_extract(record_json, '$.name')='orders_🧾'"
            ).fetchall()
        observed = [json.loads(row[0]) for row in rows]
        time.sleep(0.05)


def wait_for_statistics(
    control: Path, name: str, process: Popen[bytes], log: Path
) -> list[dict[str, Any]]:
    """Observe actual metadata-only sampling through the installed location and HTTP ingestion."""
    deadline: float = time.monotonic() + 45
    records: list[dict[str, Any]] = []
    while not records:
        assert process.poll() is None, log.read_text()
        assert time.monotonic() < deadline, log.read_text()
        with closing(sqlite3.connect(control)) as connection:
            rows: list[tuple[str]] = connection.execute(
                "SELECT record_json FROM sqb_warehouse_statistics "
                "WHERE relation_name=? ORDER BY observed_at,event_id",
                (name,),
            ).fetchall()
        records = [json.loads(row[0]) for row in rows]
        time.sleep(0.05)
    return records
