import { describe, expect, it } from "vitest";
import type { Account, RoleAssignment } from "$lib/api";
import type { BuiltinRole, RoleChoice } from "../types";
import { isLastAdmin, roleChange, roleChoice } from "./role-choice";

function assignment(
  role: string,
  source: RoleAssignment["source"],
  locationId: string | null = null,
  name = "alex",
): RoleAssignment {
  return {
    id: `${name}-${role}-${source}-${locationId}`,
    principal: {
      name,
      displayName: name,
      kind: "user",
    } as RoleAssignment["principal"],
    role,
    scope: { locationId },
    source,
    ssoGroup: source === "sso" ? "analytics" : null,
    definedIn: null,
    assignedBy: null,
    assignedAt: null,
    stale: false,
  };
}
const DEFAULT = assignment("viewer", "built_in");
const GIVEN = assignment("developer", "manual");

describe("roleChoice", () => {
  const cases: {
    description: string;
    assignments: RoleAssignment[];
    defaultRole: string | null;
    expected: RoleChoice;
  }[] = [
    {
      description: "only the default role is a single changeable role",
      assignments: [DEFAULT],
      defaultRole: "viewer",
      expected: { kind: "single", role: "viewer", given: null },
    },
    {
      description: "nothing held and no default is no role",
      assignments: [],
      defaultRole: null,
      expected: { kind: "single", role: "none", given: null },
    },
    {
      description: "one built-in role given by hand everywhere is changeable",
      assignments: [DEFAULT, GIVEN],
      defaultRole: "viewer",
      expected: { kind: "single", role: "developer", given: GIVEN },
    },
    {
      description: "one role from SSO is locked to its group",
      assignments: [DEFAULT, assignment("developer", "sso")],
      defaultRole: "viewer",
      expected: {
        kind: "locked",
        role: "developer",
        from: "SSO group “analytics”",
      },
    },
    {
      description: "a role in one location is mixed",
      assignments: [DEFAULT, assignment("operator", "manual", "shop")],
      defaultRole: "viewer",
      expected: { kind: "mixed" },
    },
    {
      description: "two roles everywhere are mixed",
      assignments: [
        assignment("operator", "manual"),
        assignment("developer", "sso"),
      ],
      defaultRole: null,
      expected: { kind: "mixed" },
    },
    {
      description: "a custom role is managed on their page",
      assignments: [assignment("support_viewer", "manual")],
      defaultRole: null,
      expected: { kind: "mixed" },
    },
  ];
  for (const { description, assignments, defaultRole, expected } of cases)
    it(description, () =>
      expect(roleChoice(assignments, defaultRole)).toEqual(expected),
    );
});

describe("roleChange", () => {
  const cases: {
    description: string;
    choice: Extract<RoleChoice, { kind: "single" }>;
    to: BuiltinRole | "none";
    expected: { give: BuiltinRole | null; revoke: string[] };
  }[] = [
    {
      description: "from the default, the new role is given",
      choice: { kind: "single", role: "viewer", given: null },
      to: "developer",
      expected: { give: "developer", revoke: [] },
    },
    {
      description:
        "a given role is replaced: the new one given, the old taken back",
      choice: { kind: "single", role: "developer", given: GIVEN },
      to: "admin",
      expected: { give: "admin", revoke: [GIVEN.id] },
    },
    {
      description: "back to the default only takes the given role back",
      choice: { kind: "single", role: "developer", given: GIVEN },
      to: "viewer",
      expected: { give: null, revoke: [GIVEN.id] },
    },
  ];
  for (const { description, choice, to, expected } of cases)
    it(description, () =>
      expect(roleChange(choice, to, "viewer")).toEqual(expected),
    );
});

describe("isLastAdmin", () => {
  const accounts = (names: string[], disabled: string[] = []) =>
    names.map(
      (name) => ({ name, disabled: disabled.includes(name) }) as Account,
    );
  const cases: {
    description: string;
    assignments: RoleAssignment[];
    accounts: Account[];
    expected: boolean;
  }[] = [
    {
      description: "the only admin is the last",
      assignments: [assignment("admin", "manual")],
      accounts: accounts(["alex", "sam"]),
      expected: true,
    },
    {
      description: "one of two admins is not",
      assignments: [
        assignment("admin", "manual"),
        assignment("admin", "manual", null, "sam"),
      ],
      accounts: accounts(["alex", "sam"]),
      expected: false,
    },
    {
      description: "a disabled second admin does not count",
      assignments: [
        assignment("admin", "manual"),
        assignment("admin", "manual", null, "sam"),
      ],
      accounts: accounts(["alex", "sam"], ["sam"]),
      expected: true,
    },
    {
      description: "someone who is not an admin is not the last",
      assignments: [assignment("admin", "manual", null, "sam")],
      accounts: accounts(["alex", "sam"]),
      expected: false,
    },
  ];
  for (const { description, assignments, accounts: people, expected } of cases)
    it(description, () =>
      expect(isLastAdmin("alex", assignments, people)).toBe(expected),
    );
});
