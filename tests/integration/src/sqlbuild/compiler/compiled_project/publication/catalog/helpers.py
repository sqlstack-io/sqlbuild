"""Observe the installed location's catalog projection and statistics samples."""

import json
import sqlite3
import time
from contextlib import closing
from datetime import UTC, datetime, timedelta
from pathlib import Path
from subprocess import Popen
from typing import Any
from uuid import uuid4


def record_finished_run(control: Path) -> None:
    """Record a run that just ended for every published target, which makes its location resync."""
    ended_at: str = datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%S.%fZ")
    run_id: str = str(uuid4())
    with closing(sqlite3.connect(control)) as database:
        _ = database.execute(
            "INSERT INTO sqb_runs (run_id,location_id,target,snapshot_id,kind,origin,root_run_id,"
            "requested_by,selection_json,request_json,write_set_digest,status,queued_at,ended_at,"
            "node_counts_json,last_event_storage_id) SELECT ?,location_id,target,snapshot_id,"
            "'adhoc','manual',?,'test-admin','[]','{}','','success',?,?,'{}',0 "
            "FROM sqb_current_snapshots",
            (run_id, run_id, ended_at, ended_at),
        )
        database.commit()


def request_warehouse_sync(control: Path) -> str:
    """Queue the location work a person's Sync now creates, for the one published target."""
    request_id: str = str(uuid4())
    now: datetime = datetime.now(UTC)
    with closing(sqlite3.connect(control)) as database:
        _ = database.execute(
            "INSERT INTO sqb_location_work (request_id,location_id,target,snapshot_id,generation,"
            "request_json,requested_by,requested_at,expires_at) SELECT ?,c.location_id,c.target,"
            "c.snapshot_id,s.generation,json_object('request_id',?,'location_id',c.location_id,"
            "'target',c.target,'reason','Sync now','work',json_object('kind','sync_warehouse')),"
            "'test-admin',?,? FROM sqb_current_snapshots c JOIN sqb_snapshots s "
            "ON s.snapshot_id=c.snapshot_id",
            (
                request_id,
                request_id,
                now.strftime("%Y-%m-%dT%H:%M:%S.%fZ"),
                (now + timedelta(minutes=5)).strftime("%Y-%m-%dT%H:%M:%S.%fZ"),
            ),
        )
        database.commit()
    return request_id


def wait_for_catalog(
    control: Path, expected: list[dict[str, Any]], process: Popen[bytes], log: Path
) -> None:
    """Wait for the exact synthetic relation, not a count that unrelated rows can satisfy."""
    record_finished_run(control)
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
