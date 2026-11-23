import { describe, expect, it } from "vitest";
import type { Account } from "$lib/api";
import { accountToggle } from "./account-toggle";

const account = (extra: Partial<Account>) =>
  ({
    kind: "user",
    name: "morgan",
    displayName: "Morgan Diaz",
    disabled: false,
    activeSessions: 2,
    ...extra,
  }) as Account;

describe("accountToggle", () => {
  it("asks before disabling, naming the sessions that end, and enabling is not destructive", () => {
    expect([
      accountToggle(account({})),
      accountToggle(account({ activeSessions: 1 })).body,
      accountToggle(account({ disabled: true })),
    ]).toEqual([
      {
        title: "Disable Morgan Diaz?",
        body: "Morgan Diaz (morgan) can't sign in any more, in the browser or with sqb login, and their 2 signed-in sessions end now.",
        confirm: "Disable",
        destructive: true,
      },
      "Morgan Diaz (morgan) can't sign in any more, in the browser or with sqb login, and their 1 signed-in session ends now.",
      {
        title: "Enable Morgan Diaz?",
        body: "Morgan Diaz (morgan) can sign in again.",
        confirm: "Enable",
        destructive: false,
      },
    ]);
  });
});
