import type { Account, RoleAssignment } from "$lib/api";
import { sourceChip } from "$lib/platform/access";
import type { BuiltinRole, RoleChange, RoleChoice } from "../types";

/** The built-in roles, lowest first; the dropdown offers these. */
export const BUILTIN_ROLES: readonly BuiltinRole[] = [
  "viewer",
  "operator",
  "developer",
  "admin",
];

const isBuiltin = (role: string): role is BuiltinRole =>
  (BUILTIN_ROLES as readonly string[]).includes(role);

/**
 * A person's role as one row shows it. The default role everyone gets is a floor, not a choice: it
 * counts only when nothing else is held. `defaultRole` is that role, or null when there is none.
 */
export function roleChoice(
  assignments: RoleAssignment[],
  defaultRole: string | null,
): RoleChoice {
  const held = assignments.filter((a) => a.source !== "built_in");
  if (held.length === 0)
    return { kind: "single", role: defaultRole ?? "none", given: null };
  const [only] = held;
  if (held.length > 1 || only.scope.locationId !== null)
    return { kind: "mixed" };
  if (only.source !== "manual")
    return { kind: "locked", role: only.role, from: sourceChip(only) };
  return isBuiltin(only.role) && !only.stale
    ? { kind: "single", role: only.role, given: only }
    : { kind: "mixed" };
}

/** What choosing `to` does for a row showing `choice`: nothing is given when `to` is the default or no role. */
export function roleChange(
  choice: Extract<RoleChoice, { kind: "single" }>,
  to: BuiltinRole | "none",
  defaultRole: string | null,
): RoleChange {
  const give: BuiltinRole | null =
    to === "none" || to === defaultRole ? null : to;
  return { give, revoke: choice.given ? [choice.given.id] : [] };
}

/** Whether `name` is the only enabled person with the admin role everywhere, so they can't be demoted. */
export function isLastAdmin(
  name: string,
  assignments: RoleAssignment[],
  accounts: Account[],
): boolean {
  const enabled = new Set(
    accounts.filter((a) => !a.disabled).map((a) => a.name),
  );
  const admins = new Set(
    assignments
      .filter(
        (a) =>
          a.role === "admin" &&
          a.scope.locationId === null &&
          !a.stale &&
          enabled.has(a.principal.name),
      )
      .map((a) => a.principal.name),
  );
  return admins.size === 1 && admins.has(name);
}

/** The default role everyone who signs in gets, from the built-in assignments, or null when there is none. */
export function defaultRoleOf(assignments: RoleAssignment[]): string | null {
  return assignments.find((a) => a.source === "built_in")?.role ?? null;
}
