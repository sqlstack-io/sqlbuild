<!-- generated-by: sqlbuild skills -->

# Observability

> Find out what a run did, or send its events to your own systems.

Online: https://sqlbuild.com/docs/concepts/observability/

SQLBuild records what each command did, separately from what it prints. Use these records instead
of parsing terminal output.

| You want to | Use |
|-------------|-----|
| Find out what a past run did: what ran, its SQL, audit results, errors and output | [Recall](observability/recall.md) and [`sqb recall`](../cli/recall.md) |
| Send events or output to your own systems, such as a message queue or a monitoring tool | [Typed sinks](observability/sinks.md) |
| Store lifecycle events and query run state from your own Python application | [Execution history](observability/execution-history.md) |
| Read the result of the command you just ran | Its final `--json` output |

Recall is on by default and covers investigation for people and agents. Sinks and execution history
are for building on SQLBuild's events elsewhere. The `stdout.log`, `stderr.log` and
`diagnostics.jsonl` files under `logs/<date>/` hold the same output as Recall; prefer
`sqb recall output`, which also works with a shared store.

## What SQLBuild records

| Record | What it holds | How far to rely on it |
|--------|---------------|-----------------------|
| `LifecycleEvent` | Each invocation, run, resource attempt, operation, statement, retry and audit | Complete once stored; events are never changed |
| `DiagnosticLog` | SQLBuild's structured diagnostics | Explanation only |
| [Recall](observability/recall.md) | Invocations, statements, SQL, audit results and output, locally or in ClickHouse | Best effort, for troubleshooting |
| `stdout.log` / `stderr.log` | The exact output of each process on this machine | For troubleshooting; Recall also stores it |
| `CommandOutputRecord` | Command output in bounded chunks, without colour codes, for remote systems | May be incomplete and may hold sensitive data |
| Final JSON output | The command's result | The result only, not what happened on the way |

## Lifecycle envelope

Events use lifecycle schema version 2:

```json
{
  "event_id": "9fb6d899754d4bd79c703067a8d5046c",
  "event_type": "statement_completed",
  "schema_version": 2,
  "producer": "sqlbuild",
  "producer_version": "0.91.0",
  "occurred_at": "2026-09-07T18:15:30.125000Z",
  "invocation_id": "4cc757dd93fe466aad2a220f1e76625e",
  "invocation_sequence": 17,
  "run_id": "run-20260907-181500",
  "resource_id": "model:orders",
  "resource_attempt_id": "97933c4aa29441cda133641345515e20",
  "operation_id": "c978e47367494051bdde68ad03e56db4",
  "statement_id": "5113ebc5afe247eb9c41dc0b27ef1a9a",
  "external_context": {
    "integration": {
      "name": "dagster",
      "run_id": "dagster-run-1",
      "job_name": "daily_models",
      "step_key": "sqlbuild_assets",
      "retry_number": 0,
      "partition_key": "2026-09-07"
    }
  },
  "payload": {
    "adapter": "snowflake",
    "duration_ms": 318.4,
    "query_id": "01b6f1c2-0000-0000-0000-000000000000",
    "sql_digest": "881b77b00a75e07936d7f20ea308a55e9ef98f2f1fd42c144a3a998d32094c1b"
  }
}
```

`event_id` is the immutable deduplication identity. `invocation_sequence` is unique and increasing
in synchronous publication order within one invocation, including concurrent workers. A sink may
prioritize delivery, so consumers reconstruct invocation chronology by sorting this field rather
than relying on arrival timestamps.

Schema-version-1 events remain readable. Unknown event names and future schema versions are retained
as opaque envelopes instead of being assigned invented semantics.

## Correlation and privacy

Every fact has an invocation ID. Nested IDs identify a run, logical resource and attempt, non-SQL
operation, and SQL statement. Orchestrators may provide bounded JSON-compatible
`external_context`; the Dagster integration supplies run, job, step, retry, and partition values
when its execution context exposes them.

Lifecycle payloads exclude full SQL, parameter values, credentials, arbitrary user messages, and
raw process output. Command output and local logs can contain sensitive data and require an explicit
retention and access policy.

## Failure semantics

A start without a terminal fact is unknown or presumed lost. Consumers must not fabricate success
or failure. Sink delivery is bounded and best effort; destination failure does not change successful
warehouse work, and successful delivery does not prove the command succeeded.

See [Recall](observability/recall.md) for the local history `sqb recall` queries, and
[Typed Sinks](observability/sinks.md) to export lifecycle and command-output records.
