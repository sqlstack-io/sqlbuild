<!-- generated-by: sqlbuild skills -->

# sqb server

> Run the control service and scheduler.

Online: https://sqlbuild.com/docs/cli/server/

## Contents

- Set retention
- Inspect published SQL
- Define project roles

Run the control API and scheduler without loading project Python:

```bash
sqb server
```

Use `sqb server --help` for service, authentication and launcher options.

## Set retention

Set how much history the server keeps:

```bash
sqb server --statistics-retention-days 30 --audit-retention-days 730 --snapshot-retention-count 20
```

| Data | Option | Default | Range |
| --- | --- | --- | --- |
| Hourly warehouse statistics | `--statistics-retention-days` | `90` | `1`–`65535` |
| Security audit records | `--audit-retention-days` | `400` | `1`–`65535` |
| Published snapshots per location and target | `--snapshot-retention-count` | `10` | `1`–`65535` |
| Skipped and covered schedule ticks | None | 30 days | Fixed |

Statistics samples older than the horizon disappear from statistics reads at once and are then
deleted in the background.

A security audit record older than the horizon is deleted only when it is no longer needed:

- The webhook in `SQLBUILD_SECURITY_AUDIT_WEBHOOK_URL`, if set, has received it.
- No retained run's retry depends on it. A manual job or rerun request keeps its record while
  the run exists.
- It is not the newest record, so audit page cursors never see a reused position.

Records kept past the horizon for these reasons are reported on stderr as
`Security audit retention: retained past horizon: N` whenever the count changes. A growing count
usually means the audit webhook is failing.

The server keeps the newest published snapshots for each location and target. It deletes an older
snapshot only when no retained run, job, trigger, trigger change or location request uses it, and
deletes its node definitions and SQL text when no other snapshot shares them. Repeating or
interrupting this cleanup is safe.

The scheduler deletes covered ticks older than 30 days. It keeps each trigger's latest tick and
every tick that started a run. A new schedule, a resumed trigger or a cursor reset catches up only
on the latest missed time, so deleted ticks never come back.

An invalid value stops the server before it opens the control database. Servers that share one
control database delete at the shortest horizon among them and count only their own audit
webhook, so give them the same values and webhook.

## Inspect published SQL

Sign in with [`sqb login`](login.md), then inspect one exact node in the current
successful publication:

```bash
sqb control sql model:orders \
  --server https://control.example.com \
  --location-id "$LOCATION_ID" \
  --target development
```

The command uses your saved person session and prints JSON. It does not load a local project
or open a warehouse connection. Use `--ca-cert PATH` for an additional trusted PEM certificate.
The node key is exact: `model:orders+` does not expand a selector. Progress and failures go to
stderr; a failed request prints no SQL to stdout. Permission denials include their decision ID.

The equivalent person-authenticated API is a `GET` request to
`/api/control/locations/{location_id}/targets/{target}/nodes/{node_key}/sql`.

Developer or Admin access must cover the requested location and target, or a
[project role](#define-project-roles) must grant `view.sql` on the node. Viewer, Operator and
the configurable default Viewer role do not grant SQL inspection. The response contains the
snapshot ID, exact node key, raw SQL and compiled SQL. Missing SQL fields are `null`; an absent
current publication or node returns JSON `null` after authorization.

SQL inspection does not execute SQL or return Python source, execution artifacts, logs or sample
data. Every successful inspection records an audit entry without copying SQL into it. If the
audit cannot be saved, the request fails without returning SQL. Revocation takes effect on the
next request, including sessions created by [`sqb login`](login.md).

## Define project roles

Put project roles in `roles.toml` at the project root. `sqb location publish` resolves each
grant's node selectors against the published project:

```toml
[roles.orders_support]
description = "Answer order questions"

[[roles.orders_support.grants]]
nodes = ["tag:orders"]
permissions = ["view.sql", "run.adhoc"]

[[roles.orders_support.grants]]
nodes = ["tag:orders,tag:pii"]
permissions = ["view.evidence"]
```

| Rule | Behaviour |
| --- | --- |
| Selectors | Same grammar as a job's `select`; an unknown tag or name fails publication |
| No matching nodes | Publication succeeds with a warning; the grant gives nothing |
| Permissions | Any non-administrative action; `user.manage`, `role.assign` and `audit.view` are rejected |
| Runs | `job.run`, `run.adhoc`, `run.rerun` and `run.rewrite_history` need the permission on every node the run writes |
| Triggers | `trigger.toggle` and `sensor.reset_cursor` need the permission on every node the trigger's job writes |
| Listings | The catalog, statistics and run list need `view` on every node of the current publication; partial coverage is denied, not filtered |
| Several roles | Grants add up across a person's roles |

Assign a project role with `POST /api/admin/roles`, using the role key as `role`. Each
publication replaces the location's project roles: new and changed grants apply at once, and
removed roles stop applying. A selector covers newly matching nodes as soon as they are
published. Each change appears in the publication's change list as `role:<key>`. A denial lists
the nodes the request was missing.
