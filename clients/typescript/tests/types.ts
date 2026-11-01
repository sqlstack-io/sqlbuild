import { createControlClient, type components } from "../src/client.ts";

// Compile-only consumer contracts: these must fail if generated typing is lost.
const client = createControlClient({ baseUrl: "https://example.com" });
// @ts-expect-error Unknown paths are not accepted.
void client.GET("/api/not-a-route");
// @ts-expect-error The location path parameter is required.
void client.GET("/api/locations/{location_id}/health", {});
// @ts-expect-error Only the native discriminated health variants are accepted.
const invalid: components["schemas"]["LocationHealth"] = { kind: "unknown" };
void invalid;
// @ts-expect-error Session mutations require the CSRF header.
void client.POST("/api/auth/logout", {});
// @ts-expect-error Changing a password also requires the current password.
void client.POST("/api/auth/password", { params: { header: { "x-sqb-csrf": "synthetic-csrf" } }, body: { new_password: "synthetic-password" } });
// @ts-expect-error Session responses never expose a browser-readable bearer.
void ({} as components["schemas"]["PasswordSessionResponse"]).bearer;
