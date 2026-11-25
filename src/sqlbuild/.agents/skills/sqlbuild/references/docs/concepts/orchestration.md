<!-- generated-by: sqlbuild skills -->

# Orchestration

> Declare jobs, job groups and schedules in TOML, and calendars and sensors in Python, for a SQLBuild server to run.

Online: https://sqlbuild.com/docs/concepts/orchestration/

## Contents

- Project layout
- Declare a job
- Group jobs
- Schedule a job
- Schedule from a calendar
- React with a sensor
- Rename models in jobs
- Errors

A job names the nodes a server builds together, and a schedule says when. Write each job as a TOML
file under `orchestration/jobs/`; `sqb compile` checks it and a code location publishes it.

```toml
# orchestration/jobs/nightly_orders_job.toml
name = "nightly_orders_job"
description = "Orders and their daily totals"
select = "+order_totals"
schedule = { cron = "0 2 * * *", timezone = "UTC" }
```

```bash
sqb build nightly_orders_job        # build what the job builds, locally
```

## Project layout

| Folder | Holds | Name suffix |
| --- | --- | --- |
| `orchestration/jobs/` | One job per `.toml` file | `_job` |
| `orchestration/job_groups/` | One job group per `.toml` file | `_job_group` |
| `orchestration/calendars/` | One calendar per `.py` module | `_calendar` |
| `orchestration/sensors/` | One sensor per `.py` module | `_sensor` |

- Subfolders are allowed, such as `orchestration/jobs/finance/revenue_job.toml`.
- A name must end with its folder's suffix. Names are unique across the whole project, with models,
  sources and every other resource.
- A Python module whose file name starts with `_`, or that sits in a folder starting with `_` under
  `orchestration/calendars/` or `orchestration/sensors/` (such as `sensors/inventory/_helpers/`), is
  private support and declares nothing. Declaring a calendar or sensor there fails compilation.
- Any other file under `orchestration/`, or a folder not in the table, is a `D017` error.
- The [Python Rules](rules/python-rules.md#laying-out-sensors-and-calendars) add a
  domain-folder layout for sensor and calendar modules.

## Declare a job

A job builds its `select` minus its `exclude`, with the same [selectors](selectors.md)
as `sqb build`.

```toml
name = "nightly_orders_job"
description = "Orders and their daily totals"
select = "+order_totals"
exclude = "tag:deprecated"
targets = ["production"]
owner = "Orders team"
overlap = "skip"
pools = { warehouse_heavy = 1 }
schedule = { cron = "0 2 * * *", timezone = "Europe/London" }

[retry]
attempts = 3
delay_seconds = 600

[settings]
concurrency = 4
vars = { batch_size = 500 }

[resources]
memory = "3g"
cpus = 2
```

| Key | Required | Value |
| --- | --- | --- |
| `name` | Yes | The job's name, ending in `_job` |
| `description` | Yes | Shown on the job's page |
| `select` | Yes | The nodes the job builds |
| `exclude` | No | Nodes removed from `select` |
| `targets` | No | The targets the job exists in; all targets when omitted |
| `owner` | No | Shown on the job's page and in the job list |
| `overlap` | No | `queue` (default) or `skip`; see [Skip overlapping runs](../cli/server.md#skip-overlapping-runs-and-retry-failures) |
| `pools` | No | Slots the job takes of each [pool](concurrency.md#jobs-sharing-something-else-pools), such as `{ warehouse_heavy = 1 }` |
| `schedule` | No | When the server starts the job; see below |
| `[retry]` | No | `attempts`, the most runs in all, from 2 to 11, and `delay_seconds` from 0 to 86400; see [Retry failures](../cli/server.md#skip-overlapping-runs-and-retry-failures) |
| `[settings]` | No | Build options for each run, such as `concurrency`, `vars`, `full_refresh`, `changed` or `ignore_audit_failures` ([Ignoring audit failures](audits.md#ignoring-audit-failures)) |
| `[resources]` | No | Docker worker limits; see [Limit worker containers](../cli/server.md#limit-worker-containers) |

A job's `[settings]` cannot set `select` or `exclude`: the job's own selection is the run's
selection.

### Settings when a build names a job

A build or plan whose selection names a job, as `job:nightly_orders_job`, the bare name or a
`job:` pattern such as `job:nightly_*`, takes the job's `[settings]`. So do ad-hoc builds and plan
previews through `sqb server`; see [Run ad-hoc commands](../cli/server.md#run-ad-hoc-commands). Selecting the same nodes some other way, such as by tag, takes none of them.
Naming a group names each of its jobs. The first value set wins:

1. A flag on the command line, such as `--vars` or `--concurrency`
2. The named job's setting
3. The target's setting
4. The project's setting

A command-line flag replaces every named job's setting rather than merging with it, so it also
settles named jobs that disagree: `--vars` replaces the job's whole `vars`, `--full-refresh`
rebuilds every selected model, and `--warehouse` puts every node on its warehouse. An option that rules another out counts as setting both, so `--reload` wins
over a job's `load_sources = false`. Any `--concurrency` sets the build's limit and turns off the
per-job limits. `SQLBUILD_CONCURRENCY` is not a command-line flag: it sets the build's limit as a
configured one does, and the named jobs' limits still apply. The job's settings are added before a `--` on the command line, so selectors after
it still name jobs. `--no-job-settings` leaves out every job's settings.

Some settings apply to the job's own nodes only, so a build that names two jobs can treat their
nodes differently:

| Setting | Applies to |
| --- | --- |
| `full_refresh` | The job's incremental models, rebuilt from scratch, and its functions, recreated |
| `run_tests` | The SQL unit tests of the job's models |
| `ignore_audit_failures` | The audits of the job's nodes, and singular audits that read them; a singular audit is ignored only when every node it reads ignores failures, by its job's setting or else the build's |
| `warehouse` | The Snowflake warehouse the job's nodes run on, Python nodes included; see [A job's own warehouse](adapters/snowflake.md#a-jobs-own-warehouse) |
| `concurrency` | How many of the job's nodes build at once, and a node in two named jobs fits within both. The build's own limit becomes the larger of the configured limit and the named jobs' limits added together |
| `[retry]` | The job's nodes |

A job's settings reach only the nodes its own selection holds. `--select job:nightly_orders_job,orders`
gives them to `orders` alone, and `+job:nightly_orders_job` builds the job's upstream nodes without
them.

Every other setting, such as `vars`, `changed` or `fail_fast`, applies to the whole build, so every
named job must set it the same way; a job that leaves it unset disagrees with one that sets it.
Pass the flag to settle a disagreement. When every named job sets the same `warehouse`, it is the
whole build's warehouse, as if passed as `--warehouse`; see
[A job's own warehouse](adapters/snowflake.md#a-jobs-own-warehouse).

Each job in a group still runs as its own build, so a group's jobs must agree only on `vars` and
`changed`, which change the compiled SQL. A group whose jobs set either differently on a target
that declares them together fails to compile, with the error a build of the group would give.

Two named jobs that give one node different values of the same setting fail the build with `S027`,
naming the node, both jobs and the setting. A setting only one of them sets is no conflict.

`sqb plan --json` names what the jobs give the build: `full_refresh_jobs` lists the jobs whose
models rebuild from scratch, `job_warehouses` maps each job to the warehouse its nodes run on, and
`job_concurrency` maps each job to how many of its nodes build at once. `job_concurrency` is left
out when `--concurrency` turns the per-job limits off.

Select a pipeline by its folder or a tag rather than listing its nodes. Rule `SQBRORCHESTRATION101`
reports a job or sensor `select` that names more than three nodes one by one; see
[Orchestration Rules](rules/orchestration-rules.md).

## Group jobs

A job group runs its jobs as one run. A job waits for another job in the group when it reads a node
that job builds, directly or through nodes no job in the group builds. It waits only for the
nearest such jobs: what they read is their own wait, so a node further upstream, such as a function
many jobs build, adds no more waits. A job never waits for the other jobs that build a node it
builds itself. Jobs that wait on each other, directly or through other members, are a cycle, and
`sqb compile` reports every cycle in the group as one `D017` error.

```toml
# orchestration/job_groups/daily_orders_job_group.toml
name = "daily_orders_job_group"
description = "Stage orders, then build the marts"
jobs = ["staging_orders_job", "orders_marts_job"]
schedule = { cron = "0 3 * * *", timezone = "UTC" }
```

A group takes `name`, `description` and `jobs`, and the same `targets`, `owner`, `overlap`,
`pools`, `schedule` and `[retry]` as a job. Its members are jobs, never other
groups.

When a location publishes, it records why each member waits, node by node: a node of the earlier
job feeds a node of the waiting job, directly or through nodes no member selects, such as a source
between a loader job and the models that read it. The job's page shows these reasons.

A job that reads a seed or a node that several members build waits for all of those members. Give
seeds and shared nodes their own job, so the jobs that read them wait only for that job.

### Choose what happens after a failure

`[dependencies]` sets how a member reacts when a job it depends on fails:

```toml
# orchestration/job_groups/nightly_job_group.toml
name = "nightly_job_group"
description = "Orders, inventory and the partner export"
jobs = ["orders_job", "inventory_job", "partner_export_job"]
schedule = { cron = "0 21 * * *", timezone = "UTC" }

[dependencies]
wait_for = "job"
on_failure = "partial"

[dependencies.partner_export_job]
on_failure = "skip"
```

| Setting | Value | Meaning |
| --- | --- | --- |
| `wait_for` | `job` (default) | A member starts once the jobs it reads from have finished |
| `on_failure` | `partial` (default) | Build what can still be built; nodes downstream of a failed node are skipped |
| | `skip` | Don't run the member; all its nodes end skipped and the member ends `skipped` |
| | `run` | Run every node of the member regardless |

- `[dependencies.<job>]` overrides the group's settings for one member, and must name a job in
  `jobs`. Overrides live in the group file because a job can belong to several groups.
- A skipped member raises no failure alert and fires `run_event` sensors with `kinds=["skipped"]`,
  not `"failed"`. The jobs that depend on it react to it as they would to a failure, since what
  they read was not built.
- A member job's own `[retry]` runs it again inside the group run, and the jobs that depend on it
  wait for its final attempt before `on_failure` applies. A group's `[retry]` reruns the whole
  group's failures once the group run ends; see
  [Retry failures](../cli/server.md#skip-overlapping-runs-and-retry-failures).
- `wait_for = "node"`, which will start each node as soon as the nodes it reads are built, is
  planned but not available yet; `sqb compile` refuses it, so use `"job"` for now.

## Schedule a job

`schedule` takes a cron expression or the name of a calendar, and always a timezone.

| Key | Value |
| --- | --- |
| `cron` | A five-field cron expression |
| `calendar` | The name of a calendar in `orchestration/calendars/` |
| `timezone` | An IANA timezone, such as `"UTC"` or `"America/New_York"` |
| `starts_at` | An RFC 3339 time with an offset; no ticks before it |
| `default_status` | `running` (default) or `paused` when the schedule is first published |

```toml
schedule = { cron = "15 2 * * *", timezone = "Europe/London", starts_at = "2026-07-03T00:00:00Z" }
```

## Schedule from a calendar

A calendar returns the upcoming times a job runs, such as fulfillment cut-offs that come from
another system. The code location evaluates it when it publishes and again before its times run
out.

```python
# orchestration/calendars/fulfillment/cutoffs.py
from sqlbuild.orchestration import calendar

@calendar(name="fulfillment_calendar", description="Upcoming fulfillment times", horizon="1d")
def fulfillment_calendar(window, fulfillment):
    return fulfillment.upcoming(window)
```

```toml
schedule = { calendar = "fulfillment_calendar", timezone = "America/New_York" }
```

The function receives a `window` with timezone-aware `start` and `end`, in the job's timezone, and
returns timezone-aware datetimes inside it. Other parameters name
[providers](python-nodes/providers.md). `horizon` sets how far ahead the window reaches.

## React with a sensor

A sensor is a function SQLBuild calls when something happens, such as a job run failing. The
function can start runs, send a message through a provider, or both:

```python
# orchestration/sensors/orders/orders_failed_sensor.py
from sqlbuild.orchestration import RunEvent, RunEventKind, RunRequest, SensorContext, run_event, sensor

@sensor(
    description="Tell the orders team, and rebuild the export, when the nightly orders job fails",
    when=run_event(job="nightly_orders_job", kinds=[RunEventKind.FAILED]),
)
def orders_failed_sensor(ctx: SensorContext[RunEvent], team_chat):
    team_chat.post(f"{ctx.event.job} failed: {ctx.event.error} {ctx.event.url or ''}")
    return RunRequest(job="orders_export_job")
```

`team_chat` is a [provider](python-nodes/providers.md) the project defines; the
function names it as a parameter, as a Python node does. The sensor is named after its function.
Pass `name="..."` to choose another name, which must also end in `_sensor`.

| `when` | The function is called once for |
| --- | --- |
| `run_event(job="...", kinds=[...])` | Each run of the job or group that ends with one of `kinds`, optionally only when `executed_successfully` nodes ran and succeeded |
| `node_event(select="...", kinds=[...])` | Each success, failure or recovery of the selected nodes in any run |
| `check_event(select="...", kinds=[...])` | Each check, audit or SQL test on the selected nodes that fails or warns inside a job, job group or manual build run, whatever the run's outcome; standalone `sqb test` and `sqb audit` runs don't count |
| `source_new_data(select="...")` | Each new version of the selected sources |
| `source_stale(select="...", after_seconds=...)` | Each source version older than `after_seconds` |
| `platform_event(kinds=[...])` | Each server event of `kinds`, such as an overdue tick or a failed run |
| Not set | Every `minimum_interval_seconds`: a polling sensor |

`kinds` take the enums `RunEventKind`, `NodeEventKind`, `CheckEventKind` and `PlatformEventKind`
from `sqlbuild.orchestration`. `RunEventKind` has `SUCCESS`, `WARN`, `PARTIAL`, `FAILED`,
`CANCELLED`, `SKIPPED` (a member its group did not run) and `COMPLETED` for any of them.
`CheckEventKind` has `FAILED` and `WARN`. `check_event` covers checks inside job runs and manual
builds, such as one started from the Plan page, where the event's `job` is `None`. It doesn't
cover standalone `sqb test` or `sqb audit` runs, or `sqb scenario test`.

### What the function gets and returns

`ctx.event` holds the event, typed by the condition:

| Condition | `ctx.event` | Fields |
| --- | --- | --- |
| `run_event` | `RunEvent` | `id`, `occurred_at`, `job`, `run_id`, `status`, `error`, `failed_nodes`, `failed_checks`, `target`, `url` |
| `node_event` | `NodeEvent` | `id`, `occurred_at`, `run_id`, `job`, `node`, `event`, `target`, `url` |
| `check_event` | `CheckEvent` | `id`, `occurred_at`, `run_id`, `job`, `check`, `status`, `target`, `url` |
| `source_new_data` | `SourceNewDataEvent` | `id`, `occurred_at`, `source`, `data_version`, `target` |
| `source_stale` | `SourceStaleEvent` | `id`, `occurred_at`, `source`, `data_version`, `after_seconds`, `target` |
| `platform_event` | `PlatformEvent` | `id`, `occurred_at`, `event`, `details` |
| Not set | `None` | None; keep your place between polls in `ctx.cursor` |

`ctx.cursor` is for polling sensors only. A sensor with a `when` condition always sees `None`,
and a value it assigns is ignored: the server keeps its place in the events.

`url` links to the run in the UI. It is `None` when the scheduler's configuration sets no sign-in
origin, such as `[server] password_origin`.

| The function returns | The tick shows |
| --- | --- |
| `None` | Handled |
| `SkipReason("...")` | Skipped, with the reason |
| `RunRequest(job="...")`, or a list of them | Started, with links to the runs |
| Nothing: it raises or runs past `timeout_seconds` | Failed, with the traceback |

A `RunRequest` starts its job once per `run_key`; different jobs can share a key. For an event
the key defaults to the event's `id` and the job, so a repeated delivery starts nothing new. A
polling sensor must pass `run_key`:

```python
# orchestration/sensors/orders/partner_files_sensor.py
from sqlbuild.orchestration import RunRequest, SensorContext, SkipReason, sensor

@sensor(description="Load each new partner order file once", minimum_interval_seconds=300)
def partner_files_sensor(ctx: SensorContext[None], partner_files):
    files = partner_files.list_after(ctx.cursor)
    if not files:
        return SkipReason("No new files")
    ctx.cursor = files[-1].name
    return [RunRequest(job="partner_orders_job", run_key=file.name) for file in files]
```

| Setting | Default | Meaning |
| --- | --- | --- |
| `minimum_interval_seconds` | 30 | How often a polling sensor is called, and how long a failed event waits before its retry |
| `timeout_seconds` | 60 | How long one call may take, at most 600 |
| `default_status` | `running` | `paused` publishes the sensor switched off |
| `targets` | Every target | The targets the sensor runs for |

A location calls one sensor function at a time, and while it runs, the location's other work
waits: its other sensors, plans and sync. Keep functions short; a call that needs longer belongs
in a job the sensor starts.

### When a tick fails

SQLBuild delivers each event at least once. A failed tick leaves its event undelivered: the event
is retried after `minimum_interval_seconds`, and later events wait behind it. A function that sends
messages can use `ctx.event.id` to skip an event it has already handled.

Each tick on the sensor's page shows its event and outcome. People with `view.evidence` also see a
failed tick's traceback. From the first failed tick, the Overview lists "Sensor `<name>` is
failing" with the stuck event and since when. The item goes away when a tick succeeds. A
location that doesn't answer within `timeout_seconds` counts as a failed tick too; a tick cut short
by a new publication, a pause or a reset doesn't.

To move past an event the function can't handle, use **Skip this event** on the stuck tick, or
`sqb admin change-trigger <TRIGGER_ID> skip-event --event <event id> --event-id <uuid> --reason
"..."`. It marks that one event delivered without calling the function, and the tick history
and audit log record who skipped it and why. It works while a tick for the event is still out,
as it usually is for a sensor that times out or is paused; that tick's late answer starts nothing.
**Start over** (`reset-cursor`) instead re-delivers
every retained event from the start, the skipped one too. Both need `sensor.reset_cursor`.

### Try a sensor

[`sqb sensor try`](../cli/sensor.md) calls a sensor's function from your local code with a real
event. It starts no runs and records no tick:

```bash
sqb sensor try orders_failed_sensor --latest --server https://control.example.com --location-id <uuid>
```

Details:

- The function runs in the code location with the target's providers, so `sqb location serve` or
  `sqb dev` must be running for sensors to tick.
- A `run_event` on a job also fires for that job's runs inside a job group, as soon as the member
  finishes, and `executed_successfully` checks that member's nodes. A sensor on the group fires
  once, when the whole group run ends.
- A literal `RunRequest(job="...")` is checked when the project compiles. A job name built at run
  time is checked when the function returns, and an unknown job fails the tick.
- Pausing a sensor or resetting its cursor needs the permission on every node of the target.

## Rename models in jobs

`sqb rename` and `sqb mv` update job and job group selectors with the models they rename or move:

```bash
sqb rename stg_orders stg_orders_clean
```

```text
Rename model  stg_orders -> stg_orders_clean
├── sql/models/marts/fact_orders.sql
│   └── 10:13   reference __ref("stg_orders")  ->  __ref("stg_orders_clean")
├── sql/models/staging/stg_orders_clean.sql  (moved from sql/models/staging/stg_orders.sql)
│   └── 1:8     migration + migrate_from stg_orders
└── orchestration/jobs/orders_refresh_job.toml
    └── 3:10    selector  "+stg_orders+ fact_orders"  ->  "+stg_orders_clean+ fact_orders"
```

- Exact names are rewritten, including inside graph operators such as `+stg_orders+`, `,` and `~`.
  Tags, globs and `job:` selectors stay as written.
- A folder selector, such as `sql/models/staging` or `path:sql/models/staging/`, moves when every model
  under it moves to the same folder. When only some leave, the command lists a note at the
  selector to check by hand.
- A selector in a multi-line TOML string is reported for a manual edit and blocks the refactor.

## Errors

Every problem with an orchestration file is a `D017` error at its file, line and column, reported by
`sqb compile` and every other command that reads the project. `sqb compile` resolves every job's
`select` and `exclude`, and every sensor condition's selectors, for the target it compiles, as a
location does when it publishes, so a selector the location would refuse fails the compile first.

| Error | Fix |
| --- | --- |
| `orchestration/jobs/orders_job.toml:3:10: job orders_job: path selectors require an explicit root: use 'sql/models/' or 'python/nodes/'` | Start the path at `sql/models/` or `python/nodes/` |
| `orchestration/sensors/raw_sensor.py:4: sensor raw_sensor: source_new_data: path selectors require an explicit root: use 'sql/models/' or 'python/nodes/'` | The same, in the sensor's condition |
| ``orchestration/jobs/nightly_orders_job.toml:4:11: unknown variant `coalesce`, expected `queue` or `skip` `` | Use one of the listed values |
| `files cannot sit directly in orchestration/` | Move jobs and groups to TOML files in `orchestration/jobs/` or `orchestration/job_groups/` |
| `` `job` was removed from sqlbuild.orchestration`` | Declare the job in TOML and start it with `RunRequest(job="...")` |
| `sensor(...) must decorate a function` | Write `@sensor(description=..., when=...)` above the function's `def` |
| `sensor takes keyword arguments only` | Remove the positional name: the sensor is named after its function, or pass `name="..."` |
| `sensor 'orders_sensor' names 'orders_missing_job', which is not a declared job or job group` | Fix the job name in `RunRequest(job=...)` |
| `declares a second sensor; each module declares exactly one` | Split the module |
| `schedule cron: invalid cron expression: Pattern must have 5 fields when seconds are disallowed.` | Write a five-field cron, such as `0 2 * * *` |
| `job nightly_orders_job resources: cpus must be a positive number` | Set `cpus` above zero |
| `job 'nightly_orders_job' uses pool 'warehouse_hevy', which [pools] in sqlbuild_project.toml does not declare` | Fix the name, or declare the pool in [`[pools]`](project-configuration.md#pools) |
| `job 'nightly_orders_job' takes 2 slots of pool 'supplier_api', more than its limit of 1 on target 'production'` | Take fewer slots, or raise the limit in `[targets.production.pools]` |
| `jobs orders_job and crm_job set vars differently ({"amount":"5"} and {"amount":"9"}); a build applies vars to everything it runs; group daily_job_group builds its jobs together` | Give the group's jobs the same `vars` and `changed`, or move one job to another group |
| `sensor 'orders_sensor' is declared here, but importing the module does not define it` | Declare the sensor at module level, not inside a function or an `if` |
| `group dependencies contain a cycle in job group daily_orders_job_group: orders_marts_job waits for staging_orders_job (model:orders reads model:stg_orders); staging_orders_job waits for orders_marts_job (model:stg_orders reads seed:regions)` | Move each node to one job, and give seeds and nodes that several members read their own job that runs first |

Details:

- `sqb build` and the other commands read job and group files without running project Python, so
  a job selector never imports a calendar or sensor module.
- A job or group declared with `targets` is selectable only when the command's target is one of
  them, as on the server.
- Job settings that cannot apply together fail the build or plan with `S027`, such as `model:orders
  is selected through jobs orders_job and finance_job, which set run_tests differently (false and
  true)`. The help names the way out: make the jobs agree, set a node setting in only one of
  them, or use `--no-job-settings`. It offers a flag only for the value the flag gives, such as
  `--no-tests` to turn tests off for all; no flag turns them back on.
- Selectors are checked for syntax at compile. A selector that names a node the project does not
  have fails when a location publishes, such as `job nightly_orders_job: unknown selector name
  'order_total'`.
