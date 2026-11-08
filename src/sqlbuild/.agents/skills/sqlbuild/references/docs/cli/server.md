<!-- generated-by: sqlbuild skills -->

# sqb server

> Run the control service and scheduler.

Online: https://sqlbuild.com/docs/cli/server/

## Contents

- Set retention
- Grant roles to OIDC groups (beta)
- Inspect published SQL
- Run ad-hoc commands
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

## Grant roles to OIDC groups (beta)

With OIDC sign-in, members of an identity-provider group can get a role without a per-person
assignment. Add `groups_claim` and `group_roles` to the JSON file passed to `--oidc-config`:

```json
{
  "origin": "https://control.example.com",
  "issuer": "https://login.example.com",
  "client_id": "orders-control",
  "identities": {"orders-person": "0190c3c1-5a4e-7b8e-9f00-2a7d1c9e4b11"},
  "groups_claim": "groups",
  "group_roles": [
    {"group": "orders-operators", "role": "operator", "location_id": "0190c3c1-6b2f-7c3d-8e44-5f1a2b3c4d5e", "target": null},
    {"group": "orders-readers", "role": "viewer", "location_id": null, "target": null}
  ]
}
```

| Key | Meaning |
| --- | --- |
| `groups_claim` | ID-token claim that lists the person's groups. Required when `group_roles` is set. |
| `group_roles[].group` | Group name exactly as the claim lists it. |
| `group_roles[].role` | `viewer`, `operator`, `developer` or a project role the location publishes. Not `admin`. |
| `group_roles[].location_id` | Limits the role to one location. Required for a project role or a target. |
| `group_roles[].target` | Limits the role to one target in that location. |

Groups grant roles only to people who already have an entry in `identities`; a group never
creates an account. Group roles add to the person's own role assignments.

The server reads groups only from the signed ID token, at sign-in. Each sign-in replaces the
person's groups with the ones in that token:

- Removing someone from a group in the identity provider stops the grant at their next sign-in.
  Until then, all of their sessions use the groups from their most recent sign-in.
- Removing or changing a `group_roles` entry and restarting the server changes access at once,
  without a new sign-in.
- A claim that is missing, is not a list of non-empty names, or is replaced by an overage pointer
  gives no groups.

The server refuses to start when a mapping grants `admin`, scopes a project role or target without
a location, has a blank group, or when `group_roles` is set without `groups_claim`.

Servers that share one control database must use identical OIDC and `group_roles` configuration.
Different configurations on one database are unsupported: the last server to start decides the
group mapping for all of them.

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

## Run ad-hoc commands

People with `run.adhoc` can run a `sqb` command on the current publication without declaring a
job. Use the web UI's Run panel, or send the command as argument tokens after `sqb`:

```json
{
  "location_id": "0191f0aa-0000-7000-8000-000000000001",
  "target": "development",
  "command": ["build", "--select", "+orders", "--full-refresh"],
  "priority": 0
}
```

`POST /api/control/runs/adhoc/check` with this body is a dry run. It returns the exact nodes the
command selects (`selection`), the nodes it writes (`write_set`), the options the worker receives
(`settings`), and each permission it needs with your decision (`permissions`). `error` explains a
command that can't run whatever your permissions, and `launchable` is `true` when the launch
would be accepted. A dry run queues nothing and records no audit entry.

To launch, `POST /api/control/runs/adhoc` with the same fields plus a new `run_id` and a
`reason`. The response is `{"applied": true}`; sending the same `run_id` again returns
`{"applied": false}` and starts nothing. The server resolves selectors against the current
publication, and the worker runs exactly the resolved nodes. The target is always the one in the
request.

| Verb | Ad-hoc runs |
| --- | --- |
| `build` | Dry run and launch |
| `test`, `seed`, `audit`, `freshness` | Dry run only; workers can't run them yet |
| Any other verb | Refused |

Selectors use the CLI grammar: names, `seed:`, `source:`, `tag:`, `path:` and `test:`, with `+`
for upstream or downstream nodes. Positional selectors work as on the command line.

| Flags | Verbs | Needs |
| --- | --- | --- |
| `--select`, `-s`, `--exclude` | All | `run.adhoc` on every selected node |
| `--full-refresh`, `--start-cursor`, `--end-cursor`, `--start-cursor-ts`, `--end-cursor-ts`, `--start-cursor-int`, `--end-cursor-int` | `build` | `run.rewrite_history` |
| `--allow-snapshot-full-refresh`, `--allow-snapshot-schema-change`, `--allow-table-type-downgrade`, `--allow-retention-decrease` | `build` | `run.rewrite_history` |
| `--warehouse` | All | `run.warehouse`, and the warehouse configured for the target on the location |
| `--max-microbatches`, `--no-load`, `--no-tests`, `--no-audits`, `--no-python`, `--fail-fast`, `--selection-diagnostics`, `--verbose` | `build` | `run.adhoc` |
| `--concurrency` | `build`, `test`, `seed`, `audit` | `run.adhoc` |
| `--test-timeout` | `build`, `test` | `run.adhoc` |
| `--case`, `--inspect` | `test` | `run.adhoc` |
| `--state`, `--fail-on-error`, `--fail-on-stale` | `freshness` | `run.adhoc` |

A priority above 0 also needs `run.prioritise`. Among built-in roles only Admin grants
`run.rewrite_history`, `run.warehouse` and `run.prioritise`. Every other flag is refused with
its reason:

| Refused flags | Reason |
| --- | --- |
| `--vars` | Variables change the published SQL |
| `--defer-to`, `--defer-sources-to`, `--defer-clone-from` | They read another target |
| `--load`, `--reload` | Source loads are not part of ad-hoc runs |
| `--allow-missing-migration-origin` | It can lose data |
| `--no-sql-analysis`, `--no-cache` | Workers verify the published SQL and manage their own cache |
| `--debug` | It prints executed SQL |
| `--since`, `--explain` | They need the worker's Git checkout |
| `--select-file` | It reads a file on the worker |
| `--target`, `--project-dir` and the `--dbt-*` paths | Workers run the published project for the request's target |
| `--json`, `--json-output`, `--event-output`, `--manifest`, `--no-color` | Workers own the run's output |

The worker checks `--warehouse` against the warehouses the location configures for the target
before any statement runs. An unconfigured warehouse fails the run with the reason
`warehouse WH_L is not configured for target development on this location; no warehouse
statement ran`, which the run page shows.

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
| Runs | `job.run`, `run.rerun` and `run.rewrite_history` need the permission on every node the run writes; an [ad-hoc command](#run-ad-hoc-commands) needs `run.adhoc` and its flags' permissions on every node it selects; a priority above 0 also needs `run.prioritise` |
| Triggers | `trigger.toggle` and `sensor.reset_cursor` need the permission on every node the trigger's job writes |
| Listings | The catalog, statistics and run list need `view` on every node of the current publication; partial coverage is denied, not filtered |
| Several roles | Grants add up across a person's roles |

Assign a project role with `POST /api/admin/roles`, using the role key as `role`. Each
publication replaces the location's project roles: new and changed grants apply at once, and
removed roles stop applying. A selector covers newly matching nodes as soon as they are
published. Each change appears in the publication's change list as `role:<key>`. A denial names
only the missing permission, such as `missing view permission for this target`, and its decision
ID. It names no nodes, and an unknown location, job, trigger or run is refused with the same
response. The security audit event for that decision ID lists the nodes the request was missing.
