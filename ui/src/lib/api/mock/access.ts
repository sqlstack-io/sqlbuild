// Mock authorizer: resolves assignments → roles → grants → scopes the way the
// server does, so the admin pages can explain answers. Pure functions over the
// mock platform data.

import type {
	AccessExplanation,
	AccessGrantPath,
	AccessNearMiss,
	AccessQuery,
	Account,
	ApprovalImpact,
	BuiltInRole,
	Capability,
	EffectiveAccess,
	EffectiveAccessCell,
	Location,
	Principal,
	RoleApproval,
	RoleAssignment,
	RoleDefinition,
	RoleGrant,
	SelectorMatch
} from '../types';
import { CAPABILITIES } from '../types';
import { ROLE_CAPABILITIES, SHOP_LOCATIONS } from './fixtures';
import { jobDef, jobNodes } from './operate';
import { resolveSelection } from './operate-graph';

export const ADMIN_ONLY: readonly Capability[] = ['location.manage', 'user.manage', 'role.assign', 'token.manage', 'audit.view'];

/** Nodes that started matching a selector in the last 7 days (mock history). */
const RECENTLY_MATCHED: Record<string, string[]> = {
	'tag:finance': ['revenue_report']
};

export type AccessData = {
	roles: RoleDefinition[];
	assignments: RoleAssignment[];
	accounts: Account[];
	approvals: RoleApproval[];
	locations: Location[];
};

/** A grant as the authorizer sees it: a selector (null = every node) and its actions. */
type ResolvedGrant = { selector: string | null; permissions: Capability[]; via: 'base' | 'addition'; baseRole: BuiltInRole | null };

/** Nodes a selector matches; commas intersect. */
export function selectorNodes(selector: string): string[] {
	const parts = selector.split(',').map((p) => p.trim()).filter(Boolean);
	let nodes: string[] | null = null;
	for (const part of parts) {
		const hit = resolveSelection([part]).nodes;
		nodes = nodes === null ? hit : nodes.filter((n) => hit.includes(n));
	}
	return nodes ?? [];
}

/** Nodes a request's subject stands for: a job's write set, a selector's matches, or one node. */
export function subjectNodes(subject: string): string[] {
	if (jobDef(subject)) return jobNodes(subject);
	if (subject.includes(',')) return selectorNodes(subject);
	const resolved = resolveSelection([subject]);
	return resolved.unknown.length ? [] : resolved.nodes;
}

export function roleAvailableAt(role: RoleDefinition, locationId: string): boolean {
	return role.source !== 'project' || SHOP_LOCATIONS.includes(locationId);
}

export function baseGrantsOf(role: RoleDefinition): RoleGrant[] {
	return role.extends ? [{ nodes: ['*'], permissions: [...ROLE_CAPABILITIES[role.extends]] }] : [];
}

function resolvedGrants(role: RoleDefinition, extra: RoleGrant[] = []): ResolvedGrant[] {
	const out: ResolvedGrant[] = [];
	const builtIn = role.source === 'built_in';
	for (const g of baseGrantsOf(role)) out.push({ selector: null, permissions: g.permissions, via: 'base', baseRole: role.extends });
	for (const g of [...role.grants, ...extra]) {
		const permissions = builtIn ? g.permissions : g.permissions.filter((p) => !ADMIN_ONLY.includes(p));
		for (const selector of g.nodes) {
			out.push({ selector: selector === '*' ? null : selector, permissions, via: builtIn ? 'base' : 'addition', baseRole: builtIn ? (role.name as BuiltInRole) : role.extends });
		}
	}
	return out;
}

/** Whether a grant's selector covers the subject (null subject = the whole location). */
function covers(selector: string | null, subject: string | null): boolean {
	if (selector === null) return true;
	if (subject === null) return false;
	if (selector === subject) return true;
	const wanted = subjectNodes(subject);
	if (!wanted.length) return false;
	const reach = selectorNodes(selector);
	return wanted.every((n) => reach.includes(n));
}

function appliesAt(a: RoleAssignment, locationId: string): boolean {
	return a.scope.locationId === null || a.scope.locationId === locationId;
}

function locationName(data: AccessData, locationId: string | null): string {
	if (locationId === null) return 'every location';
	return data.locations.find((l) => l.id === locationId)?.name ?? locationId;
}

function label(action: Capability): string {
	return PLAIN[action];
}

/** Server-side wording for summaries; the UI uses its own permission catalog. */
const PLAIN: Record<Capability, string> = {
	view: 'see jobs, runs and results',
	'view.sql': 'see SQL',
	'view.evidence': 'see sample rows',
	'job.run': 'run jobs',
	'run.cancel': 'cancel runs',
	'run.rerun': 'rerun runs',
	'run.rewrite_history': 'full refresh or reprocess past data',
	'trigger.toggle': 'pause and resume schedules and sensors',
	'sensor.reset_cursor': 'reset where a sensor is up to',
	'warehouse.sync': 'refresh the warehouse sync',
	'run.adhoc': 'build any selection of nodes',
	'connection.test': 'test warehouse connections',
	'plan.preview': 'preview build plans',
	'location.manage': 'manage locations',
	'user.manage': 'manage people and service accounts',
	'role.assign': 'give roles',
	'token.manage': 'manage service-account tokens',
	'audit.view': 'see the audit log'
};

function roleOf(data: AccessData, name: string): RoleDefinition | undefined {
	return data.roles.find((r) => r.name === name);
}

/** Chains that allow `query` for one account, ignoring the account's status. */
function chains(data: AccessData, query: AccessQuery, extra?: { role: string; grants: RoleGrant[] }): AccessGrantPath[] {
	const out: AccessGrantPath[] = [];
	for (const a of data.assignments) {
		if (a.principal.name !== query.principal || a.stale || !appliesAt(a, query.locationId)) continue;
		const role = roleOf(data, a.role);
		if (!role || !roleAvailableAt(role, query.locationId)) continue;
		const grants = resolvedGrants(role, extra?.role === role.name ? extra.grants : []);
		const hit = grants.find((g) => g.permissions.includes(query.action) && covers(g.selector, query.subject));
		if (hit) {
			out.push({
				assignmentId: a.id,
				assignmentSource: a.source,
				ssoGroup: a.ssoGroup,
				definedIn: a.definedIn,
				scope: a.scope,
				role: role.name,
				via: hit.via,
				baseRole: hit.baseRole,
				selector: hit.selector
			});
		}
	}
	return out;
}

export function allows(data: AccessData, query: AccessQuery): boolean {
	const account = data.accounts.find((a) => a.name === query.principal);
	return !!account && !account.disabled && chains(data, query).length > 0;
}

export function explain(data: AccessData, query: AccessQuery): AccessExplanation {
	const account = data.accounts.find((a) => a.name === query.principal)!;
	const grantedBy = chains(data, query);
	const nearMisses: AccessNearMiss[] = [];
	const where = query.subject ? `${query.subject} in ${locationName(data, query.locationId)}` : locationName(data, query.locationId);

	if (account.disabled) nearMisses.push({ reason: 'disabled', role: null, assignmentId: null, detail: `${account.name} is disabled, so nothing they hold applies until they are enabled again.` });

	if (!grantedBy.length) {
		for (const a of data.assignments) {
			if (a.principal.name !== query.principal) continue;
			const role = roleOf(data, a.role);
			if (a.stale || !role) {
				if (appliesAt(a, query.locationId)) nearMisses.push({ reason: 'stale', role: a.role, assignmentId: a.id, detail: `Has ${a.role}, but that role no longer exists in the code, so it gives nothing.` });
				continue;
			}
			const grants = resolvedGrants(role).filter((g) => g.permissions.includes(query.action));
			if (role.source !== 'built_in' && ADMIN_ONLY.includes(query.action) && role.grants.some((g) => g.permissions.includes(query.action))) {
				nearMisses.push({ reason: 'admin_only', role: role.name, assignmentId: a.id, detail: `${role.name} lists ${query.action}, but admin actions can only come from built-in roles, so it's ignored.` });
				continue;
			}
			if (!grants.length) continue;
			if (appliesAt(a, query.locationId) && roleAvailableAt(role, query.locationId)) {
				const selectors = [...new Set(grants.map((g) => g.selector).filter((s): s is string => s !== null))];
				nearMisses.push({
					reason: 'other_nodes',
					role: role.name,
					assignmentId: a.id,
					detail: `Has ${label(query.action)} on ${selectors.join(', ')} through ${role.name}, ${query.subject ? `not on ${query.subject}` : 'not on every node: name a job, group or node to check'}.`
				});
			} else {
				nearMisses.push({
					reason: 'other_location',
					role: role.name,
					assignmentId: a.id,
					detail: `Has ${label(query.action)} in ${locationName(data, a.scope.locationId)} through ${role.name}, not in ${locationName(data, query.locationId)}.`
				});
			}
		}
	}

	let pending: AccessExplanation['pending'] = null;
	if (!grantedBy.length) {
		for (const approval of data.approvals) {
			if (approval.state !== 'pending' || approval.locationId !== query.locationId) continue;
			if (chains(data, query, { role: approval.role, grants: approval.added }).length) {
				pending = { approvalId: approval.id, role: approval.role };
				break;
			}
		}
	}

	const assigned = new Set(grantedBy.map((g) => g.role));
	const wouldBeGrantedBy = data.roles
		.filter((r) => !assigned.has(r.name) && roleAvailableAt(r, query.locationId) && resolvedGrants(r).some((g) => g.permissions.includes(query.action) && covers(g.selector, query.subject)))
		.map((r) => r.name);

	const allowed = grantedBy.length > 0 && !account.disabled;
	const who = account.displayName;
	const summary = allowed
		? `Yes: ${who} can ${label(query.action)} on ${where}.`
		: account.disabled
			? `No: ${who} is disabled.`
			: `No: ${who} can't ${label(query.action)} on ${where}.`;
	return { query, allowed, grantedBy, nearMisses, pending, wouldBeGrantedBy, summary };
}

export function effective(data: AccessData, principal: string): EffectiveAccess {
	const locationIds = data.locations.map((l) => l.id);
	const disabled = data.accounts.find((a) => a.name === principal)?.disabled ?? false;
	const cells: EffectiveAccessCell[] = [];
	for (const locationId of locationIds) {
		for (const action of CAPABILITIES) {
			let all = false;
			const selectors = new Set<string>();
			const roles = new Set<string>();
			for (const a of data.assignments) {
				if (disabled || a.principal.name !== principal || a.stale || !appliesAt(a, locationId)) continue;
				const role = roleOf(data, a.role);
				if (!role || !roleAvailableAt(role, locationId)) continue;
				for (const g of resolvedGrants(role)) {
					if (!g.permissions.includes(action)) continue;
					roles.add(role.name);
					if (g.selector === null) all = true;
					else selectors.add(g.selector);
				}
			}
			cells.push({ action, locationId, coverage: all ? 'all' : selectors.size ? 'some' : 'none', selectors: all ? [] : [...selectors], roles: [...roles] });
		}
	}
	return { principal, locationIds, cells };
}

/** Who gains each added action, per selector, if the change is approved. */
export function approvalImpact(data: AccessData, approval: RoleApproval): ApprovalImpact[] {
	const holders = data.assignments.filter((a) => a.role === approval.role && !a.stale && appliesAt(a, approval.locationId));
	const out: ApprovalImpact[] = [];
	for (const grant of approval.added) {
		for (const selector of grant.nodes) {
			for (const action of grant.permissions) {
				const principals: Principal[] = [];
				for (const h of holders) {
					if (principals.some((p) => p.name === h.principal.name)) continue;
					const query = { principal: h.principal.name, action, locationId: approval.locationId, subject: selector === '*' ? null : selector };
					if (!allows(data, query)) principals.push(h.principal);
				}
				out.push({ action, selector, locationId: approval.locationId, principals });
			}
		}
	}
	return out;
}

export function selectorMatches(role: RoleDefinition): SelectorMatch[] {
	const selectors = [...new Set(role.grants.flatMap((g) => g.nodes).filter((n) => n !== '*'))];
	return selectors.map((selector) => {
		const nodes = selectorNodes(selector);
		const recent = selector.split(',').flatMap((part) => RECENTLY_MATCHED[part.trim()] ?? []);
		return { selector, nodes, newSinceLastWeek: nodes.filter((n) => recent.includes(n)) };
	});
}

export function recentlyMatchedCount(role: RoleDefinition): number {
	return new Set(selectorMatches(role).flatMap((m) => m.newSinceLastWeek)).size;
}

/** The role as written in its policy file or the server config. */
export function definitionText(role: RoleDefinition, grants: RoleGrant[] = role.grants): string {
	if (role.source === 'built_in') {
		return [`# Built in; not defined in a file.`, `# ${role.name}: ${role.description}`, `permissions = [${grants.flatMap((g) => g.permissions).map((p) => `"${p}"`).join(', ')}]`].join('\n');
	}
	const lines = [`[roles.${role.name}]`, `description = "${role.description}"`];
	if (role.extends) lines.push(`extends = "${role.extends}"`);
	lines.push('grants = [');
	for (const g of grants) lines.push(`  { nodes = [${g.nodes.map((n) => `"${n}"`).join(', ')}], permissions = [${g.permissions.map((p) => `"${p}"`).join(', ')}] },`);
	lines.push(']');
	return lines.join('\n');
}

const RANK: Record<BuiltInRole, number> = { viewer: 0, operator: 1, developer: 2, admin: 3 };

/** For "highest role": built-in rank, with custom roles just above their base. */
export function roleRank(role: RoleDefinition | undefined): number {
	if (!role) return -1;
	if (role.source === 'built_in') return RANK[role.name as BuiltInRole] ?? 0;
	return (role.extends ? RANK[role.extends] : 0) + 0.5;
}

/** Which of the role's actions (with selectors) the account doesn't hold in the scope. */
export function missingForGrant(data: AccessData, principal: string, role: RoleDefinition, locationIds: string[]): { action: Capability; selector: string | null; locationId: string }[] {
	const missing: { action: Capability; selector: string | null; locationId: string }[] = [];
	for (const locationId of locationIds) {
		if (!roleAvailableAt(role, locationId)) continue;
		for (const g of resolvedGrants(role)) {
			for (const action of g.permissions) {
				if (!allows(data, { principal, action, locationId, subject: g.selector })) missing.push({ action, selector: g.selector, locationId });
			}
		}
	}
	return missing;
}

export function plainAction(action: Capability): string {
	return label(action);
}
