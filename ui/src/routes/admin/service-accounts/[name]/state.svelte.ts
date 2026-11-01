import type { Account, ApiToken, IssuedToken } from '$lib/api';
import { errorMessage } from '$lib/platform/labels';
import { app } from '$lib/state/app.svelte';
import { createResource } from '$lib/state/resource.svelte';

/** Service accounts with their tokens. Must be created during component initialisation. */
export function createServiceAccountsState() {
	const accounts = createResource((client) => (app.can('user.manage') || app.can('token.manage') ? client.listAccounts().catch(() => []) : Promise.resolve([])));
	const tokens = createResource((client) => (app.can('token.manage') ? client.listServiceAccountTokens() : Promise.resolve([])));

	const services = $derived((accounts.data ?? []).filter((a) => a.kind === 'service_account'));
	const tokensOf = $derived.by(() => {
		const map = new Map<string, ApiToken[]>();
		for (const t of tokens.data ?? []) map.set(t.owner.name, [...(map.get(t.owner.name) ?? []), t]);
		for (const list of map.values()) list.sort((a, b) => Number(a.state !== 'active') - Number(b.state !== 'active') || b.createdAt.localeCompare(a.createdAt));
		return map;
	});

	let busy = $state<string | null>(null);
	let error = $state<{ account: string | null; message: string } | null>(null);
	/** The secret of a token just created or rotated, shown once under its account. */
	let issued = $state<{ account: string; token: IssuedToken<ApiToken> } | null>(null);
	let created = $state<Account | null>(null);

	async function act(key: string, account: string | null, action: () => Promise<unknown>) {
		busy = key;
		error = null;
		try {
			await action();
			accounts.refresh();
			tokens.refresh();
		} catch (e) {
			error = { account, message: errorMessage(e) };
		} finally {
			busy = null;
		}
	}

	return {
		accounts,
		tokens,
		get services() {
			return services;
		},
		tokensOf: (name: string) => tokensOf.get(name) ?? [],
		get busy() {
			return busy;
		},
		get error() {
			return error;
		},
		get issued() {
			return issued;
		},
		get created() {
			return created;
		},
		dismissIssued() {
			issued = null;
		},
		dismissCreated() {
			created = null;
		},
		refresh() {
			accounts.refresh();
			tokens.refresh();
		},
		createAccount: (name: string, description: string) =>
			act('create-account', null, async () => {
				created = await app.client.createServiceAccount({ name, description });
			}),
		createToken: (account: string, label: string, expiresInDays: number | null) =>
			act(`create-token:${account}`, account, async () => {
				issued = { account, token: await app.client.createServiceAccountToken(account, { label, expiresInDays }) };
			}),
		rotate: (t: ApiToken) =>
			act(t.id, t.owner.name, async () => {
				issued = { account: t.owner.name, token: await app.client.rotateServiceAccountToken(t.id) };
			}),
		revoke: (t: ApiToken) => act(t.id, t.owner.name, () => app.client.revokeServiceAccountToken(t.id)),
		disable: (a: Account) => act(a.name, a.name, () => app.client.disableAccount(a.name)),
		enable: (a: Account) => act(a.name, a.name, () => app.client.enableAccount(a.name))
	};
}

export type ServiceAccountsState = ReturnType<typeof createServiceAccountsState>;
