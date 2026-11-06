// Pure helpers for the access admin pages: role summaries, assignment sources,
// highest role and check-access links.

import type { AccessQuery, AssignmentSource, BuiltInRole, Capability, Location, RoleAssignment, RoleDefinition, RoleSource } from '$lib/api';
import { locationName } from './labels';

const BUILT_IN_LABEL: Record<BuiltInRole, string> = { viewer: 'Viewer', operator: 'Operator', developer: 'Developer', admin: 'Admin' };
const RANK: Record<BuiltInRole, number> = { viewer: 0, operator: 1, developer: 2, admin: 3 };

export function builtInLabel(role: BuiltInRole): string {
	return BUILT_IN_LABEL[role];
}

/** Count of action-on-selector pairs a role adds. */
export function additionCount(role: Pick<RoleDefinition, 'grants'>): number {
	return role.grants.reduce((sum, g) => sum + g.permissions.length * g.nodes.length, 0);
}

/** e.g. "Operator + 2 additions", or "8 actions on everything" for built-in roles. */
export function roleSummary(role: Pick<RoleDefinition, 'source' | 'extends' | 'grants'>): string {
	if (role.source === 'built_in') {
		const n = new Set(role.grants.flatMap((g) => g.permissions)).size;
		return `${n} ${n === 1 ? 'action' : 'actions'} on everything`;
	}
	const n = additionCount(role);
	const base = role.extends ? BUILT_IN_LABEL[role.extends] : 'Nothing';
	return `${base} + ${n} ${n === 1 ? 'addition' : 'additions'}`;
}

export const roleSourceLabel: Record<RoleSource, string> = { built_in: 'Built-in', server_config: 'Server config', project: 'Policy file' };

export const assignmentSourceLabel: Record<AssignmentSource, string> = { manual: 'Manual', sso: 'SSO group', policy: 'Server config', built_in: 'Default for everyone' };

/** Chip text, e.g. `SSO group "analytics-eng"`. */
export function sourceChip(a: Pick<RoleAssignment, 'source' | 'ssoGroup' | 'definedIn'>): string {
	if (a.source === 'sso') return `SSO group “${a.ssoGroup}”`;
	if (a.source === 'policy') return 'Server config';
	if (a.source === 'built_in') return 'Default for everyone';
	return 'Manual';
}

/** Why a derived assignment can't be removed here, and where to change it. */
export function lockedHint(a: Pick<RoleAssignment, 'source' | 'ssoGroup' | 'definedIn'>): string | null {
	if (a.source === 'sso') return `Comes from the SSO group ${a.ssoGroup}. Change the group's members in your identity provider, or the mapping in the server config.`;
	if (a.source === 'policy') return `Set in ${a.definedIn}. Change it there and redeploy.`;
	if (a.source === 'built_in') return `Everyone who signs in gets this default role (${a.definedIn ?? 'server config'}).`;
	return null;
}

/** For "highest role": built-in rank, with custom roles just above their base. Unknown or stale roles rank lowest. */
export function roleRank(roleName: string, roles: readonly RoleDefinition[]): number {
	if (roleName in RANK) return RANK[roleName as BuiltInRole];
	const role = roles.find((r) => r.name === roleName);
	if (!role) return -1;
	return (role.extends ? RANK[role.extends] : 0) + 0.5;
}

/** The highest non-stale assignment, then the rest. */
export function rankAssignments(assignments: readonly RoleAssignment[], roles: readonly RoleDefinition[]): RoleAssignment[] {
	return [...assignments].sort((a, b) => Number(a.stale) - Number(b.stale) || roleRank(b.role, roles) - roleRank(a.role, roles));
}

/** Assignments that apply in a location: scoped to it or to every location. */
export function assignmentsAt(assignments: readonly RoleAssignment[], locationId: string): RoleAssignment[] {
	return assignments.filter((a) => a.scope.locationId === null || a.scope.locationId === locationId);
}

export function scopeWords(locationId: string | null, locations: readonly Location[] | undefined): string {
	return locationId === null ? 'every location' : locationName(locations, locationId);
}

/** Pre-filled access-checker values. */
export type CheckInput = Partial<AccessQuery> & { decision?: string };

/** Link to the access checker, pre-filled. */
export function checkHref(query: CheckInput): string {
	const params = new URLSearchParams();
	if (query.principal) params.set('principal', query.principal);
	if (query.action) params.set('action', query.action);
	if (query.locationId) params.set('location', query.locationId);
	if (query.subject) params.set('subject', query.subject);
	if (query.decision) params.set('decision', query.decision);
	return `/admin/check-access?${params}`;
}

export function personHref(principal: { name: string }): string {
	return `/admin/people/${encodeURIComponent(principal.name)}`;
}

export function roleHref(name: string): string {
	return `/admin/roles/${encodeURIComponent(name)}`;
}

export function isCapability(value: string | null, all: readonly Capability[]): value is Capability {
	return value !== null && (all as readonly string[]).includes(value);
}
