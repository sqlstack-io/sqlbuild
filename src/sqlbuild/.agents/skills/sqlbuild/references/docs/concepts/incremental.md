<!-- generated-by: sqlbuild skills -->

# Incremental Models

> Cursor-based incremental strategies, microbatch execution, and backfill policies.

Online: https://sqlbuild.com/docs/concepts/incremental/

## Contents

- Strategies
- Full-refresh overrides
- Cursors
- Microbatch execution
- Replay on change

An incremental model processes only new or changed rows on each build instead of rebuilding the
whole table. Pick a strategy and a cursor column, and SQLBuild works out where to resume from the
table itself, with no separate checkpoint store:

```sql
MODEL (
  description "Order activity per hour",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_hour,
  cursor_type timestamp,
  cursor_grain hour,
  unique_key [activity_hour],
);
```

A retry works out its range again from the warehouse. [Concurrent microbatches](#concurrent-batches) are the exception:
they record each batch to coordinate.

## Strategies

### append

Inserts new rows without modifying existing data. Optionally uses a cursor (read from the target table's highest value) to avoid reprocessing the full source on every run.

```sql
MODEL (
  description "One row per customer",
  materialized incremental,
  incremental_strategy append,
  cursor created_at,
  cursor_type timestamp,
  cursor_grain second,
  append_cursor_inclusive true,
);

SELECT id, customer_id, created_at
FROM __source("raw_events")
```

When `append_cursor_inclusive` is `true` (the default), the lower bound uses `>=`, which may duplicate the boundary row but avoids missing late-arriving data with the same cursor value. Set to `false` for an exclusive (`>`) lower bound if your cursor values are guaranteed unique.

Append without a cursor is also valid. The model simply inserts all rows from the source query on every run.

Append keeps every row. A `unique_key` on an append model only matches rows in
[diff](diff.md#row-matching) and [review](../cli/review.md), so the same key can appear in the
table more than once.

### delete_insert

Deletes the rows the new delta replaces, then inserts the delta. Requires `cursor`, `unique_key` or
`replace_key`.

```sql
MODEL (
  description "One row per order",
  materialized incremental,
  incremental_strategy delete_insert,
  unique_key [order_id],
  cursor order_id,
  cursor_type integer,
);

SELECT order_id, customer_id, order_status, ordered_at, line_total_cents
FROM __ref("fact_orders")
```

Which rows it deletes depends on the model's settings:

| Settings | Rows deleted before the insert |
|----------|--------------------------------|
| `cursor` | Rows whose cursor falls in the replay window |
| `unique_key` | Rows whose key appears in the delta. The key must hold one row per value; for groups of rows per value, use `replace_key` |
| `replace_key` | Rows whose `replace_key` values appear in the delta |
| `cursor` and `unique_key` or `replace_key` | Rows in the replay window, and rows whose key appears in the delta |
| `incremental_mode microbatch` | Rows in the batch's window; a `unique_key` only matches rows in diff and review |

With both a cursor and a key, a row whose cursor value moved out of the window, such as an order
updated after its first load, replaces its earlier version instead of adding a second row.

`unique_key` promises one row per key. Use `replace_key` when a key value covers a group of rows
that each load replaces whole, such as the lines of one forecast version:

```sql
MODEL (
  description "Forecast lines, several per forecast version",
  materialized incremental,
  incremental_strategy delete_insert,
  replace_key [forecast_version],
);

SELECT forecast_version, product_id, forecast_month, quantity
FROM __ref("stg_forecast_lines")
```

A delta holding version `2026-06` deletes every `2026-06` row and inserts the new ones. Other
versions keep their rows. A model sets `unique_key` or `replace_key`, not both, and `replace_key`
is valid only with `delete_insert` and not with `incremental_mode microbatch`. BigQuery rejects
`replace_key` with `A012` when planning; every other adapter replaces groups. Rules that need a
proven key, such as `SQLCLEAN018`, use `unique_key` and never `replace_key`.

### merge

Upserts rows using a unique key. Matched rows are updated; unmatched rows are inserted.

```sql
MODEL (
  description "One row per order",
  materialized incremental,
  incremental_strategy merge,
  unique_key [customer_id],
  cursor last_ordered_at,
  cursor_type timestamp,
  cursor_grain second,
  cursor_inputs (
    fact_orders ordered_at,
  ),
);

SELECT
  customer_id,
  MAX(ordered_at) AS last_ordered_at,
  COUNT(*) AS total_orders,
  SUM(line_total_cents) AS total_revenue_cents
FROM __ref("fact_orders")
GROUP BY customer_id
```

`merge` always requires `unique_key`. The cursor controls which upstream rows are scanned; the unique key determines how they're matched against the target.

## Full-refresh overrides

Incremental models can override command-level full-refresh behavior with `full_refresh`:

| Model setting | Normal command | Command with `--full-refresh` |
|---------------|----------------|-------------------------------|
| omitted | incremental | full refresh |
| `full_refresh false` | incremental | incremental |
| `full_refresh true` | full refresh | full refresh |

Use `full_refresh false` for models that must retain their normal cursor or microbatch execution even when a broader job requests full refresh:

```sql
MODEL (
  description "One row per event",
  materialized incremental,
  incremental_strategy delete_insert,
  full_refresh false,
  cursor event_date,
  cursor_type timestamp,
  cursor_grain day,
);
```

The override is evaluated per model, so one selection can contain incrementally executed opt-outs and full-refreshed models. With `--full-refresh`, an opted-out model simply runs incrementally; it does not abort the rest of the build.

`full_refresh false` also protects the model from every other full rebuild. Only the model's own explicit configuration can fully rebuild it: its own first run, or its own query change with `replay_on_change full`. If anything else would plan a full rebuild, for example a changed function that the model calls while it sets `replay_on_change full`, the plan fails with `S203`, naming the model and the cause, instead of silently replaying its history:

```
error[S203]: model 'order_history' sets full_refresh false, but the plan would fully rebuild it (function normalize_status changed with replay_on_change full)
```

Set `replay_on_change` to `forward` or `bounded-<duration>` to keep the existing history, or remove `full_refresh false` to allow the rebuild. Upstream changes never cause a full rebuild of a downstream model; see [Replay decisions](planning/replay-decisions.md).

This does not skip initial loading. If the destination relation does not exist, an incremental or microbatch model still builds the history required by its cursor policy. Cloning an existing destination before the build can provide a current watermark and avoid a first-run historical replay.

## Cursors

Cursors define the incremental replay boundary. SQLBuild queries `MAX(cursor)` from the target table and `MIN/MAX` from upstream inputs to compute the replay window automatically. Observed maxima are inclusive warehouse values; SQLBuild advances them once to produce an effective exclusive end bound.

| Field | Description |
|-------|-------------|
| `cursor` | Output column used to track incremental position |
| `cursor_type` | `timestamp` or `integer` |
| `cursor_grain` | Time grain for timestamp cursors: `second`, `minute`, `hour`, `day`, `month`, `year` |
| `cursor_start` | Lower bound floor; the cursor will never replay before this value, even inside an [explicit cursor window](#explicit-cursor-windows) |
| `cursor_inputs` | Map of upstream ref/source names to their cursor columns |

### Explicit cursor windows

`--start-cursor-ts` and `--end-cursor-ts` (or the `-int` forms, or `--start-cursor` and
`--end-cursor`) give every selected cursor model the same window, and each model fits it to its own
`cursor_start`:

- A window that starts before a model's `cursor_start` and ends after it starts that model at its
  `cursor_start`. The plan warns on that model:
  `explicit start 2020-06-01 is before cursor_start 2021-01-01; the window starts at 2021-01-01`.
- A window that ends at or before a model's `cursor_start` skips that model. The plan lists it under
  **Window before cursor_start** with the reason `window ends before cursor_start 2021-01-01`; in
  `sqb plan --json` its row has `"action": "skip"`, `"reason": "window_before_cursor_start"` and
  that reason as `skip_reason`.
- When the window skips every selected cursor model, there is nothing to build and the plan fails
  with `P001`, naming the window and each model's `cursor_start`. Selected models without a cursor
  don't count.

A window that reaches a model's exclusive `cursor_end` still fails the plan with `P001`.

### cursor_inputs

When a model references multiple upstream inputs, `cursor_inputs` is required to tell SQLBuild which column on each input carries the cursor:

```sql
MODEL (
  description "One row per order",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_hour,
  cursor_type timestamp,
  cursor_grain hour,
  cursor_inputs (
    fact_orders ordered_at,
  ),
);
```

SQLBuild uses these to determine the replay window. With multiple listed inputs, the end is the conservative common watermark: the minimum of their maxima. This prevents a faster input from advancing the model beyond data available from a slower input.

On a first build, SQLBuild derives the interval from the declared cursor inputs and cursor policy. SQLBuild decides the window while planning, before any staging or delta relation is created. When an input is built earlier in the same run, the window is decided just before the model runs, still before any staging work.

#### Models with no inputs

A cursor-based model must read at least one input that can bound its window, either listed in `cursor_inputs` or read through `__ref()`, `__source()` or `__seed()`. A model that reads none of them could build once but could never work out a window afterwards, so compile rejects it with `P011`:

```
error[P011]: incremental model 'daily_revenue' reads no inputs, so builds after the first cannot work out their cursor window
  --> sql/models/finance/daily_revenue.sql:5:3
    |
  5 |   cursor revenue_date,
    |   ^^^^^^^^^^^^^^^^^^^
  = help: read the data through __ref(), __source() or __seed() so its cursor can bound the window, or use materialized table if the model has no upstream data
```

A `rolling_window` microbatch model takes its window from the invocation time, so it may read no inputs.

#### Inputs with no rows

Empty inputs are normal: a new development or CI environment, a source in the middle of a reload, or an upstream model that is legitimately empty. When a listed input has no rows, there is no new data to process:

- On an incremental run, the model succeeds without changing its destination, and the plan and build output show the reason, for example `no input rows (stg_payments.paid_at)`. The model's audits still run against the unchanged destination, and downstream models continue. The `--json` build output lists the inputs as `empty_cursor_inputs`, and `sqb plan --json` reports `"resolution_status": "no_input_rows"` with the same inputs in `empty_inputs`.
- When only some listed inputs are empty, the window still waits for them, so the model makes no progress even though other inputs have new rows. The plan and build output show this as a warning, for example `waiting on empty input stg_refunds.refunded_at; other inputs have new rows`. The build counts it in the summary's `WARN=` total and lists it in the asset's `warnings` in `--json` output, and the JSON output adds `"waiting_on_empty_inputs": true`. The exit code and model status do not change. A watermark microbatch model with `cursor_watermark_mode any` instead progresses from the inputs that have rows.
- On a first build, the destination is created empty so that models that reference it can build. The next build derives its window from the inputs as usual.
- A rebuild that would replace an existing destination, such as a query change with `replay_on_change full` or a microbatch `--full-refresh`, fails instead of leaving the table empty. Load the inputs first, or set the window explicitly with `--start-cursor-ts` and `--end-cursor-ts` (or the `-int` forms).

Use [source freshness](sources.md) checks and source audits to catch an input that should not be empty. If SQLBuild cannot read the cursor bounds of an input at all, the plan fails with `S302`, naming the model and its inputs.

### Cursor bounds in model SQL

Cursor-based incremental models can read their effective interval with zero-argument intrinsics:

```sql
MODEL (
  description "One row per order",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_hour,
  cursor_type timestamp,
  cursor_grain hour,
  cursor_inputs (
    fact_orders ordered_at,
  ),
);

SELECT customer_id, ordered_at AS activity_hour
FROM __ref("fact_orders")
WHERE ordered_at >= __cursor_start()
  AND ordered_at < __cursor_end()
```

`__cursor_start()` is the effective inclusive start and `__cursor_end()` is the effective exclusive end after cursor floors, lookback, replay policy, and command-line overrides have been applied. The intrinsics accept no arguments and are only valid in built-in cursor incremental model query SQL. They are rejected in functions, hooks, audits, SQL tests and scenarios, source expressions, non-incremental or cursorless models, and custom materializations.

A non-microbatch full build (the first run, `--full-refresh`, or `replay_on_change full`) has no cursor interval, so the intrinsics span every value: `__cursor_start()` is the model's `cursor_start` floor, or the cursor type's lowest value without one, and `__cursor_end()` is the type's highest value. They render as typed literals, each unbounded end followed by `/* (unbounded) */` in the compiled SQL, and `sqb plan` shows the range as `(unbounded)`. Timestamps span `0001-01-01 00:00:00` to `9999-12-31 23:59:59.999999`, except on ClickHouse, where DateTime64 spans `1900-01-01 00:00:00` to `2299-12-31 23:59:59.999999`; integers span the 64-bit range. Because `__cursor_end()` is exclusive, a row whose cursor is exactly the highest value is left out. An explicit interval (`--start-cursor-ts` and `--end-cursor-ts`, or the `-int` forms) still bounds a full build.

In microbatch mode, the intrinsics resolve to each batch's concrete bounds. A microbatch full refresh discovers its range from current inputs while ignoring the old destination watermark.

#### Listed inputs bound the window; unlisted inputs do not

Only the inputs you list in `cursor_inputs` bound the replay window. This is an explicit choice, and it has two consequences worth understanding:

- **Listed inputs** drive the window. Their new data advances the `MAX`, which is what tells SQLBuild how far to reprocess and which rows of the target to rewrite.
- **Unlisted inputs are read in full.** SQLBuild does not add a cursor filter to them, and they do not bound the window. This is correct for lookup or dimension tables that have no meaningful cursor column: you do not list them, and SQLBuild reads them whole rather than trying to filter on a column that may not exist.

The implication for `delete_insert` and `merge`: the target rows that get rewritten are the ones whose cursor falls inside the window derived from the listed inputs. If an unlisted input changes in a way that should affect target rows outside that window, those rows are not rewritten on a normal incremental run. List every input whose new data should drive reprocessing; leave unlisted only the inputs you intend to read in full.

To capture changes that fall outside the normal forward window, see [Lookback](#lookback) for late-arriving data and [Replay on change](#replay-on-change) for model changes.

### Lookback

Lookback extends the start of the replay window backwards to re-process recent data. The cursor is forward-moving, so use lookback to capture late-arriving or backfilled records that land just behind the current position:

```sql
MODEL (
  description "One row per event",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor event_date,
  cursor_type timestamp,
  cursor_grain day,
  lookback 3d,
);
```

With `lookback 3d`, the replay window starts 3 days before the normal cursor position, ensuring that any late-arriving data within that window is picked up.

## Microbatch execution

For large incremental ranges, microbatch mode splits the replay window into configurable batches.
Choose an explicit strategy:

| Strategy | Window source |
|----------|---------------|
| `watermark` | Declared input availability; supports timestamp and integer cursors |
| `rolling_window` | The run's time rather than input maxima: the `lookback` periods before the current one, and the current period |

A `rolling_window` window is counted in `cursor_grain` periods and ends at the end of the current period, so every run replaces the partial current period along with the `lookback` periods before it. A model that should hold only complete periods filters the current one out in its own SQL. When the destination is created or replaced, as on a first build or `--full-refresh`, a model with `cursor_start` builds from `cursor_start` through the current period.

Each batch has its own audit cycle: create delta, run delta audits, apply DML, and clean up.

```sql
MODEL (
  description "One row per order",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_hour,
  cursor_type timestamp,
  cursor_grain hour,
  incremental_mode microbatch,
  microbatch_strategy watermark,
  cursor_watermark_mode all,
  cursor_inputs (
    fact_orders (column ordered_at, roles [filter, watermark]),
  ),
  batch_size 1d,
);
```

Without microbatch mode, the entire replay range is processed in one pass.

### Batch size

`batch_size` controls the window size for each batch. For timestamp cursors, use duration strings like `1d`, `6h`, `1mo`. For integer cursors, use an integer value.

For watermark models, each `cursor_inputs` entry declares whether the input filters model SQL,
contributes an availability watermark, or does both. `cursor_watermark_mode all` uses the
conservative common watermark; `any` permits progress from any watermark input. A filter-only input
does not claim that its intervals are available.

### Concurrent batches

Sequential execution is the default and does not read or write `_sqlbuild_microbatches`. To opt into
concurrent `delete_insert` batches, enable the project gate and choose a model limit:

```toml
[settings]
microbatch_concurrency = true
```

```sql
MODEL (
  description "Example model",
  materialized incremental,
  incremental_strategy delete_insert,
  incremental_mode microbatch,
  microbatch_strategy watermark,
  batch_concurrency 4,
  -- cursor configuration omitted
);
```

Concurrent execution uses immutable requirement and completion facts to coordinate dependencies and
reconcile retries. It does not update one mutable status row through planned/running/complete
states. Increase concurrency deliberately because every active batch can consume warehouse work.

### Watermark batch limits

Watermark microbatch models can declare what to do when their resolved range contains more batches than an ordinary run should process:

```sql
MODEL (
  description "One row per event",
  materialized incremental,
  incremental_strategy delete_insert,
  incremental_mode microbatch,
  microbatch_strategy watermark,
  cursor_watermark_mode all,
  cursor event_date,
  cursor_type timestamp,
  cursor_grain day,
  cursor_inputs (
    raw_events (column event_date, roles [filter, watermark]),
  ),
  batch_size 1d,
  microbatch_limit (
    max_batches 7,
    action cap_from_end,
  ),
);
```

| Action | Behavior when the range exceeds `max_batches` |
|--------|------------------------------------------------|
| `error` | Fail before hooks or model mutation |
| `warn` | Emit a prominent warning and execute the full range |
| `cap_from_start` | Execute the earliest N aligned batches and defer later work |
| `cap_from_end` | Execute the latest N aligned batches ending at the resolved watermark and defer older work |

A cap changes only the work selected for that invocation. Deferred batches are not recorded as complete. `cap_from_end` is useful for feeds where keeping the latest projection current is more important than catching up oldest-first; `cap_from_start` is the oldest-first catch-up policy.

Downstream watermark models consume durable completion facts from capped upstream models. A target
table's physical minimum/maximum does not prove that deferred or intervening intervals were built;
if completion evidence is unavailable, SQLBuild fails closed rather than inventing availability.

The project can also set an outer safety policy:

```toml
[microbatches.limits]
max_batches = 100
action = "error" # or "warn"
```

Project limits support only `error` and `warn`; they never silently cap work. When a model has a nested `microbatch_limit`, the project limit checks the full resolved range first and the model policy then applies.

`--max-microbatches N` is an invocation-wide, hard `error` ceiling and an explicit one-run authorization. It takes precedence over project and model limits, applies to models without a declared limit, and never inherits `cap_from_start` or `cap_from_end`. For example, passing a value large enough for an intentional backfill authorizes the full range instead of retaining the model's ordinary-run cap.

The legacy scalar `max_microbatches` model field remains supported as a fail/warn guard. New models should use the nested form when the action is part of the model's execution policy.

### Mixed-grain chains

When a downstream microbatch model reads from an upstream model with a coarser time grain, SQLBuild aligns the replay to the coarsest participating grain (the model's own grain and its cursor-input grains). This happens on every run that resolves cursor bounds from upstream models, not only when something changes. It is independent of the `replay_on_change` policy described below.

Alignment does two things: it floors the replay window edges to the coarsest grain, and it coarsens the batch size to that grain. For example, an hourly model downstream of a daily model processes in day-sized batches, so each batch lines up with a unit of upstream data that actually advances instead of producing empty or boundary-straddling windows:

```sql
-- Upstream: daily grain, 2d batches
MODEL (
  description "One row per order",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_day,
  cursor_type timestamp,
  cursor_grain day,
  cursor_inputs (
    hourly_order_activity activity_hour,
  ),
  incremental_mode microbatch,
  batch_size 2d,
);

-- Downstream: hourly grain, but aligns to day automatically
MODEL (
  description "Example model",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_hour,
  cursor_type timestamp,
  cursor_grain hour,
  cursor_inputs (
    daily_activity_rollup activity_day,
  ),
  incremental_mode microbatch,
  batch_size 6h,
);
```

## Replay on change

When a model's own query changes, a function it calls directly changes, or its own header, contract, or declared columns change, `replay_on_change` is the explicit, per-model policy for how much data to reprocess. Reprocessing is a policy you set, not an automatic forced rebuild, so a definition change does not silently trigger a full rebuild of large tables. Upstream changes never replay a downstream model. You choose the cost per model:

| Value | Effect |
|-------|--------|
| `forward` | Run the normal incremental delta from the cursor (default) |
| `full` | Full table rebuild |
| `bounded-14d` | Replay the last 14 days of data |

The bounded duration supports `d` (days), `h` (hours), `m` (minutes), and `s` (seconds). For example: `bounded-7d`, `bounded-24h`, `bounded-30m`.

```sql
MODEL (
  description "Example model",
  materialized incremental,
  incremental_strategy delete_insert,
  cursor activity_hour,
  cursor_type timestamp,
  cursor_grain hour,
  replay_on_change full,
);
```

See [Replay decisions](planning/replay-decisions.md) for what counts as a model's own change and how materialization types respond to upstream changes.

### on_schema_change

Controls how schema differences are handled at execution time when the incremental delta has different columns than the target table:

| Value | Effect |
|-------|--------|
| `append_new_columns` | Add new columns to the target table (default) |
| `sync_all_columns` | Add, drop, and alter columns to match the delta |
| `ignore` | Log and continue without schema changes |
| `fail` | Reject the build with an error |

Columns are compared by name, so a renamed column looks like one column removed and one added. [Column migrations](models/column-migrations.md) rename the warehouse column in place before `on_schema_change` runs, so its history keeps its values; the policy then applies only to the differences that remain. Pure renames such as `amount` becoming `amount AS revenue` are detected automatically, and `sqb plan` suggests `migrate_from` for near matches.
