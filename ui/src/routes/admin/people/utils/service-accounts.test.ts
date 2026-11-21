import { describe, expect, it } from "vitest";
import type { ServiceAccount, ServiceAccountKey } from "$lib/api";
import { keyFacts, nameProblem, statusLine } from "./service-accounts";

const NOW = Date.parse("2026-11-21T12:00:00Z");
const ago = (minutes: number) => new Date(NOW - minutes * 60_000).toISOString();
const ahead = (minutes: number) => new Date(NOW + minutes * 60_000).toISOString();

function key(extra: Partial<ServiceAccountKey> = {}): ServiceAccountKey {
  return {
    id: "k",
    prefix: "sqbk_Q7m2xT",
    createdAt: ago(13),
    createdBy: null,
    lastUsedAt: null,
    expiresAt: null,
    origin: "issued",
    ...extra,
  };
}

function account(extra: Partial<ServiceAccount> = {}): ServiceAccount {
  return {
    kind: "service",
    name: "orders-deploy",
    displayName: "orders-deploy",
    description: null,
    role: "developer",
    createdAt: ago(600),
    createdBy: null,
    lastUsedAt: ago(6),
    disabled: false,
    keys: [key()],
    ...extra,
  };
}

describe("statusLine", () => {
  it("names the state first, then when it was used or when the old key stops", () => {
    expect([
      statusLine(account(), NOW),
      statusLine(account({ keys: [key(), key({ expiresAt: ahead(47) })] }), NOW),
      statusLine(account({ disabled: true }), NOW),
      statusLine(account({ lastUsedAt: null }), NOW),
    ]).toEqual([
      "Active · used 6m ago",
      "Rotating · old key stops in 47m",
      "Disabled · used 6m ago",
      "Not used yet",
    ]);
  });
});

describe("keyFacts", () => {
  it("gives a key's age and last use, and says when it was imported", () => {
    expect([
      keyFacts(key(), NOW),
      keyFacts(key({ origin: "imported", lastUsedAt: ago(2) }), NOW),
    ]).toEqual([
      "created 13m ago · used never",
      "imported 13m ago · used 2m ago",
    ]);
  });
});

describe("nameProblem", () => {
  it("accepts what the server accepts and explains the rest", () => {
    expect(
      ["orders-deploy", "", "Orders", "1st", "a", "ci runner"].map(
        nameProblem,
      ),
    ).toEqual([
      null,
      null,
      "Start with a lowercase letter.",
      "Start with a lowercase letter.",
      "Use 2 to 63 characters.",
      "Use lowercase letters, digits, - and _ only.",
    ]);
  });
});
