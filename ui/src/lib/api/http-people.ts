import type { components } from '$control/schema.d.ts';
import type { Account, RoleAssignment } from './types/platform';

type Wire = components['schemas'];

/** A person's sign-in source as far as the server knows it: a password, or SSO groups. */
export function toAccount(wire: Wire['PersonSummary']): Account {
	return {
		kind: 'user',
		name: wire.name,
		displayName: wire.name,
		email: null,
		source: wire.has_password ? 'password' : wire.groups.length ? 'oidc' : null,
		ssoGroups: wire.groups,
		createdAt: wire.created_at,
		lastSeenAt: wire.last_signed_in_at ?? null,
		disabled: wire.disabled_at != null,
		activeSessions: wire.active_sessions
	};
}

function scopeKey(locationId: string | null | undefined, target: string | null | undefined): string | null {
	return locationId ? (target ? `${locationId}:${target}` : locationId) : null;
}

/** Assignments given here, then one SSO assignment per member of each mapped group. */
export function toRoleAssignments(holdings: Wire['RoleHoldings'], people: Wire['PersonSummary'][]): RoleAssignment[] {
	const person = (name: string) => ({ kind: 'user' as const, name, displayName: name });
	const manual: RoleAssignment[] = holdings.assignments.map((a) => ({
		id: a.assignment_id,
		principal: person(a.name),
		role: a.role_key,
		scope: { locationId: scopeKey(a.location_id, a.target) },
		source: 'manual',
		ssoGroup: null,
		definedIn: null,
		assignedBy: null,
		assignedAt: null,
		stale: false
	}));
	const viaGroups: RoleAssignment[] = holdings.group_roles.flatMap((g) =>
		people
			.filter((p) => p.groups.includes(g.group_name))
			.map((p) => ({
				id: `sso:${g.group_name}:${g.role_key}:${scopeKey(g.location_id, g.target) ?? '*'}:${p.principal_id}`,
				principal: person(p.name),
				role: g.role_key,
				scope: { locationId: scopeKey(g.location_id, g.target) },
				source: 'sso' as const,
				ssoGroup: g.group_name,
				definedIn: null,
				assignedBy: null,
				assignedAt: null,
				stale: false
			}))
	);
	return [...manual, ...viaGroups];
}
