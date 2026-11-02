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
const auditMeasurement: components["schemas"]["WarehouseAuditRecord"]["measured_value"] = "99.5";
void auditHistory;
void auditMeasurement;
void client.POST("/api/locations/{location_id}/targets/{target}/metadata/audit-results", {
  params: { path: { location_id: "synthetic-location", target: "development" } },
  body: { session_id: "synthetic-session", generation: 1, records: [] },
});
const janitorHistory: components["schemas"]["WarehouseHistoryKind"] = "janitor_events";
const archivedAt: components["schemas"]["WarehouseJanitorRecord"]["archived_at"] = "2026-01-02T03:04:05Z";
void janitorHistory;
void archivedAt;
void client.POST("/api/locations/{location_id}/targets/{target}/metadata/janitor-events", {
  params: { path: { location_id: "synthetic-location", target: "development" } },
  body: { session_id: "synthetic-session", generation: 1, records: [] },
});
const migrationHistory: components["schemas"]["WarehouseHistoryKind"] = "migrations";
const oldNameHistory: components["schemas"]["WarehouseHistoryKind"] = "old_name_views";
const optionalExpiry: components["schemas"]["WarehouseOldNameRecord"]["expires_at"] = null;
const optionalAliases: components["schemas"]["WarehouseOldNameRecord"]["column_aliases"] = null;
const createdAt: components["schemas"]["WarehouseMigrationRecord"]["created_at"] = "2026-01-02T03:04:05Z";
void migrationHistory;
void oldNameHistory;
void optionalExpiry;
void optionalAliases;
void createdAt;
void client.POST("/api/locations/{location_id}/targets/{target}/metadata/migrations", {
  params: { path: { location_id: "synthetic-location", target: "development" } },
  body: { session_id: "synthetic-session", generation: 1, records: [] },
});
void client.POST("/api/locations/{location_id}/targets/{target}/metadata/old-name-views", {
  params: { path: { location_id: "synthetic-location", target: "development" } },
  body: { session_id: "synthetic-session", generation: 1, records: [] },
});
const microbatchHistory: components["schemas"]["WarehouseHistoryKind"] = "microbatches";
const cursorBound: components["schemas"]["WarehouseMicrobatchRecord"]["run_end"] = "18446744073709551617";
const optionalCompletion: components["schemas"]["WarehouseMicrobatchRecord"]["completed_at"] = null;
void microbatchHistory;
void cursorBound;
void optionalCompletion;
void client.POST("/api/locations/{location_id}/targets/{target}/metadata/microbatches", {
  params: { path: { location_id: "synthetic-location", target: "development" } },
  body: { session_id: "synthetic-session", generation: 1, records: [] },
});
const catalogHistory: components["schemas"]["WarehouseHistoryKind"] = "catalog";
const catalogColumn: components["schemas"]["WarehouseCatalogColumn"] = {
  name: "order_id", data_type: "INTEGER",
};
void catalogHistory;
void catalogColumn;
void client.POST("/api/locations/{location_id}/targets/{target}/metadata/catalog", {
  params: { path: { location_id: "synthetic-location", target: "development" } },
  body: {
    session_id: "synthetic-session", generation: 1, records: [],
    reconciliation: {
      scan_id: "synthetic-scan", page: 0, state_database: null, state_schema: "main",
      day: null, complete: true, after: null,
    },
  },
});
// @ts-expect-error Catalog scans must include reconciliation; incremental imports cannot remove relations safely.
const incompleteCatalog: components["schemas"]["CatalogBatch"] = { session_id: "synthetic-session", generation: 1, records: [] };
void incompleteCatalog;
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
