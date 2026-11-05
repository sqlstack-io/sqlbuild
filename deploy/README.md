# Run the reference Docker Compose stack

This Linux reference runs PostgreSQL, the native SQLBuild server/scheduler, and one outbound
Python code location. Workers use Docker only. The server image contains no Python or project.
The location image includes the example orders project and its Python dependencies.

Use Docker Engine with a Unix socket and Docker Compose 2.23.1 or newer, Python 3.12, `uv`,
a Rust toolchain, and `jq`. Build and test from a trusted checkout. Docker socket access is
host-administrator authority: only the server gets that socket and control-store credentials.
Locations and workers get warehouse access, but no control-store credentials or inbound ports.
The default server port binds to loopback, not the public network.

## Build the images

Build matching wheels from this checkout before building the images:

```sh
uv sync --locked --all-extras
uv run --no-sync maturin build --release --compatibility linux --out dist
uv run --no-sync maturin build --manifest-path crates/sqlbuild-cli/Cargo.toml --release --compatibility linux --out dist
cp deploy/compose.env.example .env
```

Set the ignored `.env` values. Use a unique PostgreSQL password and put its URL-encoded value
in `SQLBUILD_CONTROL_DATABASE_URL`, for example
`postgresql://sqlbuild:<encoded-password>@postgres:5432/sqlbuild`.
Set `DOCKER_GID` to `stat -c %g /var/run/docker.sock` and `SQLBUILD_GIT_REVISION` to
`git rev-parse HEAD`. Keep only the matching SQLBuild and CLI wheels in `dist/`; their Python
version and architecture must match the image's `PYTHON_VERSION` build argument and Docker host.
The default image uses Python 3.12. Do not put credentials in build arguments or project files.
The Docker build context includes only wheels and the example project.

```sh
docker compose build
docker compose up -d --wait postgres
```

## Register and start the code location

Run these commands on the trusted control-store owner's host. They use the server image's
direct-store administration path, not an HTTP or person-role grant:

```sh
docker compose run --rm --no-deps server admin create-location orders
```

Replace `.env`'s placeholder `SQLBUILD_LOCATION_ID` with the returned `location_id`. Issue a
bounded credential, using a future RFC 3339 expiry you choose:

```sh
docker compose run --rm --no-deps server admin issue-location-token "$SQLBUILD_LOCATION_ID" --expires-at "$EXPIRY"
```

Shell variables above must be set explicitly; Compose's `.env` file is not sourced by the shell.
Store the one-time `bearer` in `.env` as `SQLBUILD_LOCATION_TOKEN`, restrict the file to its owner
(`chmod 600 .env`), and start the stack:

```sh
docker compose up -d --wait server location
docker compose logs -f server location
```

The location watches and compiles its baked project, publishes a successful snapshot, and checks
its warehouse connection. The example schedule runs every minute. The server resolves the
running container's actual Docker image ID at publication; no `--image-id` or Docker socket is
needed in the location. Workers recompile in that pinned image and verify environment and node
hashes before writing. The warehouse persists in its named volume.

For your project, build the `runtime` target as a base, then extend it with `COPY --chown=10001:10001`
of your project to `/project`, install its locked dependencies, and set `SQLBUILD_GIT_REVISION`.
Set `SQLBUILD_LOCATION_IMAGE` to that project image. Give location UID 10001 access to its
warehouse volumes. Never mount control credentials, location-token files, or Docker sockets
into the location: workers inherit its mounts and filtered warehouse environment.

## Redeploy without interrupting a run

Build the new project into a new revision-tagged image. Set `SQLBUILD_LOCATION_IMAGE` to that
image and update only the location:

```sh
docker compose up -d --no-deps location
```

Active workers keep their existing image, process, mounts, and independent network endpoint.
New publications affect only new runs. A failed compile does not replace the last good snapshot;
its scheduled jobs still launch from the last good image even while the new location is broken.
Keep the warehouse mounts, network, working directory, and warehouse environment compatible with
that image. Changing those deployment settings requires a coordinated maintenance window.

Retain every image referenced by a published snapshot or run. Do not run indiscriminate image
pruning or delete those images. Inspect `sqb_snapshots.image_id` and `sqb_runs.image_id` through
the trusted control-store owner before implementing retention. Removing a required image makes
a queued or historical run unlaunchable.

For bind-mounted source development, explicitly add the development overlay:

```sh
docker compose -f compose.yml -f deploy/compose.dev.yml up -d
```

The location watcher republishes changed source. The overlay keeps `target` and `logs` in
named volumes owned by the image's runtime user, so location and worker output does not require
changing the host source tree's ownership. Use the same overlay for `down --volumes` when removing
these development caches and logs. Workers reject source drift against a pinned snapshot; the
bind mount is not an immutable deployment and does not provide old-source fallback.

## Credentials, identity, and platform events

Rotate a location credential with `docker compose run --rm --no-deps server admin rotate-token
TOKEN_UUID`, store its one-time replacement bearer, and recreate the location. The scope and
expiry stay unchanged, and the old credential is revoked atomically. Service-account principal
tokens are separate from location/run tokens. Person identities do not become machine bearers.

For OIDC sign-in, mount a server-owned JSON file with `origin`, `issuer`,
`client_id`, and explicit `identities` subject-to-existing-person UUID bindings; append
`--oidc-config /etc/sqlbuild/oidc.json` to the server command. Both origin and issuer require
HTTPS. Terminate HTTPS at a trusted proxy, preserve the external Host, restrict backend access,
and register `/api/auth/oidc/callback` at the issuer. The callback verifies an explicitly bound,
enabled person and issues a one-hour browser session. With a trusted proxy, every request
authenticates from the configured socket peer and the upstream-replaced `X-SQB-User` subject; no
browser session is issued, and mutations require the configured origin. Neither mechanism
provisions people or grants roles. A session bearer stays in a Secure, HttpOnly, host-only
cookie; mutations require the returned CSRF token and matching origin. Sessions work across replicas sharing the installation, with live
person and role checks. `POST /api/auth/logout` revokes the session across replicas; password
resets and `grant-admin` revoke all of that person's browser sessions.

The local control-store owner initializes the first installation administrator with
`sqb admin create-user NAME`, then provisions its password separately with
`sqb admin set-password PRINCIPAL_ID`. `create-user` works only while no person exists; once
anyone exists, use `grant-admin` instead. `create-principal` creates a role-free identity, not
another administrator. With password, OIDC or trusted-proxy authentication, installation administrators
use `POST /api/admin/people/{principal_id}` with a JSON action of `grant`, `revoke`, `disable`,
or `enable`, their session cookie and CSRF header. This cannot remove or disable the final active
administrator. These endpoints never authorize a machine bearer or grant non-admin defaults.

If administrator access is lost, run `sqb admin grant-admin PRINCIPAL_ID` as the installation's local
control-store owner; servers may keep running. It enables an existing person, grants installation
admin membership, revokes that person's browser sessions and records an audit event. It never
creates a person. Provision a password with `set-password` if needed.

An installation administrator assigns or revokes additive built-in roles through
`POST /api/admin/roles`, using `principal_id`, `role`, optional `location_id` and `target`,
and `revoke` (false for a grant). An administrator assignment must be installation-wide.
Other built-ins can be installation-wide or restricted to a location or target. Revocation
also works while a person is disabled, before they are enabled again.

Browser sessions use `GET /api/control/locations/{location_id}/targets/{target}/runs`
for recent run metadata, `POST /api/control/runs` for published-job requests, and
`POST /api/control/triggers` for trigger overrides. Use `POST /api/control/runs/cancel`
with `run_id`, `event_id` and `reason` to request cancellation; use
`POST /api/control/runs/rerun` with `run_id`, `source_run_id`, `from_failure`, `reason`
and `priority` to repeat a terminal run. Retrying an identical request is idempotent, but
still requires live permission. Reruns keep the original snapshot, image, selection and bounds;
`from_failure` narrows selection to durable failed/skipped outcomes. Cancellation of active
work remains a request until its worker confirms shutdown; it never releases ownership early.
Viewers can read metadata but cannot run, cancel or rerun jobs; operators and developers can
control ordinary jobs and toggle triggers. Ad-hoc reruns also require developer or admin access.
Full-refresh jobs, published cursor bounds, explicit backfill windows and their reruns require
an installation administrator in these endpoints. Sensitive work can still be cancelled without
history-rewrite permission. The server defaults to `--default-role none`: a person with no
assigned role cannot use them. Set `--default-role viewer` to give every authenticated person
installation-wide metadata access, including people whose explicit roles have been revoked.
This is a live server setting, not a persisted assignment; returning it to `none` removes that
ambient access without changing explicit grants. It never permits SQL, evidence, job execution
or administration. Metadata responses do not contain SQL, sampled rows or captured output.
These person routes do not accept machine
credentials; location and worker credentials continue to use the separate machine routes.

Set `SQLBUILD_PLATFORM_WEBHOOK_URL` in `.env` before creating the server to receive the native
platform stream. Leave it unset to disable delivery. Receivers must deduplicate by `event_id`
or the matching `Idempotency-Key` header: delivery is at least once and retries after failure.
The native watchdog runs independently of location Python and scheduler leadership. It observes
heartbeats, overdue ticks, missing leadership, and locations silent for over 90 seconds; its
30-second scan and delivery polling add detection latency. Each alert is sent once when its
condition starts, not on every scan. Monitor platform heartbeats from
outside this stack too: a stopped server cannot report its own outage.

## Back up and upgrade

Back up PostgreSQL with `pg_dump` using your normal protected database backup tooling. Also
back up warehouse data, the restricted `.env`/identity configuration, and referenced project
images. Keep backup credentials and output outside the repository. Test restoration before an
upgrade. Ordinary `docker compose down` preserves named volumes; `down --volumes` destroys them.

Upgrade matching server/location wheels and images together, retaining referenced old project
images. The native server applies additive control-store migrations before its API reports
ready. Do not start an older server against a newer schema without a documented compatible
rollback or restored backup. Scheduler leadership resumes from durable ticks after restart;
catch-up runs each trigger's latest missed time once, not every missed time.
The Docker launcher adopts an existing claim-labeled worker rather than starting a duplicate.

## Verify the packaged lifecycle locally

`make test-compose` builds matching wheels and images, then tests scheduled Docker execution,
redeploy during an active run, failed-deploy fallback, scheduler catch-up, credential/mount
isolation, the development overlay's source reload and worker outputs, and the stopped-location
platform webhook against PostgreSQL. It also backs up the
populated control store, restores into an empty database, compares every public table's row count
and content digest before restarting writers, and checks that retained credentials and last-good
scheduling work after restore without duplicate ticks. This is a control-store restore check,
not an image, secret-file, or warehouse disaster-recovery test. It creates disposable containers
and volumes and removes them on exit. It needs Docker, Compose, and network access for image
and Python dependency downloads; it never deploys to an external installation.
