<!-- generated-by: sqlbuild skills -->

# sqb recall

> Query the recorded history of past invocations.

Online: https://sqlbuild.com/docs/cli/recall/

## Contents

- Usage
- sqb recall query
- sqb recall schema
- sqb recall output
- Errors

Read the [Recall](../concepts/observability/recall.md) store that `read_connection` names, or the
local store: past invocations, the statements they executed, and what they printed. `sqb recall`
never records itself, and never creates or changes the store.

## Usage

```bash
sqb recall query "SELECT command, status, duration_ms FROM invocations ORDER BY invocation_started_at DESC"
sqb recall schema
sqb recall output last
```

## sqb recall query

Run one read-only `SELECT` against the Recall views, in the store's dialect: SQLite for the local
store, ClickHouse for a [ClickHouse store](../concepts/observability/recall-clickhouse.md).
Statements that write, and multiple statements, are rejected.

| Flag | Description |
|------|-------------|
| `sql` | The query (positional argument) |
| `--format` | `long` prints one record per row; `json` is for programs. Without it, a table when the result fits, as [`sqb query`](query.md#output) prints it |
| `--json` | Shorthand for `--format json` |
| `--limit` | Maximum rows to return (default: 100) |
| `--no-limit` | Disable the row limit |
| `--max-cell-bytes` | Longest cell in text formats, in bytes (default: 4096; `0` for no limit) |
| `--target` | Target whose store to read (default: the default target) |
| `--backend` | `logs` (default) reads the Recall store; `warehouse` runs the query on the target's connection |

The latest invocation in a project named `orders_shop`:

```bash
sqb recall query \
  "SELECT command, status, exit_code, duration_ms FROM invocations
   ORDER BY invocation_started_at DESC LIMIT 1"
```

```
┌─────────┬───────────┬───────────┬─────────────┐
│ command │  status   │ exit_code │ duration_ms │
│  text   │   text    │  integer  │    real     │
├─────────┼───────────┼───────────┼─────────────┤
│ build   │ completed │         0 │       752.0 │
└─────────┴───────────┴───────────┴─────────────┘

1 row
```

The second header line is each column's type in the store. A result wider than the terminal (120
characters when output is redirected), or with a value spanning lines, prints one record per row
instead, so nothing is cut. `--format table` and `--format csv` are not accepted: the default
output already prints a table when it fits, and `--json` is for programs.

The slowest statements of past builds, with their SQL:

```bash
sqb recall query --max-cell-bytes 60 \
  "SELECT s.resource_name, round(s.duration_ms, 1) AS ms, a.text AS sql
   FROM statements s JOIN artifacts a
     ON a.invocation_id = s.invocation_id AND a.digest = 'sha256:' || s.sql_digest
   WHERE s.statement_kind = 'CREATE' ORDER BY s.duration_ms DESC LIMIT 2"
```

```
┌───────────────┬──────┬──────────────────────────────────────────────────────────────────────────┐
│ resource_name │  ms  │                                   sql                                    │
│     text      │ real │                                   text                                   │
├───────────────┼──────┼──────────────────────────────────────────────────────────────────────────┤
│ orders        │  5.1 │ CREATE OR REPLACE TABLE dev.orders__staging AS SELECT 1 AS o…[+81 bytes] │
│ NULL          │  3.2 │ CREATE SCHEMA IF NOT EXISTS dev                                          │
└───────────────┴──────┴──────────────────────────────────────────────────────────────────────────┘

2 rows
```

Failed invocations of the last week:

```bash
sqb recall query \
  "SELECT invocation_id, command, error_code FROM invocations
   WHERE status = 'failed' AND invocation_started_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-7 days')"
```

Timestamps are ISO-8601 UTC text, so they compare correctly as strings. Filter on
`invocation_started_at` to bound a scan.

## sqb recall schema

List every Recall view with its columns, types, and join keys. Accepts `--format`, `--json`, and
`--target`.

Recall creates and upgrades its store itself on the first write when the writer may run DDL, so
there is usually nothing to run. With `--ddl`, print instead the SQL an administrator runs when the
writer may not: what creates the store, or brings it up to this SQLBuild version (missing columns,
replaced views, and the schema version row). It reads the store through `read_connection` and
changes nothing; when the store is up to date it prints nothing and says so on stderr. With `--backend warehouse`, it lists SQLBuild's state tables in the target's warehouse
instead, or describes one with `--relation <name>`.

## sqb recall output

Print what an invocation wrote, byte for byte.

```bash
sqb recall output last                       # the latest invocation, both streams in order
sqb recall output 7607b5b6 --stream stderr   # an invocation ID or unique prefix
sqb recall output last --tail 20             # only the last 20 lines
```

| Flag | Description |
|------|-------------|
| `invocation` | Invocation ID, unique prefix, or `last` (default) |
| `--stream` | `combined` (default), `stdout`, or `stderr` |
| `--tail` | Print only the last N lines |
| `--target` | Target whose store to read |

Output shows an invocation ID by its first eight characters; pass that or any other unique prefix.
A prefix that matches several invocations fails with `C973` and lists them, so you can pass a longer
one.

Where output past [`max_output_bytes`](../concepts/observability/recall.md#output-limit) was left
out, a note line marks the gap in place. Output that Recall could not record completely for any
other reason is followed by a note on stderr naming the gap.

## Errors

With `--json` (or `--format json`), a failing `query` or `schema` still exits `1` and prints its
coded error on stderr, and prints the error as JSON on stdout, so a program parsing the output reads
the error instead of failing to parse it:

```json
{"error": {"code": "C974", "message": "invalid Recall query: no such column: status ...", "help": null}}
```

| Code | Meaning |
|------|---------|
| `C970` | Recall is disabled for this project or target |
| `C971` | The store cannot be read, or no history is recorded yet |
| `C972` | No invocation matches |
| `C973` | The invocation prefix matches more than one invocation |
| `C974` | The query is invalid or not read-only |
| `C975` | The `[recall]` settings are invalid |
