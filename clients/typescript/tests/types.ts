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
const auditHistory: components["schemas"]["WarehouseHistoryKind"] = "audit_results";
void auditHistory;
void client.POST("/api/locations/{location_id}/targets/{target}/metadata", {
  params: { path: { location_id: "synthetic-location", target: "development" } },
  body: { kind: "janitor_events", session_id: "synthetic-session", generation: 1, records: [] },
});
const catalogColumn: components["schemas"]["WarehouseCatalogColumn"] = {
  name: "order_id", data_type: "INTEGER",
};
void catalogColumn;
void client.POST("/api/locations/{location_id}/targets/{target}/metadata", {
  params: { path: { location_id: "synthetic-location", target: "development" } },
  body: {
    kind: "catalog", session_id: "synthetic-session", generation: 1, records: [],
    scan: { scan_id: "synthetic-scan", location: { database: null, schema: "main" }, complete: true },
  },
});
// @ts-expect-error Every metadata page names the state table it mirrors.
const unnamedPage: components["schemas"]["MetadataBatch"] = { session_id: "synthetic-session", generation: 1, records: [] };
void unnamedPage;
void client.GET("/api/control/locations/{location_id}/targets/{target}/catalog", {
  params: {
    path: { location_id: "synthetic-location", target: "development" },
    query: { schema: "main", after_observed_at: "2026-01-02T03:04:05Z", after_event_id: "synthetic-relation", limit: 100 },
  },
});
const catalogPage: components["schemas"]["CatalogPage"] = { records: [], next_after: null };
void catalogPage;
// @ts-expect-error Catalog reads require an exact physical schema.
void client.GET("/api/control/locations/{location_id}/targets/{target}/catalog", { params: { path: { location_id: "synthetic-location", target: "development" } } });
const sourceCondition: components["schemas"]["SensorCondition"] = {
  kind: "source_new_data", selection: ["source:raw_orders"],
};
void sourceCondition;
// @ts-expect-error Source conditions select sources, not execution event kinds.
const invalidSourceCondition: components["schemas"]["SensorCondition"] = { kind: "source_new_data", selection: ["source:raw_orders"], kinds: ["success"] };
void invalidSourceCondition;
const staleCondition: components["schemas"]["SensorCondition"] = {
  kind: "source_stale", selection: ["source:raw_orders"], after_seconds: 60,
};
void staleCondition;
// @ts-expect-error Staleness conditions require an explicit age threshold.
const missingStaleThreshold: components["schemas"]["SensorCondition"] = { kind: "source_stale", selection: ["source:raw_orders"] };
void missingStaleThreshold;
// @ts-expect-error Session mutations require the CSRF header.
void client.POST("/api/auth/logout", {});
// @ts-expect-error Changing a password also requires the current password.
void client.POST("/api/auth/password", { params: { header: { "x-sqb-csrf": "synthetic-csrf" } }, body: { new_password: "synthetic-password" } });
// @ts-expect-error Session responses never expose a browser-readable bearer.
void ({} as components["schemas"]["PasswordSessionResponse"]).bearer;
void client.GET("/api/control/locations/{location_id}/targets/{target}/fingerprint", {
  params: { path: { location_id: "orders-location", target: "development" },
    query: { node_type: "model", node_name: "orders", schema: "main", name: "orders" } },
});
// @ts-expect-error Build evidence does not contain SQL or arbitrary metadata.
void ({} as components["schemas"]["FingerprintObservation"]).definition;
void client.GET("/api/control/locations/{location_id}/targets/{target}/node-status", {
  params: { path: { location_id: "orders-location", target: "development" },
    query: { node_key: "model:orders" } },
});
const sourceData: components["schemas"]["SourceDataStatus"] = {
  status: "unknown", reason: "consumed-input provenance is not recorded",
};
void sourceData;
// @ts-expect-error Observation times cannot establish consumed-source currency.
const inferredCurrency: components["schemas"]["SourceDataStatus"] = { status: "behind", reason: "newer observation" };
void inferredCurrency;
// @ts-expect-error Node status requires one exact published node.
void client.GET("/api/control/locations/{location_id}/targets/{target}/node-status", { params: { path: { location_id: "orders-location", target: "development" } } });
