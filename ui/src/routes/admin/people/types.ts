import type { RoleAssignment } from "$lib/api";

/** The built-in roles a row's dropdown offers, lowest first. */
export type BuiltinRole = "viewer" | "operator" | "developer" | "admin";

/**
 * What a People row shows for a person's role:
 * - `single`: one role everywhere, changeable inline when it was given by hand (or is only the default);
 * - `locked`: one role everywhere from SSO or a policy file, changed where it is set;
 * - `mixed`: roles that differ by location, or more than one, managed on their page.
 */
export type RoleChoice =
  | { kind: "single"; role: string; given: RoleAssignment | null }
  | { kind: "locked"; role: string; from: string }
  | { kind: "mixed" };

/** What changing a row's role does: give the new role everywhere, then take back the assignments it replaces. */
export type RoleChange = { give: BuiltinRole | null; revoke: string[] };
