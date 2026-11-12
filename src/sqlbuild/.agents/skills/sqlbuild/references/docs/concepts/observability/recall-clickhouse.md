<!-- generated-by: sqlbuild skills -->

# Sharing Recall in ClickHouse

> Keep one Recall history for a team or CI in ClickHouse: users, grants, setup, retention and reading.

Online: https://sqlbuild.com/docs/concepts/observability/recall-clickhouse/

## Contents

- Users and grants
- Setting up and upgrading the store
- Retention in ClickHouse
- Reading a ClickHouse store

Point `write_connection` and `read_connection` at ClickHouse connections to keep one history for a
whole team. Every machine
that writes to it, including CI and scheduled production runs, becomes queryable from anywhere:
see what last night's production build ran and printed without access to the machine, find the
first run where an audit started failing across everyone's runs, or compare a production run with
your own by `target`, `git_revision` or `sqlbuild_version`.

The connection needs `adapter = "clickhouse"` when the project uses another adapter. Recall creates
and upgrades its own store on the first write: the `database` when it does not exist yet, then its
tables, views and schema version. A writer that may run that DDL needs no other setup.

```toml
[connections.history_writer]
adapter = "clickhouse"
host = "${env:CLICKHOUSE_HOST}"
user = "recall_writer"
password = "${env:RECALL_WRITER_PASSWORD}"
database = "observability"

[connections.history_reader]
adapter = "clickhouse"
host = "${env:CLICKHOUSE_HOST}"
user = "recall_reader"
password = "${env:RECALL_READER_PASSWORD}"
database = "observability"

[recall]
write_connection = "history_writer"  # every command writes history; creates the store on first use
read_connection = "history_reader"   # sqb recall query, schema and output (SELECT only)
retention = "30d"
```

`sqb recall query`, `schema` and `output` use only `read_connection` and never create or change
anything in the store, so someone investigating production runs needs no credentials that could
rewrite or delete its history. Both keys may name one connection when separate users are not
wanted. A machine that only reads shared history, such as a laptop investigating production runs,
sets only `read_connection`; its own commands keep recording to the local store. Each side
resolves only its own connection, so environment references in the other connection, such as the
writer's password on a reader's laptop, need not be set.

## Users and grants

SQLBuild doesn't enforce grants; it lets you separate the roles. Give the writer the rights to
create and upgrade the store in its database, so the first command sets it up and later SQLBuild
versions upgrade it without anyone running DDL by hand:

```sql
-- once, as an administrator: the users
CREATE USER recall_writer IDENTIFIED BY '...';
CREATE USER recall_reader IDENTIFIED BY '...';
GRANT CREATE DATABASE, CREATE TABLE, CREATE VIEW, DROP TABLE, ALTER ADD COLUMN, SELECT, INSERT
  ON observability.* TO recall_writer;
GRANT SELECT ON observability.* TO recall_reader;
```

`DROP TABLE` is needed because ClickHouse replaces a view through a temporary table; Recall never
drops a table itself.

| User | Rights | What happens |
|------|--------|--------------|
| Writer with the grants above | Create and upgrade the store | The first command creates the store; later versions upgrade it |
| Writer with only `SELECT` and `INSERT` | Write history | Works once the store exists. A missing or older store makes Recall print the statements an administrator should run |
| Reader with `SELECT` | Read history | `sqb recall query`, `schema` and `output` work, and can't change the store |

A reader may be a standard read-only user (`SETTINGS readonly = 1`, or a settings profile with
it). Its queries run with the server's own settings; every other reader gets the
[settings SQLBuild pins](../adapters/clickhouse.md#settings-sqlbuild-pins). The views
return the same rows either way. `sqb recall query` runs with `readonly = 2`, which refuses writes
but allows query settings, unless the user is already read-only. A user that may not set the row
limit gets the same truncated result, cut off while reading. A setting Recall needs that the user
may not change fails with an error naming it: set it in the user's profile, or give the user
`readonly = 2`, which keeps it read-only.

## Setting up and upgrading the store

- **Setup.** On first use, the writer creates the database if it's missing, printing
  ``Recall: created ClickHouse database `observability` for the store.`` once, then the store's
  tables and views. An existing database is used as it is, whatever its engine.
- **Clusters.** The database is created on the server the writer connects to, never `ON CLUSTER`.
  On a self-managed cluster whose default database engine isn't `Replicated`, create the database
  yourself first with `ENGINE = Replicated(...)`; Recall leaves it alone. ClickHouse Cloud needs
  nothing. `ON CLUSTER` stores aren't supported: use a single ClickHouse server or ClickHouse Cloud.
- **Without DDL rights.** If the writer may not create or upgrade the store, the command still
  runs and exits as it otherwise would. Recall prints the statements an administrator should run, at
  most once a day, starting with `CREATE DATABASE IF NOT EXISTS` when the database is missing.
  `sqb recall schema --ddl` prints them again. A store behind the current version still receives
  history, in the columns that exist.
- **Mixed versions.** A writer never replaces the views of a store that a newer SQLBuild version
  created.
- **Many writers.** Many concurrent commands can write one store. A repeated delivery is counted
  once. A record written again with different content keeps the stored row, and the rejected copy
  is listed in the `conflicts` view.
- **Writes.** History is sent about every five seconds and when the command ends, with the same shutdown
  budget and exit-code guarantees as the local store.

## Retention in ClickHouse

Retention belongs to the store. Every table has a TTL of `retention`; SQL text is kept a week
longer, and for as long as a kept invocation references it. The views hide expired rows at once,
but ClickHouse drops expired parts in the background, so rows may stay on disk for a while.

When a project's `retention` differs from the tables' TTL, Recall warns once a day with the
`ALTER TABLE … MODIFY TTL` and view statements that would apply it. It never changes the TTL
itself.

## Reading a ClickHouse store

`sqb recall query`, `schema` and `output` read the ClickHouse store, so queries use ClickHouse SQL.
`--json` output includes the server's `rows_read`, `bytes_read` and `query_id`.
