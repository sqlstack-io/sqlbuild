<!-- generated-by: sqlbuild skills -->

# sqb server

> Run the control service or repair its derived views.

Online: https://sqlbuild.com/docs/cli/server/

Run the control API and scheduler without loading project Python:

```bash
sqb server
```

Use `sqb server --help` for service, authentication and launcher options.

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
