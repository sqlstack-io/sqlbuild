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
const sync: components["schemas"]["LocationHealth"] = {
  kind: "warehouse_sync", generation: 1,
  location: { database: null, schema: "main" }, table: "fingerprints",
  failure: "read", records_sent: 0, elapsed_millis: 43,
};
void sync;
declare const observation: components["schemas"]["LocationHealthObservation"];
if (observation.report.health.kind === "warehouse_sync") {
  const records: number = observation.report.health.records_sent;
  void records;
}
// @ts-expect-error Raw warehouse exceptions cannot be used as a health failure stage.
const invalidFailure: components["schemas"]["WarehouseSyncFailure"] = "warehouse exception text";
void invalidFailure;
// @ts-expect-error Session mutations require the CSRF header.
void client.POST("/api/auth/logout", {});
// @ts-expect-error Changing a password also requires the current password.
void client.POST("/api/auth/password", { params: { header: { "x-sqb-csrf": "synthetic-csrf" } }, body: { new_password: "synthetic-password" } });
// @ts-expect-error Session responses never expose a browser-readable bearer.
void ({} as components["schemas"]["PasswordSessionResponse"]).bearer;
