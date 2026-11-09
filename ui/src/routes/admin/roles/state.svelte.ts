import type { RoleDefinition, SsoMapping } from '$lib/api';
import { app } from '$lib/state/app.svelte';
import { createResource } from '$lib/state/resource.svelte';

/** Roles of the selected location and SSO mappings. Must be created during component initialisation. */
export function createRolesState() {
	const can = () => app.can('role.assign');
	const roles = createResource((client, scope) => (can() ? client.listRoles(scope.locationId) : Promise.resolve([] as RoleDefinition[])));
	const sso = createResource((client) => (can() && client.supports('listSsoMappings') ? client.listSsoMappings() : Promise.resolve([] as SsoMapping[])));

	return {
		roles,
		sso,
		refresh() {
			roles.refresh();
			sso.refresh();
		}
	};
}

export type RolesState = ReturnType<typeof createRolesState>;
