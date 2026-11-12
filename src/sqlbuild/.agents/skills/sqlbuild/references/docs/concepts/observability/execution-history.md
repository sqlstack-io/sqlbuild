<!-- generated-by: sqlbuild skills -->

# Execution history

> Store and query lifecycle events and run history from Python, with SQLite locally or PostgreSQL when deployed.

Online: https://sqlbuild.com/docs/concepts/observability/execution-history/

## Contents

- Public contracts
- Local SQLite
- Deployed PostgreSQL

Execution history is a Python API for building your own run history: store SQLBuild's lifecycle
events in SQLite or PostgreSQL and read runs back, for example to show run status in an internal
tool. Everything is exported from `sqlbuild.execution_history`.

To look into what past runs did, you don't need this API: use
[Recall](recall.md), which records every command automatically and is
queried with [`sqb recall`](../../cli/recall.md).

`LifecycleEventLogStorage` stores events as they arrive and never changes them. `RunStorage` serves
the current state of each run, rebuilt from those events. Neither stores logs or command output.

## Public contracts

`LifecycleEventLogStorage` provides `append_event`, `append_events`, `get_events`, `get_schema_version`,
`upgrade_schema`, `close`, `dispose`, and context management. `RunStorage` provides `get_run`,
`get_runs`, `project`, `rebuild_from_events`, the schema/lifecycle methods, and context management.
The concrete SQLite and PostgreSQL classes additionally provide transactional
`append_and_project`, `reconcile`, and `check_health` operations.

Events are stored in the `lifecycle_event_log` table in SQLite and `sqlbuild_lifecycle_event_log`
in PostgreSQL.

Each newly stored event receives a positive, increasing `storage_order`, opaque stable `cursor`,
and UTC `received_at`. Pages ascend in storage order. `after_cursor` is exclusive,
`next_cursor` identifies the last returned record and is `None` for an empty page, and `has_more`
states whether another page exists. Limits default to 100 and cannot exceed 1000. Cursors are
backend- and storage-instance-owned; persist them verbatim and never parse them. They identify a
global position and remain valid if a later call changes filters.

`EventFilter` supports invocation ID, run ID, event types, event family, producer, and inclusive
event-time bounds. Event types and family are mutually exclusive. `EventFamily` contains
invocation, run, resource-attempt, operation, and statement; select retry facts with
`EventFilter(event_types=("retry_scheduled",))`. `RunFilter` supports invocation, status, and
inclusive projection-creation bounds. A run cursor identifies the global
`(created_at, run_id)` order and is also filter-independent.

Projection applies durable facts in `storage_order`, never `occurred_at` order. `created_at` is the
first run fact's `received_at`; start/end times retain event time. A run with no terminal is
`RunStatus.UNKNOWN` and `is_complete=False`. The last durable terminal wins if conflicting terminal
facts exist. Incremental projection and a full rebuild produce the same result, and one projection
call publishes all changes or none.

## Local SQLite

The CLI does not open local history implicitly. Construct and subscribe this backend only when an
application has a concrete durable-history consumer.

```python
from pathlib import Path

from sqlbuild.execution_history import EventFilter, RunFilter
from sqlbuild.sqlite_history import SQLiteExecutionHistory

with SQLiteExecutionHistory(project_dir=Path(".")) as history:
    assert history.check_health()

    cursor = None
    while True:
        page = history.get_events(
            event_filter=EventFilter(run_id="RUN_ID"),
            after_cursor=cursor,
        )
        for stored in page.records:
            print(stored.storage_order, stored.event)
        if not page.has_more:
            break
        cursor = page.next_cursor

    for run in history.get_runs(run_filter=RunFilter()).records:
        print(run.run_id, run.status, run.is_complete)
```

With no arguments, `SQLiteExecutionHistory()` uses the current directory. Passing `project_dir`
uses `<project_dir>/.sqlbuild/history.sqlite3`; passing `path` selects an exact file, and `path` may
be `":memory:"`. Do not pass both. Startup creates/migrates schema version 1 and reconciles the run
projection from all event facts.

Schema v1 has `execution_history_metadata`, append-only `lifecycle_event_log`, and disposable
`run_projection` tables plus run, invocation, type, and created-time indexes. SQLite uses WAL mode,
foreign keys, a configurable 5000 ms default busy timeout, a process-local reentrant lock, and
transactions for append/projection publication. Multiple processes may use SQLite subject to normal
SQLite/WAL host-filesystem constraints and busy timeout; it is a host-local backend, not a deployed
coordination service. Startup refuses unknown future schema versions and never resets or downgrades
data.

## Deployed PostgreSQL

Install `sqlbuild[postgres]` only in deployed processes and construct the backend explicitly. The
CLI never selects PostgreSQL implicitly. Neither backend is selected by ordinary CLI commands.

```python
import os
from datetime import UTC, datetime

from sqlbuild.execution_history import EventFilter
from sqlbuild.observability import create_lifecycle_event, invocation_scope, run_scope
from sqlbuild.postgres_history import PostgresExecutionHistory

occurred_at = datetime(2026, 9, 2, 10, 15, tzinfo=UTC)
with invocation_scope("deployment-20260902-1015"):
    with run_scope("run-20260902-1015"):
        run_started = create_lifecycle_event(
            event_type="run_started",
            event_id="run-20260902-1015-started",
            occurred_at=occurred_at,
            payload={"run_kind": "build", "selected_count": 12},
        )

with PostgresExecutionHistory(os.environ["SQLBUILD_HISTORY_DSN"]) as history:
    first = history.append_and_project((run_started,))[0]
    retry = history.append_event(run_started)
    assert retry.storage_order == first.storage_order

    page = history.get_events(
        event_filter=EventFilter(run_id="run-20260902-1015"),
        after_cursor=None,
    )
    checkpoint = page.next_cursor

# Persist checkpoint securely. A later process resumes after that exact global position.
with PostgresExecutionHistory(os.environ["SQLBUILD_HISTORY_DSN"]) as history:
    later = history.get_events(
        event_filter=EventFilter(run_id="run-20260902-1015"),
        after_cursor=checkpoint,
    )
```

The DSN must be resolved from a secret before construction. SQLBuild neither retains it for
diagnostics nor includes it in exceptions or `repr`. PostgreSQL schema v1 uses
`sqlbuild_storage_migrations`, authoritative `sqlbuild_lifecycle_event_log`, and disposable
`sqlbuild_run_projection`. Startup takes a transaction-scoped advisory migration lock, performs
forward-only migrations, and reconciles projection. All instances should run the same SQLBuild
version during rollout.

Append and projection serialize under a history advisory lock. Only serialization failures and
deadlocks (`40001`, `40P01`) are retried internally, up to three retries by default. If connection
loss makes commit acknowledgement uncertain, discard the backend, reconnect, and retry the exact
same deterministic event ID and content. Equal content returns the existing stored fact; different
content raises `IntegrityConflictError`. This is storage idempotency, not exporter exactly-once
delivery.

Grant deployed application roles only the required schema/table privileges and reserve migration,
backup, retention, and destructive privileges for operators. Back up `sqlbuild_lifecycle_event_log` and
`sqlbuild_storage_migrations`, preserving event IDs, canonical JSON, identity sequence values, and
storage namespace. `sqlbuild_run_projection` can be rebuilt with `reconcile()`. PostgreSQL performs
no automatic retention, partitioning, or archival.

These are selected operational examples rather than an exhaustive API reference. The public
`sqlbuild.execution_history` facade also exports canonical content/ID helpers, page-limit
validation, projection helpers, schema and paging constants, storage protocols, models, filters,
statuses, and typed storage/cursor/schema errors.

See [Observability](../observability.md) for the lifecycle event envelope, correlation, and
failure behavior.
