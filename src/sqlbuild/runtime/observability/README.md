# Runtime observability contracts

This package defines wire contracts and synchronous in-process publication. It does not store,
export, queue, route across processes, or render records.

## Execution identity

Runtime correlation uses one immutable `ExecutionIdentity` snapshot backed by a scoped context
variable. Invocation identity is required; run, resource, resource-attempt, operation, statement,
and diagnostic log-stream identities are optional. Resource and resource-attempt IDs remain
separate. Explicit non-empty string IDs are preserved exactly, including SQLBuild run IDs, while
new invocation, resource-attempt, operation, statement, and log-stream IDs use `uuid4().hex`.

## Lifecycle facts

Schema version 1 uses an immutable envelope with `event_id`, `event_type`, `schema_version`,
`producer`, `producer_version`, UTC `occurred_at`, required `invocation_id`, optional `run_id`,
`resource_id`, `resource_attempt_id`, `operation_id`, and `statement_id` correlations, and a
JSON-compatible `payload` object.

The v1 event catalog is:

- Invocation: `invocation_started`, `invocation_completed`, `invocation_failed`; schema version 2 adds
  `invocation_phase`, one per finished preparation step
- Run: `run_started`, `run_completed`, `run_failed`
- Resource attempt: `resource_attempt_started`, `resource_attempt_completed`,
  `resource_attempt_failed`, `resource_attempt_skipped`
- Operation: `operation_started`, `operation_completed`, `operation_failed`
- Retry: `retry_scheduled`
- Statement: `statement_started`, `statement_submitted`, `statement_completed`,
  `statement_failed`

Known events are validated against catalogued correlations and safe payload fields. Statement
facts never contain full SQL text or parameter values. Query text, parameters, credentials, and
other sensitive or high-cardinality details must not be added to lifecycle payloads.

Statement facts may carry `intent`, `sql_digest`, warehouse `job_id`, `query_id`, `row_count`,
`affected_rows`, `batch_size`, and a JSON object `metadata` of at most 4096 bytes in deterministic
encoded form. Counts are nonnegative integers and durations are nonnegative finite milliseconds.

Completed and failed facts are terminal for their correlated scope. Started and submitted facts
are not terminal. Facts are append-only. Re-publication of a byte-equivalent fact with the same
`event_id` is idempotent; reuse of an ID for different content is a contract violation.

Consumers must treat unknown event names and schema versions as opaque envelopes. Compatible
additions happen through catalogued payload fields or new event names. Known v1 envelopes reject
unknown top-level fields; incompatible envelope changes require a new schema version.

## Diagnostic logs

Diagnostic schema version 1 is structurally separate from lifecycle facts. Its envelope contains
`severity`, `logger`, `source`, `message`, JSON-compatible `fields`, producer and UTC timestamp
metadata, and optional `log_stream_id` plus lifecycle correlation IDs. Logs explain behavior but
are not lifecycle facts and cannot establish completion or failure semantics.

SQLBuild's structured `log_sql` and `log_debug_event` helpers attach the current execution identity.
The CLI installs logging routes only for the active invocation. Internal diagnostics and user
Python `INFO`-and-higher records are retained in that invocation's `diagnostics.jsonl`; internal
debug records reach stderr only with `--debug`. SQL records always include an action and SHA256
digest. Full SQL is omitted from diagnostics, stderr, metadata, and lifecycle facts; sensitive SQL
artifacts remain under `target/run/`.

SQLBuild does not write a shared `target/sqlbuild.log`; diagnostics are isolated in each
invocation's `diagnostics.jsonl`.

Project-creation commands (`sqb init`, `sqb playground`, and `sqb dbt init`) intentionally bypass
project-local compute and diagnostic routing. Their command handlers must see the requested
destination before SQLBuild creates `logs/`, `target/`, or any diagnostic file there.

## In-process publication

`EventDispatcher` keeps lifecycle facts and diagnostic logs on separate typed channels. Publication
is synchronous and invokes a registration-order snapshot before returning, preserving each
producer's call order without promising a global order across concurrent producers. Unknown event
names and newer schema versions are delivered intact only to lifecycle subscribers registered with
`accepts_opaque=True`.

Subscriber failures are isolated and reported through an optional bounded health callback. Health
callback failures are swallowed, and nested failure reporting is suppressed to prevent recursion.
Registration and unsubscription are thread-safe. Callbacks run without the registration lock, so a
callback may publish recursively without deadlock. Nested publication runs immediately on the
current thread using the then-current subscriber snapshot; registration changes made by a callback
affect only later publishes, not the snapshot already in progress.

Framework boundaries may install an explicit dispatcher with `dispatcher_scope`; the scoped current
dispatcher is ContextVar-backed and defaults to `None`, avoiding mutable process-global lifecycle
state. `create_lifecycle_event` copies lifecycle correlation fields from the current execution
identity but intentionally excludes diagnostic-only `log_stream_id` and does not dispatch.

## Terminal and integration-result projections

The CLI installs one invocation-scoped terminal event index before command work starts. It retains
known lifecycle facts once by `event_id` in dispatcher publication order and indexes resource
attempts by `resource_id` and `resource_attempt_id`. Opaque or future events are not interpreted.

Final execution JSON remains a useful aggregate command document. The Dagster side channel is a
separate versioned canonical integration-result envelope. Their shared lifecycle mapping is:

- Invocation, run, or command-operation terminals determine aggregate failure when available.
- Resource-attempt terminals authorize asset and check rows and supply canonical monotonic
  `duration_ms` where that field already exists in version 1.
- Stable resource and attempt IDs correlate retries. A terminal can be claimed only once, so
  duplicate delivery or repeated callbacks do not duplicate integration results.
- Result callbacks claim the latest exact terminal available for that resource and flush one
  envelope immediately. Physical JSONL order is callback order, while `event_sequence` records the
  stable zero-based position among unique, known events observed by that invocation's in-memory
  terminal index. It is not durable or cross-process ordering. Claiming a retry can stale only
  earlier attempts of the same resource; unrelated terminals are never retired.
  Closing a writer never emits payloads.
- A rich callback without matching canonical terminal evidence emits no JSONL terminal row. Final
  JSON omits that incomplete resource while preserving the existing envelope and summary fields.

The event side-channel is an output-routing signal, not `--json`. When active, human progress is
uncolored and routed to stderr while integration-result JSONL is flushed to its configured file.
It does not request aggregate JSON or change command semantics; aggregate JSON remains exclusive to
an explicit `--json` or `--json-output` option.

Executor results provide bounded typed enrichment for structural fields that Dagster needs,
including relations, materialization actions, check attachment and severity, and allowlisted future
cursor and microbatch evidence. Arbitrary error messages and help, skip reasons, warnings, Python or
check metadata, full SQL, raw process output, arbitrary user output, and unbounded metadata are
never copied into the integration stream. They remain available through final aggregate output and
compute logs after process exit.

Operator documentation is at
[sqlbuild.com/docs/concepts/observability](https://sqlbuild.com/docs/concepts/observability/).
