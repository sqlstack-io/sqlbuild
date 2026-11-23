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

/** Enable a disabled account, or disable an enabled one. */
export async function setAccountEnabled(a: Account): Promise<void> {
  await (a.disabled
    ? app.client.enableAccount(a.name)
    : app.client.disableAccount(a.name));
}
