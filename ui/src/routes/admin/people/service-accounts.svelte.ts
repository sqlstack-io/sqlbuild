import { untrack } from "svelte";
import type { ServiceAccount } from "$lib/api";
import { app } from "$lib/state/app.svelte";
import { createResource } from "$lib/state/resource.svelte";

/** The Service accounts tab: the list and the open panel. Create during component initialisation. */
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
    /** Open a service account's panel, or close it when it is already open, as rows do on People. */
    toggle(name: string) {
      selected = selected === name ? null : name;
    },
    open(name: string) {
      selected = name;
    },
    close() {
      selected = null;
    },
    setAdding(value: boolean) {
      adding = value;
    },
  };
}

export type ServiceAccountsState = ReturnType<typeof createServiceAccountsState>;
