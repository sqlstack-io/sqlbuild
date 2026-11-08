import type { AccessOverview, Account, RoleAssignment, RoleDefinition } from '$lib/api';
import { app } from './app.svelte';
import { createResource } from './resource.svelte';

/**
 * Accounts with their role assignments, the roles of every location, and
 * per-account risky actions: what the People pages show.
 * Must be created during component initialisation.
 */
export function createAccessDirectory() {
	const canAssign = () => app.can('role.assign');
	const accounts = createResource((client) => (app.can('user.manage') || canAssign() ? client.listAccounts().catch(() => [] as Account[]) : Promise.resolve([] as Account[])));
	const assignments = createResource((client) => (canAssign() ? client.listRoleAssignments() : Promise.resolve([] as RoleAssignment[])));
	const overview = createResource((client) =>
		(app.can('user.manage') || canAssign()) && client.supports('listAccessOverview') ? client.listAccessOverview() : Promise.resolve([] as AccessOverview[])
	);
	const roles = createResource(async (client) => {
		const map = new Map<string, RoleDefinition[]>();
		if (!canAssign() || !client.supports('listRoles')) return map;
		const locations = app.bootstrap?.locations ?? [];
		const lists = await Promise.all(locations.map((l) => client.listRoles(l.id)));
		locations.forEach((l, i) => map.set(l.id, lists[i]));
		return map;
	});

	const rolesByLocation = $derived(roles.data ?? new Map<string, RoleDefinition[]>());
	const allRoles = $derived([...new Map([...rolesByLocation.values()].flat().map((r) => [r.name, r])).values()]);
	const riskyOf = $derived(new Map((overview.data ?? []).map((o) => [o.principal, o.riskyActions])));

	return {
		accounts,
		assignments,
		overview,
		roles,
		get rolesByLocation() {
			return rolesByLocation;
		},
		get allRoles() {
			return allRoles;
		},
		assignmentsOf: (name: string) => (assignments.data ?? []).filter((a) => a.principal.name === name),
		riskyOf: (name: string) => riskyOf.get(name) ?? [],
		refresh() {
			accounts.refresh();
			assignments.refresh();
			overview.refresh();
			roles.refresh();
		}
	};
}

export type AccessDirectory = ReturnType<typeof createAccessDirectory>;
