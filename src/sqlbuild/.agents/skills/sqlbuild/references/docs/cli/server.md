<!-- generated-by: sqlbuild skills -->

# sqb server

> Run the control service or repair its derived views.

Online: https://sqlbuild.com/docs/cli/server/

## Contents

- Set retention
- Inspect published SQL
- Repair control views
- Handle a refused repair

Run the control API and scheduler without loading project Python:

```bash
sqb server
```

Use `sqb server --help` for service, authentication and launcher options.

## Set retention

Set how long the server keeps hourly warehouse statistics and security audit records:

```bash
sqb server --statistics-retention-days 30 --audit-retention-days 730
```

| Data | Option | Default | Range |
| --- | --- | --- | --- |
| Hourly warehouse statistics | `--statistics-retention-days` | `90` | `1`–`65535` |
| Security audit records | `--audit-retention-days` | `400` | `1`–`65535` |

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

Developer or Admin access must cover the requested location and target. Viewer, Operator and
the configurable default Viewer role do not grant SQL inspection. The response contains the
snapshot ID, exact node key, raw SQL and compiled SQL. Missing SQL fields are `null`; an absent
current publication or node returns JSON `null` after authorization.

SQL inspection does not execute SQL or return Python source, execution artifacts, logs or sample
data. Every successful inspection records an audit entry without copying SQL into it. If the
audit cannot be saved, the request fails without returning SQL. Revocation takes effect on the
next request, including sessions created by [`sqb login`](login.md).

## Repair control views

Repair snapshot selection, jobs, triggers, runs, node status and the structural catalog from
retained control facts:

```bash
sqb server rebuild-projections
```

This operation requires direct ownership of the initialized control database. A browser role
or location credential does not grant it. Back up the database before repair. This command
does not initialize or upgrade a database; start the matching server version first if a schema
upgrade is needed.

| Database | Selection |
| --- | --- |
| SQLite | `.sqlbuild/control.sqlite3` in the project directory when the database variable is unset |
| PostgreSQL | URL in `SQLBUILD_CONTROL_DATABASE_URL` |

Use `--database-env VARIABLE` to read the database URL from a different environment variable.
Use `--project-dir PATH` to select a different SQLite project directory.

Repair waits for the current writer and uses one consistent source cut. It commits all repaired
views together or leaves every view unchanged. Repeating a successful repair has the same result.
It preserves credentials, security audits, retained source facts, worker claims, leases and other
operational ownership. It does not rerun jobs, release workers or modify the warehouse.

## Handle a refused repair

Repair fails if a retained source is unsupported, invalid or insufficient to reconstruct the
views exactly. A trigger without a recorded semantic-progress image cannot be repaired: tick
history alone cannot distinguish a cursor reset from a subsequent reevaluation of existing ticks.
The command never guesses the cursor, substitutes the current time or reports partial success.

If repair reports missing semantic progress, preserve the database and use a known-good backup.
Do not delete source history to force a repair. Recording future transitions does not recover
missing historical facts.

| Result | Exit status | stdout |
| --- | --- | --- |
| All views repaired | `0` | `{"rebuilt":true}` |
| Repair refused or rolled back | Nonzero | No success result |

Progress and completion messages go to stderr. If stdout delivery fails after commit, the
command reports that changes may already be committed; rerunning the repair is safe.
