<!-- generated-by: sqlbuild skills -->

# sqb location

> Run a code location that publishes a project to the server, and check its health.

Online: https://sqlbuild.com/docs/cli/location/

## Contents

- Check a location's health
- Provide settings and secrets
- Republish

Run a code location: it compiles the project in its own Python environment, publishes it to the
server, and publishes again when the project changes:

```bash
export SQLBUILD_LOCATION_TOKEN=...
sqb location serve --server https://control.example.com --location-id "$LOCATION_ID" --target production
```

With a separate `sqb server`, keep a SQLite control database and a local
[output folder](server.md#keep-run-output) outside the project or under the project's
`.sqlbuild` folder. Otherwise the location takes their writes for project edits and publishes again.

Create the location and its token with `sqb admin create-location orders` and
`sqb admin issue-location-token` on the server's host. The token is the only thing on stdout, so
`export SQLBUILD_LOCATION_TOKEN="$(sqb admin issue-location-token "$LOCATION_ID" --expires-at "$EXPIRY")"`
captures it. The location reads it from `SQLBUILD_LOCATION_TOKEN` and never opens the control
database.

## Check a location's health

Ask the server how a location and target are doing, for example from a container health check:

```bash
sqb location health --server https://control.example.com --location-id "$LOCATION_ID" --target production
```

```json
{"location_id":"01a1d7ff-0b3a-7631-ba35-e44d44d072f2","target":"production","state":"healthy"}
```

| `state` | Meaning | Exit code |
| --- | --- | --- |
| `starting` | The newest instance checked in but hasn't finished its first load | `3` |
| `healthy` | An instance checked in within 90 seconds and its last load published | `0` |
| `degraded` | The warehouse connection check after the last load failed | `1` |
| `missing_secrets` | A provider [secret](../concepts/python-nodes/providers.md#secrets) is unset in the location's environment, and the target's [`allow_missing_secrets`](../concepts/project-configuration.md#allowed-missing-secrets) doesn't list the provider, so runs that use it fail | `1` |
| `change_plan_failed` | The location's plan of what changed on the current snapshot failed, so the UI's **Build what changed** and the nodes' changed state are unknown | `1` |
| `discovery_failed` | A [discover function](../concepts/python-nodes/discovered-nodes.md) failed at the last publication, so its set serves its last good items, or none | `1` |
| `falling_back` | The last load failed, so runs use the previous snapshot | `1` |
| `broken` | No instance has checked in for 90 seconds | `1` |

Only `healthy` exits `0`. The command exits `2` when it cannot read the state: the server is unreachable, or the token is
missing, expired or for another location. It needs the location's token, not a person's sign-in.

The state is the newest instance's: the one that started last among those that checked in within
90 seconds. Until its first load finishes it is `starting`, whatever an earlier instance loaded, so a
deploy that restarts the location and polls `sqb location health` until it exits `0` waits for the
new code. Once that load finishes, the state comes from it: `falling_back` if it failed, otherwise
`healthy` or one of the states above. For an instance that started more than 7 days ago, whose
first reports are no longer kept, the state comes from the location's newest reports of any instance.

A container health check asks whether the location is alive, not whether the newest code has
loaded: pass `--allow-starting`, which exits `0` while the state is `starting` and leaves every other
exit code as it is. Whether a deploy's new code loaded is the deploy's cutover check: poll
`sqb location health` without the flag until it exits `0`. A Compose health check for the location
service, as the shipped `compose.yml` has:

```yaml
healthcheck:
  test: [CMD, sqb, location, health, --allow-starting, --server, "http://server:8000", --location-id, "${SQLBUILD_LOCATION_ID}", --target, development]
  interval: 30s
  start_period: 60s
```

A Quadlet unit uses the same command in `HealthCmd=`.

A `falling_back` location still runs scheduled jobs from its last good snapshot. Restarting it
does not fix the failed load; fix the project and deploy it again. Plan previews on it fail at once
with the load's error instead of waiting for the location.

A location that checks in but is `falling_back`, `change_plan_failed`, `discovery_failed` or
`degraded` is a problem everyone who can view its target sees: a banner on every page of the UI
names the location and target, the error, and the generation it serves instead with when that was
published; the Overview lists it under **Needs attention**; and the server sends a
[`location_failing` platform event](server.md#receive-platform-events) once per failure, so a
platform webhook or a [`platform_event` sensor](../concepts/orchestration.md) with `PlatformEventKind.LOCATION_FAILING` can alert on it.

Loading a project is a full compile, as [`sqb compile`](compile.md) runs it, including the
configured [Rules](../concepts/rules.md). A Rule finding fails the load like any other compile
error: nothing is published, and the location's output and its page in the UI show the findings:

```text
Compile failed: 1 rule finding
sql/models/orders.sql:2:8 [SQBRMODEL102] SELECT * is allowed only inside dependency import CTEs
```

Runs and plan previews on a published snapshot don't evaluate the Rules again; they passed when it
loaded. After a SQLBuild upgrade that adds Rules to a family you select, a location keeps running
its last good snapshot until the project passes them.

A `missing_secrets` location publishes and runs jobs whose nodes have their secrets. Each time it
publishes it reports the providers missing secrets, with their variables and the nodes that use
them, and prints them in its output. Its page in the UI lists them, and the Overview shows the
problem. A run that needs one fails those nodes with `D012` before connecting to the warehouse and
skips the nodes that read them. Set the missing variables in its env file or in the environment it
was started with, then restart it (under `sqb dev`, `r` is enough after editing the env file); `sqb debug` in the location lists them too. A provider the target allows to miss its
secrets shows as a note on the location's page instead.

## Provide settings and secrets

A location reads provider settings from its environment and from the project's
[env file](../concepts/project-configuration.md#env-file), which it loads once when it starts.
For a deployment, point the target at a file outside the project:

```toml
[targets.production]
env_file = "/etc/opt/sqlbuild/sqlbuild.env"
```

The location stops at start-up with an error naming the file when it doesn't exist. Variables
set in the container or service environment win over the file. After changing the file, restart
the location so its compiles and plan previews use the new values. Each
[worker](server.md) reads the file when its run starts.

## Republish

Ask a served location to compile and publish again, for example after a deploy of a system its
[discover functions](../concepts/python-nodes/discovered-nodes.md) read from. Every discover
function runs, as with `r` in `sqb dev`.

With an API key, from the environment and never an argument, the server may be `http://` or
`https://`, so a server on a private network needs no TLS:

```bash
export SQLBUILD_API_KEY=...
sqb location republish --server http://control.internal.example.com:8000 --location-id "$LOCATION_ID" \
  --target production --reason "fulfilment 0.58.0 deployed" --wait
```

A key sent over plain HTTP to another host prints one warning, because anyone on the network path
can read it. Without `SQLBUILD_API_KEY` the command uses your [`sqb login`](login.md) session,
which needs `https://`; `--ca-cert` trusts a private CA for it.

| Outcome | Exit code |
| --- | --- |
| Accepted, without `--wait`; or published | `0` |
| The publish failed, or no location took the request within 15 minutes | `1` |
| The request was refused or got no answer, or `--wait` ended after `--timeout` seconds (default 600) | `2` |

The JSON result has `requested_by`, `requested_at` and `state` (`pending`, `published`, `failed` or
`expired`), with `snapshot_id` after a publish or `error` after a failed one.

The same request with `curl`:

```bash
BASE="http://control.internal.example.com:8000/api/control/locations/$LOCATION_ID/targets/production"
REQUEST_ID=$(uuidgen)

curl --fail -X POST "$BASE/republish?wait=10" \
  -H "Authorization: Bearer $SQLBUILD_API_KEY" -H "Content-Type: application/json" \
  -d "{\"request_id\": \"$REQUEST_ID\", \"reason\": \"fulfilment 0.58.0 deployed\"}"

# While the state is pending, ask again
curl --fail "$BASE/republish/$REQUEST_ID?wait=10" -H "Authorization: Bearer $SQLBUILD_API_KEY"

# Each discovered set's status, fetch time, digest and tags in the current snapshot
curl --fail "$BASE/discovered" -H "Authorization: Bearer $SQLBUILD_API_KEY"
```

- `wait` holds a response open for at most 60 seconds. Reusing a `request_id` queues nothing new.
- Republishing needs `location.republish`, which the operator, developer and admin roles have.
  Reading `discovered` needs `view`.
- A failed discover function doesn't fail the publish, as its set keeps its last good items. Check
  the set's `status` and `tags` in `discovered` to confirm a deploy.
- The audit log records who asked and why, and the location prints both.
- A location that [`sqb dev`](dev.md#republish-from-a-script) serves takes the same
  request at its UI address, with any `SQLBUILD_API_KEY`, as it has no sign-in.
