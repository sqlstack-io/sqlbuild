import type { ServiceAccount, ServiceAccountKey } from "$lib/api";
import type { PeopleTab, ServiceState } from "../types";

/** How long the old key keeps working after a rotation, in minutes; null stops it at once. */
export const OVERLAP_CHOICES: readonly {
  minutes: number | null;
  label: string;
}[] = [
  { minutes: null, label: "No overlap: the old key stops now" },
  { minutes: 15, label: "15 minutes" },
  { minutes: 60, label: "1 hour" },
  { minutes: 24 * 60, label: "24 hours" },
];

/** The People page's tab from `?tab=`; anything else is People. */
export function parsePeopleTab(value: string | null): PeopleTab {
  return value === "service-accounts" ? value : "people";
}

/** The key new requests should use. */
export function currentKey(a: ServiceAccount): ServiceAccountKey | null {
  return a.keys.find((k) => k.expiresAt === null) ?? null;
}

/** The key a rotation replaced, while it still works. */
export function oldKey(a: ServiceAccount): ServiceAccountKey | null {
  return a.keys.find((k) => k.expiresAt !== null) ?? null;
}

export function serviceState(a: ServiceAccount): ServiceState {
  if (a.disabled) return "disabled";
  if (oldKey(a)) return "rotating";
  return a.lastUsedAt ? "active" : "unused";
}

export const SERVICE_STATE_LABEL: Record<ServiceState, string> = {
  active: "Active",
  rotating: "Rotating",
  disabled: "Disabled",
  unused: "Not used yet",
};

/** Why a new service account's name won't do, or null; the server checks the same and that it's free. */
export function nameProblem(name: string): string | null {
  const n = name.trim();
  if (!n) return null;
  if (!/^[a-z]/.test(n)) return "Start with a lowercase letter.";
  if (!/^[a-z][a-z0-9_-]*$/.test(n))
    return "Use lowercase letters, digits, - and _ only.";
  if (n.length < 2 || n.length > 63) return "Use 2 to 63 characters.";
  return null;
}
