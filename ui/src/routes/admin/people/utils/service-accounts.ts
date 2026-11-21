import type { ServiceAccount, ServiceAccountKey } from "$lib/api";
import { formatAgo, formatIn } from "$lib/format/time";
import type { OverlapChoice, PeopleTab, ServiceState } from "../types";

/** How long the old key keeps working after a rotation; `now` stops it at once. */
export const OVERLAP_CHOICES: readonly { value: OverlapChoice; label: string }[] = [
  { value: "now", label: "Now" },
  { value: "15", label: "In 15 minutes" },
  { value: "60", label: "In 1 hour" },
  { value: "1440", label: "In 24 hours" },
];

/** The overlap a rotation asks for, in minutes, or null for none. */
export function overlapMinutes(choice: OverlapChoice): number | null {
  return choice === "now" ? null : Number(choice);
}

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

/** The panel's one status line, e.g. "Active · used 6m ago" or "Rotating · old key stops in 47m". */
export function statusLine(a: ServiceAccount, now: number): string {
  const state = serviceState(a);
  const old = oldKey(a);
  if (state === "rotating" && old)
    return `Rotating · old key stops ${formatIn(old.expiresAt, now)}`;
  if (state === "unused") return "Not used yet";
  return `${SERVICE_STATE_LABEL[state]} · used ${formatAgo(a.lastUsedAt, now)}`;
}

/** A key's line in the panel: when it was created and last used. */
export function keyFacts(k: ServiceAccountKey, now: number): string {
  const used = k.lastUsedAt ? formatAgo(k.lastUsedAt, now) : "never";
  return `${k.origin === "imported" ? "imported" : "created"} ${formatAgo(k.createdAt, now)} · used ${used}`;
}

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
