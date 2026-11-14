# Investigating past runs with Recall

Use Recall to answer "what did that run do?" without rerunning anything: which commands ran and
how they ended, which statements they executed and with what SQL, which audits failed, and exactly
what they printed. `sqb recall query` runs one read-only `SELECT` over a set of views;
`sqb recall output` prints a run's output byte for byte.

## Which store you are reading

- **Local (the default).** `logs/recall.sqlite3` in the project: this checkout's own runs. Queries
  use SQLite SQL.
- **Shared.** `read_connection` in `[recall]` or `[targets.<name>.recall]` names a ClickHouse (or
  SQLite) connection holding every machine's runs: teammates, CI, scheduled production builds.
  Queries on a ClickHouse store use ClickHouse SQL. `write_connection` is where this machine's own
  commands record; it can differ from `read_connection`, so your own runs may be in the local store
  while you read the shared one.
- `--target <name>` picks the target whose Recall settings apply. The first lines of
  `sqb recall schema` name the store and dialect you are reading.

## How the store works

- **The views are the interface.** Query `invocations`, `statements`, `audit_results`, `events`,
  `diagnostics`, `artifacts`, `output_invocations`, `output_transcripts`, `output_tail` and
  `conflicts`. Never query the `recall_*` tables: they hold raw facts, and folding them correctly
  (duplicates, terminal facts, retention) is what the views do.
- **One invocation, many facts.** Every view has `invocation_id` and `invocation_started_at`; join
  on `invocation_id` (and on `statement_id`, `run_id`, `resource_id` or `sql_digest` where both
  sides have it).
- **`invocation_started_at` bounds scans.** Retention is per invocation: all rows of an invocation
  expire together. On a shared ClickHouse store, filter on `invocation_id` or
  `invocation_started_at` and the filter reaches the table reads; without one, a view folds the
  whole retention window.
- **Writing history never fails the command,** so a missing row means "not recorded", not "did not
  happen": `invocations.status = 'abandoned'` (the process died; `ended_at` is its last heartbeat
  and `ended_at_estimated = 1`), `statements.outcome = 'no_terminal'` (no terminal fact arrived),
  and `output_invocations.complete = 0` with a `gap_reason` when output is incomplete. A command
  stopped by SIGTERM or Ctrl-C is `cancelled`, with `signal_number` 15 or 2 and its real end time.
- **SQL is stored once by digest.** `statements.sql_digest` joins `artifacts` on
  `invocation_id` and `artifacts.digest = 'sha256:' || statements.sql_digest` for the SQL text
  (`artifacts.text`), also for failed and cancelled statements. `statements.sql_available = 0` means the text is missing, and
  `sql_unavailable_reason` says why.
- **Audit evidence is an artifact.** A warning or failing audit's flagged rows (up to
  `evidence_limit`) are in `artifacts` with `digest = 'sha256:' || audit_results.sample_digest`
  in the same invocation; `sample_digest` is
  `NULL` when `[recall] audit_evidence` kept none. Evidence holds warehouse data values.
- **Output is a byte-for-byte copy** of stdout and stderr, up to `max_output_bytes` per stream (the
  local store keeps the first and last MiB). Use `sqb recall output` rather than querying
  `output_transcripts` for long output.
- **The same views exist in both dialects.** Column names and meanings match; types and SQL
  functions differ (SQLite stores timestamps as ISO-8601 UTC text, ClickHouse as
  `DateTime64(6, 'UTC')`).
- **Not recorded:** `sqb recall` itself, commands that create a project (`sqb init`), raw-query
  `sqb diff`, commands answered without starting Python (a warm-cache `sqb compile`, `sqb clean`),
  and anything when `[recall] enabled = false`.

## Values worth knowing

| Column | Values |
|--------|--------|
| `invocations.status` | `completed`, `failed`, `cancelled`, `running`, `abandoned`, `unknown` |
| `invocations.selection`, `selection_count`, `selector_files` | The selectors the command ran with, including those read from `--select-file` (paths in `selector_files`); `selection_truncated = 1` when the list exceeded 64 KiB |
| `invocations.cost_status` | `complete`, `partial`, `pending`, `unavailable`, `collection_failed`; `NULL` when the adapter attributes no cost |
| `invocations.origin` | `integration` (an orchestrator such as Dagster started it), `ci` (CI environment variables were set), `local`; `NULL` on runs older SQLBuild versions recorded |
| `invocations.command_mode` | What a diagnostic checked: `sqb debug` records `config_only` (`--no-connection`) or `config_and_connection`; `NULL` for other commands |
| `invocations.integration`, `integration_job`, `integration_run_id`, `integration_retry` | The orchestrator, job, run and retry from `external_context`, for example `dagster` and its job name; `NULL` when there is none or an older version stored it as non-JSON |
| `invocations.error_type`, `error_code`, `error_message` | The cause of a failed invocation, also when it failed only through its exit code (`sqb debug`: `C980` connection, `C981` query, `C982` other check) |
| `statements.outcome` | `completed`, `failed`, `handled` (failed, and SQLBuild recovered, such as a timed-out cost history read retried in smaller windows), `no_terminal` |
| `statements.purpose` | `model_build` for a model's build statements, `cost_telemetry` for cost history reads, and others; check with `SELECT DISTINCT purpose` |
| `audit_results.outcome` | `pass`, `warn`, `error`, `insufficient`; `execution_error` is set when the audit query itself failed |
| `output_invocations.gap_reason` | `NULL` (complete), `truncated`, `filtered`, `dropped`, `unexplained` |
| `events.event_type` | `invocation_started`, `invocation_completed`, `invocation_failed`, `run_started`, `run_completed`, `run_failed`, `resource_attempt_started`, `resource_attempt_completed`, `resource_attempt_failed`, `resource_attempt_skipped`, `operation_*`, `statement_started`, `statement_submitted`, `statement_completed`, `statement_failed`, `statement_heartbeat`, `microbatch_*`, `retry_scheduled`, `audit_completed` |

## Traps

- `invocations` has `status`; `statements` and `audit_results` have `outcome`. There is no
  `statements.status`.
- On a shared ClickHouse store write ClickHouse SQL (`now() - INTERVAL 1 DAY`, `countIf`), not
  SQLite SQL (`strftime`, `datetime('now', '-1 day')`).
- Check values before filtering on a kind or status column: `SELECT DISTINCT resource_kind, purpose
  FROM statements`. A model's `resource_kind` is its materialization (`table`, `view` or a custom
  one), never `model`; its build statements have `purpose = 'model_build'`.
- Keep queries few and targeted: select the columns you need, filter on `invocation_id` or
  `invocation_started_at`, and add `LIMIT`. `SELECT *` over a view on a large store is slow.
- Filter on a literal `invocation_id`. On ClickHouse, `invocation_id = (SELECT ...)` does not reach
  the table reads, so look the ID up first and paste it into the next query.
- `--json` returns `{"columns": [...], "rows": [[...], ...], "row_count", "truncated",
  "statistics"}`: rows are arrays in column order, and `statistics` carries `rows_read`,
  `bytes_read` and `elapsed_ms` on ClickHouse. A failing `--json` query prints
  `{"error": {"code", "message", "help"}}` on stdout and exits `1`.
- The default `--limit` is 100 rows; `truncated: true` means there were more.
- Every command is recorded, including someone's local `sqb debug --target prod`. When looking for
  scheduled runs, filter by purpose: `command IN ('build', 'run', 'test', 'audit', 'seed')`, or
  `origin = 'integration'` for orchestrated runs, so local diagnostics do not look like failed
  production runs.
- Output shows invocation IDs by their first 8 characters. `sqb recall output` accepts any unique
  prefix and lists the matches when one is ambiguous; in SQL use `invocation_id LIKE '<prefix>%'`
  (SQLite) or `startsWith(invocation_id, '<prefix>')` (ClickHouse).

## Output format

- Keep the default format for anything a person reads. It prints a table with column types when
  the result fits and one record per row otherwise, every value in full.
- If a result is too long, reshape the question, not the output: aggregate with `GROUP BY`, select
  only the columns you need, or add a `LIMIT`. A long listing usually means the query is not
  precise enough yet.
- Never switch format to shorten output or save context. `--format long` always prints records;
  `--json` is for programs, for example to chain an `invocation_id` into the next query, or when
  you need `statistics` (`rows_read`, `bytes_read`, elapsed time). If you change the format, say
  why.

## Worked investigations

Each query below runs as written against a local SQLite store; the ClickHouse spelling is given
where it differs.

Why did the last run fail?

```bash
sqb recall query "SELECT invocation_id, command, status, exit_code, error_type, error_code, error_message
  FROM invocations WHERE status = 'failed' ORDER BY invocation_started_at DESC LIMIT 1"
sqb recall output <invocation_id> --stream stderr --tail 40
```

For the last scheduled run, filter on where it came from or the target, for example
`WHERE origin = 'integration' AND target = 'prod'`.

The latest runs of an orchestrated job (the same SQL on SQLite and ClickHouse):

```bash
sqb recall query "SELECT invocation_id, invocation_started_at, status, integration_run_id, integration_retry
  FROM invocations WHERE integration_job = 'orders_job'
  ORDER BY invocation_started_at DESC LIMIT 5"
```

What did a model actually execute, with its SQL? Find the run, then its statements:

```bash
sqb recall query "SELECT invocation_id, invocation_started_at, status FROM invocations
  WHERE command = 'build' ORDER BY invocation_started_at DESC LIMIT 1"
sqb recall query "SELECT s.statement_kind, s.outcome, s.duration_ms, a.text
  FROM statements s JOIN artifacts a
    ON a.invocation_id = s.invocation_id AND a.digest = 'sha256:' || s.sql_digest
  WHERE s.invocation_id = '<invocation_id>' AND s.resource_name = 'orders'
  ORDER BY s.started_at"
```

The slowest statements in a run:

```bash
sqb recall query "SELECT resource_name, statement_kind, duration_ms, row_count, outcome
  FROM statements WHERE invocation_id = '<invocation_id>'
  ORDER BY duration_ms DESC LIMIT 10"
```

The first run where an audit started failing:

```bash
sqb recall query "SELECT invocation_id, invocation_started_at, outcome, violation_count
  FROM audit_results WHERE audit_name = 'revenue_is_non_negative'
  ORDER BY invocation_started_at DESC LIMIT 20"
```

Read down until `outcome` changes back to `pass`; the row above it is the first failing run.

A production run against mine:

```bash
sqb recall query "SELECT invocation_id, target, git_revision, sqlbuild_version, status, duration_ms
  FROM invocations WHERE command = 'build' AND target IN ('prod', 'dev')
  ORDER BY invocation_started_at DESC LIMIT 4"
```

`git_revision` is empty in deployments without `.git` unless they set `SQLBUILD_GIT_REVISION`.

Did a run record everything?

```bash
sqb recall query "SELECT complete, gap_reason, missing_sequences, reported_drops, truncated_bytes
  FROM output_invocations WHERE invocation_id = '<invocation_id>'"
```

On ClickHouse, bound time-based questions the same way, for example
`WHERE invocation_started_at >= now() - INTERVAL 1 DAY`; on SQLite,
`WHERE invocation_started_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day')`.

## Running queries in parallel

`sqb recall query` is read-only and starts quickly, so independent questions can be separate calls
run at the same time. When the answers relate to each other, prefer one query with CTEs, joins or
`UNION ALL`.

## Every view and column

`sqb recall schema` prints the same list for the store you are reading, with its dialect's types.

<!-- recall-schema:start (generated from `sqb recall schema`) -->

```text
Timestamps are ISO-8601 UTC text with microseconds; filter invocation_started_at to bound scans; all rows of an invocation expire together.

artifacts (view): captured SQL and other artifacts, by reference
  invocation_id TEXT  join key
  invocation_started_at TEXT
  digest TEXT
  ref_id TEXT
  role TEXT
  statement_id TEXT  join key
  resource_id TEXT  join key
  kind TEXT
  media_type TEXT
  size INTEGER
  available ANY
  unavailable_reason ANY
  text ANY
  content BLOB

audit_results (view): one row per evaluated audit; its evidence rows are in artifacts
  invocation_id TEXT  join key
  invocation_started_at TEXT
  event_id TEXT
  occurred_at TEXT
  run_id TEXT  join key
  resource_id TEXT  join key
  resource_attempt_id TEXT
  audit_name ANY
  audit_definition_name ANY
  target_relation ANY
  attached_target_kind ANY
  attached_target_name ANY
  column ANY
  outcome TEXT
  severity ANY
  violation_count ANY
  measured_value ANY
  evaluation_mode ANY
  execution_error ANY
  sql_digest TEXT  join key
  evidence_count ANY
  evidence_truncated ANY
  sample_digest ANY

conflicts (view): record keys reused with different content; the first row is kept
  dataset TEXT
  record_key TEXT
  existing_sha256 TEXT
  rejected_sha256 TEXT
  detected_at TEXT
  invocation_id TEXT  join key
  invocation_started_at TEXT
  schema_version INTEGER
  producer_version TEXT
  ingested_at TEXT
  content_hash TEXT

diagnostics (view): structured diagnostics at or above the configured severity
  record_id TEXT
  sequence INTEGER
  occurred_at TEXT
  severity TEXT
  logger TEXT
  source TEXT
  message TEXT
  fields TEXT
  log_stream_id TEXT
  run_id TEXT  join key
  resource_id TEXT  join key
  resource_attempt_id TEXT
  operation_id TEXT
  statement_id TEXT  join key
  invocation_id TEXT  join key
  invocation_started_at TEXT
  schema_version INTEGER
  producer_version TEXT
  ingested_at TEXT
  content_hash TEXT

events (view): every lifecycle fact
  event_id TEXT
  invocation_sequence INTEGER
  occurred_at TEXT
  event_type TEXT
  event_kind TEXT
  severity TEXT
  producer TEXT
  event_schema_version INTEGER
  run_id TEXT  join key
  resource_id TEXT  join key
  resource_attempt_id TEXT
  operation_id TEXT
  statement_id TEXT  join key
  command TEXT
  resource_kind TEXT
  resource_name TEXT
  error_code TEXT
  error_type TEXT
  duration_ms REAL
  exit_code INTEGER
  query_id TEXT
  sql_digest TEXT  join key
  intent TEXT
  statement_kind TEXT
  purpose TEXT
  outcome TEXT
  row_count INTEGER
  affected_rows INTEGER
  payload ANY
  started_at TEXT
  invocation_id TEXT  join key
  invocation_started_at TEXT
  schema_version INTEGER
  producer_version TEXT
  ingested_at TEXT
  content_hash TEXT

invocations (view): one row per invocation; status is completed, failed, cancelled, running, abandoned or unknown; origin is integration, ci or local; integration, integration_job, integration_run_id and integration_retry come from external_context
  invocation_id TEXT  join key
  invocation_started_at ANY
  started_at ANY
  status ANY
  ended_at ANY
  duration_ms ANY
  ended_at_estimated ANY
  exit_code INTEGER
  error_type TEXT
  error_code TEXT
  error_message ANY
  signal_number ANY
  cost_status ANY
  command ANY
  subcommand ANY
  origin TEXT
  command_mode TEXT
  target TEXT
  adapter TEXT
  project_name TEXT
  selection TEXT
  selection_count ANY
  selection_truncated INTEGER
  selector_files TEXT
  exclusion TEXT
  sqlbuild_version TEXT
  git_revision TEXT
  git_dirty INTEGER
  host_token TEXT
  pid INTEGER
  external_context TEXT
  integration ANY
  integration_job ANY
  integration_run_id ANY
  integration_retry ANY
  recall_filter_active INTEGER
  last_heartbeat_at ANY

output_invocations (view): output completeness per invocation: gaps, drops, filtering, truncation, flush
  invocation_id TEXT  join key
  invocation_started_at BLOB
  chunks ANY
  bytes ANY
  stdout_bytes ANY
  stderr_bytes ANY
  truncated_bytes ANY
  max_sequence ANY
  missing_sequences ANY
  reported_drops ANY
  filtered ANY
  failed ANY
  filter_active ANY
  flush_complete ANY
  gap_reason ANY
  complete ANY

output_tail (view): the last 200 output chunks of each invocation as text
  invocation_id TEXT  join key
  invocation_started_at TEXT
  sequence INTEGER
  stream TEXT
  occurred_at TEXT
  byte_count INTEGER
  text TEXT

output_transcripts (view): exact stdout, stderr and combined bytes per invocation
  invocation_id TEXT  join key
  invocation_started_at ANY
  stdout BLOB
  stderr BLOB
  combined BLOB

statements (view): one row per statement; outcome handled means SQLBuild recovered from the failure; no_terminal means no terminal fact arrived
  invocation_id TEXT  join key
  invocation_started_at TEXT
  statement_id TEXT  join key
  run_id TEXT  join key
  resource_id TEXT  join key
  resource_name ANY
  resource_kind ANY
  resource_attempt_id TEXT
  operation_id TEXT
  intent TEXT
  statement_kind TEXT
  purpose TEXT
  query_id ANY
  started_at ANY
  submitted_at ANY
  ended_at TEXT
  outcome ANY
  error_type TEXT
  error_code TEXT
  duration_ms REAL
  row_count INTEGER
  affected_rows INTEGER
  sql_digest TEXT  join key
  sql_available ANY
  sql_unavailable_reason ANY
```

<!-- recall-schema:end -->
