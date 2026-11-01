import type { RoleApproval, RoleDefinition, SsoMapping } from '$lib/api';
import { errorMessage } from '$lib/platform/labels';
import { app } from '$lib/state/app.svelte';
import { createResource } from '$lib/state/resource.svelte';

/** Roles of the selected location, SSO mappings and approvals. Must be created during component initialisation. */
export function createRolesState() {
	const can = () => app.can('role.assign');
	const roles = createResource((client, scope) => (can() ? client.listRoles(scope.locationId) : Promise.resolve([] as RoleDefinition[])));
	const sso = createResource((client) => (can() ? client.listSsoMappings() : Promise.resolve([] as SsoMapping[])));
	const approvals = createResource((client) => (can() ? client.listRoleApprovals() : Promise.resolve([] as RoleApproval[])));

	const pending = $derived((approvals.data ?? []).filter((a) => a.state === 'pending'));
	const decided = $derived((approvals.data ?? []).filter((a) => a.state !== 'pending'));

	let busy = $state<string | null>(null);
	let errors = $state<Record<string, string>>({});

	async function decide(approval: RoleApproval, decision: 'approve' | 'reject', note: string): Promise<boolean> {
		busy = approval.id;
		errors = { ...errors, [approval.id]: '' };
		try {
			await app.client.decideRoleApproval(approval.id, decision, note);
			approvals.refresh();
			roles.refresh();
			app.refreshNavCounts();
			return true;
		} catch (e) {
			errors = { ...errors, [approval.id]: errorMessage(e) };
			return false;
		} finally {
			busy = null;
		}
	}

	return {
		roles,
		sso,
		approvals,
		get pending() {
			return pending;
		},
		get decided() {
			return decided;
		},
		get busy() {
			return busy;
		},
		errorOf: (id: string) => errors[id] || null,
		decide,
		refresh() {
			roles.refresh();
			sso.refresh();
			approvals.refresh();
		}
	};
}

export type RolesState = ReturnType<typeof createRolesState>;
