import createClient, { type ClientOptions } from "openapi-fetch";
import type { paths } from "./schema.d.ts";

export type { components, operations, paths } from "./schema.d.ts";

/** Typed transport only: credentials and request cancellation belong to the caller. */
export function createControlClient(options: ClientOptions) {
  return createClient<paths>(options);
}
