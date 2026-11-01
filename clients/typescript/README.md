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

## Authenticate through a trusted proxy

Create a person with `sqb admin create-principal orders-reader`, then put the
returned `principal_id` in a server-owned JSON file. Subjects are exact ASCII
upstream identities, not inferred local usernames or email addresses:

```json
{
  "origin": "https://control.example.com",
  "trusted_peers": ["127.0.0.1"],
  "identities": {
    "external-orders-reader": "00000000-0000-0000-0000-000000000001"
  }
}
```

Replace the example UUID with the created person's ID, then start
`sqb server --trusted-proxy-config proxy.json`. The proxy must terminate HTTPS,
remove any client-supplied `X-SQB-User`, and replace it with the authenticated
upstream subject. Restrict access to the HTTP backend to the configured proxy
peers. Trusting loopback also trusts other local processes on that host;
`Forwarded` and `X-Forwarded-For` never establish peer trust.

Through that proxy, `GET /api/auth/proxy` returns the bound `principal_id` only.
It issues no cookie or bearer and grants no machine or resource authority.
Unbound subjects, missing or disabled people, and service accounts are denied;
accounts are not auto-provisioned. Password and trusted-proxy modes cannot be
combined. No default role or SSO group assignment is inferred by this identity
endpoint.
