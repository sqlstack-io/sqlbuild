# Control API client

The client uses the native server's OpenAPI request and response types with
`openapi-fetch`. It adds no permissions, credential storage, retries or sync policy.
This package is private and consumed from source; it is not published to npm.

Rebuild the native package after changing Rust API types, then run from the repository root:

```sh
uv run --no-sync npm --prefix clients/typescript ci
uv run --no-sync npm --prefix clients/typescript run generate
uv run --no-sync npm --prefix clients/typescript run check
uv run --no-sync npm --prefix clients/typescript test
```

Generation invokes `sqb server --openapi` without opening a control database or
importing project Python. Set `SQLBUILD_EXECUTABLE` to test another built binary.
The check regenerates in memory and fails if the committed types differ. Tests use
a real native server, disposable SQLite store and CLI-issued location credential.

```ts
import { createControlClient } from "@sqlbuild/control-client";

const client = createControlClient({
  baseUrl: serverUrl,
  headers: { Authorization: `Bearer ${locationCredential}` },
});
const { data, error, response } = await client.GET(
  "/api/locations/{location_id}/targets/{target}/runs",
  { params: { path: { location_id: locationId, target }, query: { limit: 100 } } },
);
```

Inspect `response.status` and `error` before using `data`. Pass an `AbortSignal`
for cancellation. For the scoped changes endpoint, set `parseAs: "stream"` and
consume SSE `changed` frames. An `all` topic, a reconnect, or a lost stream requires
refetching the authorized REST resources; the stream is not durable history.
Browser `EventSource` cannot set a bearer header: use a fetch-based SSE consumer
for the existing machine-authenticated endpoint. Human browser sessions are a
separate authentication contract, not an inferred privilege of this client.
