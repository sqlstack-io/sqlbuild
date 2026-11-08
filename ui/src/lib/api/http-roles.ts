import type { components } from '$control/schema.d.ts';
import type { BuiltInRole, Capability } from './types';
import type { RoleAssignment, RoleDefinition, RoleDetail } from './types/platform';

type Wire = components['schemas'];

/** What each built-in role is for; the server owns their actions. */
export const BUILT_IN_DESCRIPTION: Record<BuiltInRole, string> = {
	viewer: 'Sees runs, jobs, triggers, checks and the catalog.',
	operator: 'Runs jobs, cancels and reruns runs, and pauses or resumes triggers.',
	developer: 'Everything an operator does, plus SQL, ad-hoc runs and connection tests.',
	admin: 'Every action, including people, roles and the audit log.'
};

/** A role as the Roles list shows it; where it's defined and how it grew aren't known to the server. */
export function toRoleDefinition(wire: Wire['RoleOverview']): RoleDefinition {
	return {
		name: wire.key,
		source: wire.builtin ? 'built_in' : 'project',
		definedIn: null,
		description: wire.builtin ? BUILT_IN_DESCRIPTION[wire.key as BuiltInRole] : (wire.description ?? ''),
		extends: null,
		grants: wire.grants.map((g) => ({ nodes: g.nodes, permissions: g.permissions as Capability[] })),
		holders: wire.holders,
		recentlyMatched: null
	};
}

/** Whether an assignment applies to this location key: everywhere, the whole location, or this target. */
function covers(assignment: RoleAssignment, locationKey: string): boolean {
	const scope = assignment.scope.locationId;
	return scope === null || scope === locationKey || locationKey.startsWith(`${scope}:`);
}

/** One role with what each selector matched at the latest publish and who holds it here. */
export function roleDetailOf(wire: Wire['RoleOverview'], assignments: RoleAssignment[], locationKey: string): RoleDetail {
	const role = toRoleDefinition(wire);
	return {
		...role,
		baseGrants: [],
		selectors: wire.grants.flatMap((g) => g.nodes.map((selector) => ({ selector, nodes: g.matched[selector] ?? [], newSinceLastWeek: [] }))),
		assignments: assignments.filter((a) => a.role === wire.key && covers(a, locationKey)),
		definition: wire.builtin ? null : JSON.stringify({ description: wire.description, grants: wire.grants.map(({ nodes, permissions }) => ({ nodes, permissions })) }, null, 2)
	};
}
