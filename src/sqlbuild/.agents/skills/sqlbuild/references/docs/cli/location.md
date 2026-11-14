<!-- generated-by: sqlbuild skills -->

# sqb location

> Run a code location that publishes a project to the server, and check its health.

Online: https://sqlbuild.com/docs/cli/location/

Run a code location: it compiles the project in its own Python environment, publishes it to the
server, and publishes again when the project changes:

```bash
export SQLBUILD_LOCATION_TOKEN=...
sqb location serve --server https://control.example.com --location-id "$LOCATION_ID" --target production
```

Create the location and its token with `sqb admin create-location orders` and
`sqb admin issue-location-token` on the server's host. The location reads the token from
`SQLBUILD_LOCATION_TOKEN` and never opens the control database.

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
| `healthy` | An instance checked in within 90 seconds and its last load published | `0` |
| `degraded` | The warehouse connection check after the last load failed | `0` |
| `falling_back` | The last load failed, so runs use the previous snapshot | `1` |
| `broken` | No instance has checked in for 90 seconds | `1` |

The command exits `2` when it cannot read the state: the server is unreachable, or the token is
missing, expired or for another location. It needs the location's token, not a person's sign-in.

A Compose health check for the location service:

```yaml
healthcheck:
  test: [CMD, sqb, location, health, --server, "http://server:8000", --location-id, "${SQLBUILD_LOCATION_ID}", --target, development]
  interval: 30s
  start_period: 60s
```

A `falling_back` location still runs scheduled jobs from its last good snapshot. Restarting it
does not fix the failed load; fix the project and deploy it again.
