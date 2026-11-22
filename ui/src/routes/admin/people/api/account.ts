import type { Account } from "$lib/api";
import { app } from "$lib/state/app.svelte";
import type { RoleChange } from "../types";

/** Give a person's new role everywhere, then take back the assignments it replaces, so they never lose access in between. */
export async function changeRole(
  principal: string,
  change: RoleChange,
): Promise<void> {
  if (change.give)
    await app.client.createRoleAssignment({
      principal,
      role: change.give,
      scope: { locationId: null },
    });
  for (const id of change.revoke) await app.client.deleteRoleAssignment(id);
}

/** Enable or disable an account once its person confirms; false when they cancel. */
export async function toggleAccount(a: Account): Promise<boolean> {
  const sessions =
    a.activeSessions === 1
      ? "their 1 signed-in session ends"
      : `their ${a.activeSessions} signed-in sessions end`;
  const ask = a.disabled
    ? `Enable ${a.displayName} (${a.name})? They can sign in again.`
    : `Disable ${a.displayName} (${a.name})? They can't sign in any more, in the browser or with sqb login, and ${sessions} now.`;
  if (!confirm(ask)) return false;
  await (a.disabled
    ? app.client.enableAccount(a.name)
    : app.client.disableAccount(a.name));
  return true;
}
