<!-- generated-by: sqlbuild skills -->

# sqb janitor

> Archive and then delete stale warehouse relations.

Online: https://sqlbuild.com/docs/cli/janitor/

## Contents

- Usage
- Flags
- Lifecycle
- What else the janitor cleans
- Safety
- Targets
- Configuration
- Output
- Examples

`sqb janitor` cleans up relations your project no longer builds. It never drops a live relation
directly: it first renames a stale relation to an archive name, and drops the archive only once it
expires. It shows the plan and asks before changing anything.

```bash
sqb janitor
```

```
Archiving 3 relations...
  1/3  archived  analytics.old_orders -> analytics._sqb_archive__20261022t101500z__old_orders        OK  0.21s
  2/3  archived  analytics.old_customers -> analytics._sqb_archive__20261022t101500z__old_customers  OK  0.19s
  3/3  archived  analytics.old_products -> analytics._sqb_archive__20261022t101500z__old_products    OK  0.24s
Archived 3 relations. (0.64s)
Deleting 1 expired archive...
  1/1  deleted   analytics._sqb_archive__20261001t080000z__old_inventory  OK  0.12s
Deleted 1 expired archive. (0.12s)

✓ Janitor completed  archived 3, deleted 1, pruned 0  (0.81s)
```

The janitor is off until you set `enabled = true` under [`[janitor]`](#configuration).

## Usage

```bash
sqb --project-dir <path> janitor [flags]
```

## Flags

| Flag | Description |
|------|-------------|
| `--target <name>` | Clean up another configured target, with that target's connection, schemas and state |
| `--as <name>` | Preview the plan for another target's schemas through the active target's connection. Changes nothing and doesn't prompt. Can't be combined with `--auto-approve` |
| `--auto-approve` | Run the plan without the confirmation prompt |
| `--retention-days` | Override `retention_days` for this run |
| `--direct-state-history-versions` | Override how many state-history versions are kept per identity |
| `--keep-in-use` | Keep relations read within `in_use_window` instead of archiving them, for this run. See [Relations in use](#relations-in-use) |
| `--drop-old-name-view <name>` | Drop the [compatibility view](../concepts/models/migrations.md#old-names) at a renamed model's old name before it expires. Repeat for several views. Fails with `C503` if no compatibility view is recorded at the name |
| `--verbose`, `-v` | List every entry of each preview section and every skipped object with its reason |
| `--json` | Print the plan and the result as one JSON document on stdout; the preview, prompt and progress go to stderr |
| `--warehouse <name>` | Snowflake warehouse for this run; overrides the target's `query` [warehouse group](../concepts/project-configuration.md#command-group-warehouses) and the connection warehouse |

## Lifecycle

A relation is **stale** when all of these are true:

- it is in a schema the project writes to;
- the current, complete project doesn't produce it;
- SQLBuild tracked it, when `delete_tracked_only = true` (the default);
- it matches no `exclude_patterns` entry, ignoring case.

A stale relation goes through two stages:

1. **Archive.** Once it has been stale for `retention_days` (default `14`), the janitor renames it,
   in the same schema, to `_SQB_ARCHIVE__<UTC timestamp>__<original name>`. For example,
   `stg_orders` becomes `_SQB_ARCHIVE__20260924T101500Z__stg_orders`. A rename copies no data and
   keeps the relation's table type, grants and time travel.
2. **Delete.** Once the archive is older than `archive_retention_days` (default `14`), the janitor
   drops it.

Setting both to `0` archives and deletes stale relations in the same run.

Archive names are written as unquoted lowercase identifiers, so warehouses that fold unquoted names
show them in their own case; on Snowflake the example above appears as
`_SQB_ARCHIVE__20260924T101500Z__STG_ORDERS`. Long names are shortened with a short hash to fit the
adapter's identifier limit. The prefix and timestamp are never shortened.

An archive's age comes from the timestamp in its name, so deleting needs no other state.

### Relation age

`retention_days` counts from the last time the relation was written, as the warehouse reports it.
For a relation the project no longer builds, that is roughly when it stopped being built.

| Adapter | Age source |
|---------|------------|
| Snowflake | `INFORMATION_SCHEMA.TABLES` `CREATED` and `LAST_ALTERED`, which change on every write |
| Databricks | `DESCRIBE DETAIL` `lastModified` for tables, and `INFORMATION_SCHEMA.TABLES` for views |
| BigQuery | `__TABLES__` `creation_time` and `last_modified_time` |
| PostgreSQL, SQL Server, DuckDB, MotherDuck | Not available |

When the warehouse can't report an age, the janitor skips the relation unless `retention_days` is
`0`. SQL Server is in this group because its modification date ignores inserts and updates, so a
recently written table could look old.

### Restoring an archive

Rename the archive back to its original name before it expires. For example, on Snowflake:

```sql
ALTER TABLE analytics._SQB_ARCHIVE__20260924T101500Z__stg_orders RENAME TO analytics.stg_orders;
```

If the archive's name was shortened, look up the original name in the
[audit record](#audit-record). After an archive is deleted, your warehouse's time travel or undrop
feature may still recover it within its own retention window.

## What else the janitor cleans

Each run works through these phases, in order:

1. archive stale relations;
2. delete expired archives;
3. drop expired [compatibility views](#compatibility-views);
4. drop expired query artifacts;
5. drop leftover [review schemas](#review-schemas);
6. [prune state history](#state-history-pruning).

### Compatibility views

When a model is renamed, SQLBuild keeps a [compatibility view](../concepts/models/migrations.md#old-names)
at its old name. The janitor lists each one with the model it reads and its expiry:

```
Old name views (1)
└── prod.revenue  -> model:daily_revenue
    └── expires  2026-10-28  (live; drop early with --drop-old-name-view prod.revenue)
```

A compatibility view is never archived as a stale relation. Once it expires, or when you name it
with `--drop-old-name-view`, the janitor drops it and records the drop in
`_sqlbuild_old_name_views`.

The janitor drops the view only if the relation at the old name is still the view SQLBuild created.
Any other relation there, such as a view a project model built after an interrupted early drop, is
left alone. The janitor then records the compatibility view as dropped and lists it as
`record  dropped  (name now used by another relation, model:revenue)`, naming the model when the
name belongs to one. A compatibility view that no longer exists is recorded as dropped too.

### Review schemas

[`sqb review`](review.md) builds into a schema it creates and drops for one run. When a
review couldn't drop its schema, the janitor drops it once the timestamp in its name is more than 24
hours old, so a running review is never touched. Only schemas with a review ownership marker in the
active target schema's `_sqlbuild_fingerprints` table are dropped; a schema whose name only looks
like a review schema is left alone.

### State history pruning

The janitor prunes old rows from `_sqlbuild_fingerprints` and `_sqlbuild_source_freshness`, keeping
the latest `direct_state_history_versions` rows per identity (default `20`), and from
`_sqlbuild_consumption`, keeping the latest `direct_state_history_versions` builds of each model and
seed. This is separate from relation retention.

## Safety

- **You confirm the plan.** The preview lists the planned archives and deletes with exact names,
  ages and expiry, then asks you to type an exact confirmation phrase. Use `--auto-approve` only in
  automation, or when you're certain.
- **Only exact archive names are deleted.** An archive is deleted only when its name strictly
  matches the archive format and it is in a schema the project writes to. Names that only look
  similar are reported and never touched.
- **Unusual names are left alone.** The janitor acts only on plain lowercase identifiers (letters,
  digits and underscores) that don't collide, ignoring case, with another relation in the same
  schema. Anything else is reported and skipped.
- **Pending migration origins are kept.** While a model declares `migrate_from` and its own
  relation doesn't exist yet in the target, the origin isn't archived and is listed as
  `pending migration origin for <model>`. A bare `migrate_from` name is matched in each schema the
  project builds into; a qualified name only at that location. Once the model has built, the old
  name holds a compatibility view, which expires as above.
- **Schemas with sources are protected.** A schema that holds the project's configured sources is
  skipped and listed under `Skipped schemas`, so what the project reads is never archived. In
  direct mode, when such a schema is also one SQLBuild builds into, the preview shows
  `Janitor blocked`, lists the actions it suppressed, and the run exits `1`.
- **Run the same SQLBuild version as your builds.** An older janitor doesn't know compatibility
  views and archives them as stale relations.

### Relations in use

Archiving renames a relation, so anything that still reads it fails from then on. Before
archiving, the janitor checks which candidates other queries read within `in_use_window` (default
`7d`) and lists them, with their readers:

```
In use
  analytics.orders_daily  queried 2026-10-21 by svc_reporting, alice (14 queries in 7 days)
  analytics.old_orders  queried 2026-10-20 by svc_reporting, alice, bob +2 more (40 queries in 7 days); frozen since 2026-09-01, still in use
  analytics.old_customers  queried 2026-10-21 by alice via analytics.customer_report (3 queries in 7 days); frozen since 2026-09-01, still in use
```

Each line shows the latest read, up to three readers (most queries first) and the query count.

- Reading a view also reads what it's built on, so those relations are in use too. A relation read
  only that way shows `via` and the views, or `through a view` when the warehouse doesn't record
  which.
- A stale table that is still read is flagged `frozen since <date>, still in use`: its data stopped
  changing on that date, so its readers get old data. Views aren't flagged, because their reads stay
  live.
- The prompt states the count, for example
  `14 relations will be archived; 3 of them are in use.`

By default, in-use relations are still archived, which is also how their readers find out. To keep
them, set `keep_in_use = true` or pass `--keep-in-use`. They are then listed under `Kept (in use)`
and left out of the archive list and the counts. A later run archives them once nothing reads them
within the window. Only relations to archive are checked, not expired archives or state pruning.

Usage comes from the warehouse's query history:

| Adapter | Source | Grants | Caveats |
|---------|--------|--------|---------|
| Snowflake | `ACCOUNT_USAGE.ACCESS_HISTORY` with `QUERY_HISTORY`, and `OBJECT_DEPENDENCIES` for view chains | `IMPORTED PRIVILEGES` on the `SNOWFLAKE` database, or the `SNOWFLAKE.GOVERNANCE_VIEWER` and `SNOWFLAKE.OBJECT_VIEWER` database roles | Needs Enterprise edition. Lags up to about 3 hours |
| BigQuery | `` `<project>.region-<location>`.INFORMATION_SCHEMA.JOBS `` of each candidate's project, in its dataset's location | `bigquery.jobs.listAll` and `bigquery.jobs.create` on each project, for example BigQuery Resource Viewer and BigQuery User | Keeps 180 days of jobs |
| Databricks | `system.access.table_lineage` | `USE CATALOG` on `system`, `USE SCHEMA` on `system.access`, `SELECT` on `system.access.table_lineage` | Needs Unity Catalog with the `access` system schema. Keeps one year of events |
| ClickHouse | `system.query_log` of the connected server | `SELECT` on `system.query_log` | Only the connected server's log is read |
| Other adapters | Not available | | |

```sql
-- Snowflake
GRANT DATABASE ROLE SNOWFLAKE.GOVERNANCE_VIEWER TO ROLE janitor_role;
GRANT DATABASE ROLE SNOWFLAKE.OBJECT_VIEWER TO ROLE janitor_role;

-- Databricks
GRANT USE CATALOG ON CATALOG system TO `janitor-principal`;
GRANT USE SCHEMA ON SCHEMA system.access TO `janitor-principal`;
GRANT SELECT ON TABLE system.access.table_lineage TO `janitor-principal`;

-- ClickHouse
GRANT SELECT ON system.query_log TO janitor_user;
```

When usage can't be checked, because the adapter has no source or the role can't read it, the
preview and the prompt show one notice, such as `usage not checked: unsupported adapter`, and the
janitor archives as usual. With `keep_in_use` on, a second notice, `nothing kept in use`, says that
in-use relations are archived too.

#### What counts as a read

**Snowflake.** A relation read through the objects a query named shows `via` those objects.
Everything a directly read object is built on, through any number of views, is in use, so a
candidate view between a view someone reads and a candidate table is kept too. Links through views
in other databases aren't followed, and dependencies created by `CREATE TABLE ... AS SELECT` and
similar statements aren't recorded. These don't count as reads:

- queries SQLBuild tags as its own (`QUERY_TAG` with `"app":"sqlbuild"`, set by builds, audits,
  loads and hooks);
- `CREATE ... CLONE` of tables, dynamic tables, schemas and databases. A statement counts as a clone
  only when it starts with `CREATE`, after any comments, and names its source after `CLONE`.

Without `OBJECT_VIEWER`, reads are still checked but view chains aren't followed, and the preview
says `view chains not followed: grant SNOWFLAKE.OBJECT_VIEWER`.

**BigQuery.** Successful query jobs that reference the relation count, by `user_email`. A job lists
the views it reads and the tables behind them, so both count. These are missed or don't count:

- a candidate view read only through another view may not show, because BigQuery doesn't document
  whether nested views are listed;
- jobs answered from cached results record no tables, so with an `in_use_window` under about a day
  a reader whose queries were all cached can be missed;
- jobs that run in another project aren't seen;
- script parent jobs, `CREATE SNAPSHOT TABLE` and `CREATE TABLE ... CLONE` don't count;
- SQLBuild's own jobs count like any other.

**Databricks.** Read events in Unity Catalog's table lineage count, by `created_by`. A view a query
reads is a direct read, and the tables behind it are reads through the view. Lineage covers only
reads Databricks can infer lineage for, and can arrive late, so "not in use" is a weaker signal than
on Snowflake. Clones and SQLBuild's own reads count as reads.

**ClickHouse.** Finished `SELECT` and `INSERT` queries count, by the user who started them, once per
initial query. A query lists every table and view it touched, so a whole view chain shows as in
use, without `via`. Writes count, since renaming the relation breaks the writer too. These are
missed or don't count:

- queries on other replicas or shards, which matters on ClickHouse Cloud and behind load-balanced
  replicas. When `system.clusters` shows other replicas of the connected server's shard, the preview
  says `usage read from this server only; other replicas not checked`;
- queries ClickHouse answers from metadata without logging the table, such as a plain `count()`;
- readers whose settings profile turns `log_queries` off or samples it with
  `log_queries_probability`. When `log_queries` is off for the janitor's own connection, usage isn't
  checked;
- queries in the last few seconds: the log is flushed about every 7.5 seconds;
- queries older than the server's `query_log` TTL;
- SQLBuild's own reads count.

### Audit record

Every archive and delete is recorded in an append-only `_sqlbuild_janitor_events` table in the
affected schema: the original name, the archive name, the time and the invocation. The janitor
never reads it to decide what to do; it is for auditing, and for restoring archives whose names
were shortened, which are recorded right after their rename. Other events are written in batches
of up to 100 per schema, so if the process is killed, up to 100 actions in a schema may have no
event. Those archives still expire by the timestamp in their names.

## Targets

By default the janitor cleans the active target. `--target <name>` selects another configured
target, exactly like `sqb build --target`. Other targets are never touched.

`--as <name>` previews another target without changing anything, like
[`sqb plan --as`](plan.md). The project compiles for that target, and its schemas are
inspected through the active target's connection. The preview lists what would be archived,
deleted, dropped and pruned, then exits. It drops nothing, writes no audit or state rows and doesn't
prompt. `--drop-old-name-view` with `--as` only previews the drop.

```
Previewing janitor as target 'prod' through the connection of target 'dev' (inspection only).
...
Relations to archive
  prod.customers  ->  prod._sqb_archive__20261001t101500z__customers  age unknown, delete after 2026-10-15 10:15:00 UTC

Archives to delete
  prod._sqb_archive__20200101t000000z__old_products  archived 2020-01-01 00:00:00 UTC, age 2465d, expired 2020-01-15 00:00:00 UTC

Previewed janitor as target 'prod' through the connection of target 'dev'. Nothing was changed.
Rerun with `--target prod` instead of `--as prod` to apply it.
```

An unknown target name fails before anything connects. A preview exits `0`, or `1` when a schema
is blocked, as a normal run would.

## Configuration

```toml
[janitor]
enabled = true
retention_days = 14
archive_retention_days = 14
delete_tracked_only = true
exclude_patterns = ["audit_*", "tmp_*"]
in_use_window = "7d"
keep_in_use = false
```

See [Project Configuration](../concepts/project-configuration.md#janitor) for every key and default.
On Snowflake, BigQuery and Databricks, the actions of each phase run on up to the target's
[`concurrency`](../concepts/project-configuration.md#target-concurrency) connections at once.

## Output

After the preview and confirmation, each phase with work prints how many actions it runs, one line
per action, and its duration, as in the example at the top. The run ends with one line of totals.

If an action fails, its line shows `FAIL` and the error, no further action starts, and the run ends
with `✗ Janitor failed` and the totals completed so far. The exit code is `1`.

Without `--verbose`, each preview section shows its first 10 entries and then
`... and N more (use --verbose to show all)`, and skipped objects are counted by reason:

```
Skipped objects (385)
  210  relation is newer than 14 days
  175  relation is not tracked by SQLBuild
```

### JSON output

With `--json`, stdout holds one document:

| Field | Contents |
|-------|----------|
| `status` | `completed`, `failed`, `cancelled`, `blocked`, `nothing_to_do`, `previewed` or `disabled` |
| `plan` | The full plan |
| `usage_check` | `status` (`checked` or `not_checked`), `reason`, `window`, `keep_in_use` and `notices` (what a check left out, such as view chains it couldn't follow); `null` when there was nothing to check |
| `result` | Counts and every action |
| `error` | The error, if any |

Each entry of `plan.relations_to_archive` and `plan.kept_in_use` has `in_use`:

- `null` when usage wasn't checked;
- `false` when nothing read the relation within the window;
- otherwise an object with `last_read`, `readers`, `query_count` and `via` (`null` for a direct
  read, otherwise the views it was read through, empty when unknown).

Entries of `kept_in_use` have only `name`, `age_days` and `in_use`. Errors before the cleanup
starts, such as a refused confirmation, go to stderr only.

## Examples

```bash
# Show the plan and prompt for confirmation
sqb janitor

# Run the plan without prompting
sqb janitor --auto-approve

# Archive relations stale for 7 days instead of the configured retention
sqb janitor --retention-days 7

# Preview the prod cleanup through the current connection, changing nothing
sqb janitor --as prod

# Clean up the prod target
sqb janitor --target prod

# Keep relations that are still read instead of archiving them
sqb janitor --keep-in-use
```
