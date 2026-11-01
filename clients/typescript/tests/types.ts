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
