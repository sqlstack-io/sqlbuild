<!-- generated-by: sqlbuild skills -->

# sqb dev

> Run the server, scheduler and a reloading code location for one project on your machine.

Online: https://sqlbuild.com/docs/cli/dev/

## Contents

- Options
- Set provider secrets
- Sign in to the warehouse
- Keys
- Plain output
- Run scheduled jobs locally
- Warehouse cost
- Details

Run the control API, the scheduler and a code location for the project in the current directory,
then open the web UI at the address on the ready card:

```bash
sqb dev
```

```text
  sqlbuild dev  v0.233.0   ready in 1.0s

  ➜  UI          http://127.0.0.1:8000
  ➜  Location    dev · gen 1 · 4 python nodes, 38 models
  ➜  Warehouse   ● duckdb · /work/shop/warehouse.duckdb
  ➜  Triggers    3 paused
  ➜  Watching    shop/
  ➜  Env file    .env · 2 variables · edit it, then press r to reload

  o open UI · r recompile · s retry warehouse · l logs · q quit
```

The location compiles the project, runs its [Rules](../concepts/rules.md), and publishes it to
the server. When you save a model, orchestration file or other project input, it compiles and
publishes again, so the UI and the next run use the new version. Each change, publication, failure
and run gets one line above the hotkey bar. A compile error or a Rule finding leaves the previous
version published, and its failure line names the error or the first finding.

## Options

| Option | Default | Effect |
| --- | --- | --- |
| `--port` | `8000` | Port to listen on, on `127.0.0.1`. `0` picks a free port |
| `--target` | The configured target | Target to compile and run |
| `--control-store-url` | `.sqlbuild/control.sqlite3` in the project | Control store URL or path |
| `--no-scheduler` | Off | Serve and reload without running scheduled work |
| `--triggers` | Off | Keep each trigger's default status and your changes to it, instead of pausing every trigger at start-up |

## Set provider secrets

`sqb dev` loads the project's [env file](../concepts/project-configuration.md#env-file), `.env`
by default, when it starts, and the **Env file** row on the ready card says how many variables
it set. After editing the file, press `r`: the location reads it again and restarts its Python
interpreter when a value changed, so the next compile and run use the new values. Variables set
in the shell that started `sqb dev` still win.

## Sign in to the warehouse

Once the first version is published, `sqb dev` connects to the target's warehouse in the
foreground. A client that asks for a password, a code or a browser sign-in asks once, in a framed
block on your terminal, and every later connection in the session reuses that sign-in.

If the connection fails, the card shows the error and the UI still works. Press `s` to try again.
The location reads warehouse history once the connection succeeds.

## Keys

| Key | Effect |
| --- | --- |
| `o` | Open the UI in your browser |
| `r` | Read the [env file](../concepts/project-configuration.md#env-file) again, then compile and publish |
| `s` | Retry the warehouse connection in the foreground |
| `l` | Show or hide the detailed log, the latest 500 lines |
| `q` | Stop, like Ctrl+C |

## Plain output

On Windows, when `NO_COLOR`, `CI` or `SQLBUILD_NO_PROGRESS` is set, or when stderr is not a
terminal, `sqb dev` prints one stable line per step, event and warning, with no redraws or keys:

```text
sqlbuild dev: control store ready (0.1s)
sqlbuild dev: location 01a1d7ff-0b3a-7631-ba35-e44d44d072f2 · target dev
sqlbuild dev: discovered 4 python nodes, 38 models (0.6s)
sqlbuild dev: published gen 1 01a1d7ff (0.1s)
sqlbuild dev: env file .env · 2 variables · edit it, then press r to reload
sqlbuild dev: ready http://127.0.0.1:8000
sqlbuild dev: warehouse read failed: retrying in 30s: CatalogException: Table _sqlbuild_fingerprints does not exist
```

Warnings cover failed warehouse reads, a stopped warehouse sync and unset provider secrets.

When stdout is redirected, it receives one JSON record per published version.

## Run scheduled jobs locally

Triggers start paused, so a schedule never runs on your machine by accident. Resume a trigger in
the UI to let it run, or start with `--triggers` to keep the status each trigger was declared with:

```bash
sqb dev --triggers
```

## Warehouse cost

While it runs, the location reads what has been built back from the warehouse after each
publication, after each run and at least every 30 minutes. In `sqb dev` every saved change is a
publication. These reads never write to the warehouse; what they cost depends on the adapter:

| Adapter | What a sync costs |
| --- | --- |
| Snowflake | A sync with nothing new does not resume a suspended warehouse. The first sync after each publication or restart resumes it, which bills Snowflake's 60-second minimum |
| BigQuery | Every sync runs one billed query job per state table, each charged at least 10 MB on demand |
| Databricks | Every sync runs SQL on the SQL warehouse, which starts it if it is stopped |
| PostgreSQL, SQL Server, ClickHouse, MotherDuck | No cost from SQLBuild itself. A serverless host that pauses when idle wakes for each sync |
| DuckDB, SQLite | None: the warehouse is a local file |

To read less often, set `[control] sync_at_least_every`, as described in
[Sync from the warehouse now](server.md#sync-from-the-warehouse-now). `"off"` stops the
timed reads but not the read after each publication or run.

## Details

- `sqb dev` listens only on loopback and needs no sign-in. Use [`sqb server`](server.md)
  for a server other people or machines connect to.
- The control store keeps runs, trigger changes and history between sessions. Delete
  `.sqlbuild/control.sqlite3` to start again from nothing.
- The location is named after the project, the `name` in `sqlbuild_project.toml`. When the
  control store already has a location with that name, for example from another target of the
  same project, it gets ` (2)`, then ` (3)`, and so on.
- The UI is ready once the project compiles and publishes. The location then reads SQLBuild's
  history from the warehouse; plan previews, history and costs show
  **Syncing warehouse history** until that first sync completes. See
  [Follow a sync](server.md#follow-a-sync).
- The control store keeps the newest 5 builds of each node and results of each check that it copies
  from the warehouse; older ones are deleted and the file shrinks. See
  [retention](server.md#set-retention).
- When `sqb dev` stops, it prints `Development services stopped successfully`. After a failure it
  prints the last log lines that explain it.
