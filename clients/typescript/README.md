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
returned `principal_id` in the `[trusted_proxy]` section of the server's
`server.toml`. Subjects are exact ASCII upstream identities, not inferred local
usernames or email addresses:

```toml
[trusted_proxy]
origin = "https://control.example.com"
trusted_peers = ["127.0.0.1"]

[trusted_proxy.identities]
external-orders-reader = "00000000-0000-0000-0000-000000000001"
```

Replace the example UUID with the created person's ID, then start
`sqb server --config server.toml`. The proxy must terminate HTTPS,
remove any client-supplied `X-SQB-User`, and replace it with the authenticated
upstream subject. Restrict access to the HTTP backend to the configured proxy
peers. Trusting loopback also trusts other local processes on that host;
`Forwarded` and `X-Forwarded-For` never establish peer trust.

Through that proxy, each person request authenticates as the bound person; it
issues no cookie or bearer and grants no machine or resource authority. Requests
that change state must carry the configured `Origin`. Unbound subjects and missing
or disabled people are denied; accounts are not auto-provisioned. Password and
trusted-proxy modes cannot be combined. No default role or SSO group assignment
is inferred from the subject.

## Verify an OIDC identity

Register an authorization-code client with your HTTPS identity provider, using
`https://control.example.com/api/auth/oidc/callback` as its redirect URI. Create
each person through the local control-store-owner administration path and put
their UUID and exact provider subject in the `[oidc]` section of `server.toml`:

```toml
[oidc]
origin = "https://control.example.com"
issuer = "https://identity.example.com"
client_id = "orders-control"

[oidc.identities]
external-orders-reader = "00000000-0000-0000-0000-000000000001"
```

Start `sqb server --config server.toml` behind an HTTPS-terminating proxy that
preserves the configured Host and restricts access to the HTTP backend. For a
confidential client, add `client_secret_env` naming the server environment
variable holding its secret; never put the secret in `server.toml`. To trust a
private provider CA, add `certificate_authority` pointing to a PEM bundle.
Relative certificate paths resolve from the server process's working directory.
TLS verification remains mandatory and upstream HTTP redirects are not followed.
Password, proxy, disabled-auth and OIDC human-authentication modes are exclusive.

Navigate the browser to `/api/auth/oidc/login`. The server redirects to the
provider using S256 PKCE and sets a Secure, HttpOnly, host-only, five-minute
SameSite=Lax flow cookie. The provider's GET callback must return through that
same browser. State is single-use across server replicas; a changed server
identity configuration requires starting a fresh flow. The server validates
the ID token's signature, issuer, audience, expiry and nonce, and an access-token
hash when supplied. Provider keys are refreshed for each exchange.

The callback returns only `principal_id` and clears the flow cookie. Unbound,
missing or disabled people are denied. This identity-only
endpoint does not create an account, browser/CLI session, bearer or role grant,
and cannot authenticate machine routes. It is not a complete browser SSO or
`sqb login` workflow. Authentication denial and successful identity verification
are audited; flow starts share a 20-per-minute real-peer limit across replicas.
