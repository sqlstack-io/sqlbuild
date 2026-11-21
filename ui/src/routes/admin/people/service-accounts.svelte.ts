import { untrack } from "svelte";
import type { IssuedToken, ServiceAccount } from "$lib/api";
import { app } from "$lib/state/app.svelte";
import { createResource } from "$lib/state/resource.svelte";
import type { IssuedKey } from "./types";

/** The Service accounts tab: the list, the open panel, and a key just issued. Create during component initialisation. */
export function createServiceAccountsState(open: () => string | null) {
  // Only read where the server has them and the person may see them; the page shows why not otherwise.
  const list = createResource((client) =>
    client.supports("listServiceAccounts") &&
    (app.can("user.manage") || app.can("role.assign"))
      ? client.listServiceAccounts()
      : Promise.resolve([] as ServiceAccount[]),
  );
  /** The service account whose panel is open, first from `&open=<name>`. */
  let selected = $state<string | null>(untrack(open));
  let adding = $state(false);
  /** Shown once in the panel; gone when the panel closes or another opens. */
  let issued = $state<IssuedKey | null>(null);

  const accounts = $derived(list.data ?? []);
  const selectedAccount = $derived(
    selected ? (accounts.find((a) => a.name === selected) ?? null) : null,
  );

  return {
    list,
    get accounts() {
      return accounts;
    },
    get selected() {
      return selected;
    },
    get selectedAccount() {
      return selectedAccount;
    },
    get adding() {
      return adding;
    },
    get issued() {
      return issued;
    },
    pick(name: string) {
      adding = false;
      if (name !== selected) issued = null;
      selected = name;
    },
    startAdding() {
      selected = null;
      issued = null;
      adding = true;
    },
    close() {
      selected = null;
      adding = false;
      issued = null;
    },
    /** After add or rotate: open its panel with the new key showing. */
    showIssued(
      result: IssuedToken<ServiceAccount>,
      reason: IssuedKey["reason"],
    ) {
      adding = false;
      selected = result.token.name;
      issued = { name: result.token.name, secret: result.secret, reason };
      list.refresh();
    },
    dismissIssued() {
      issued = null;
    },
  };
}

export type ServiceAccountsState = ReturnType<
  typeof createServiceAccountsState
>;
