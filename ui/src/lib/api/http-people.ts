import type { components } from '$control/schema.d.ts';
import type { Account, RoleAssignment, ServerSettings } from './types/platform';

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

const days = (n: number) => `${n} ${n === 1 ? 'day' : 'days'}`;

/** The server's own facts, with each kind of record and how long the server keeps it. */
export function toServerSettings(wire: Wire['ServerSettings']): ServerSettings {
	const r = wire.retention;
	return {
		serverVersion: wire.version,
		storage: wire.installation.storage,
		installedAt: wire.installation.installed_at,
		schemaChecksum: wire.installation.schema_checksum,
		schemaAppliedAt: wire.installation.schema_applied_at,
		storeBytes: wire.installation.store_bytes,
		authMode: wire.login_mode,
		schedulerEnabled: wire.scheduler,
		retention: [
			{ name: 'Security audit log', value: days(r.audit_days), description: 'Who did what, and every refusal.' },
			{ name: 'Run logs', value: days(r.run_log_days), description: 'What finished runs logged. The runs themselves are kept.' },
			{ name: 'Logged runs', value: String(r.run_logs_kept), description: 'Newest finished runs per location target that keep their logs.' },
			{ name: 'Published versions', value: String(r.snapshots_kept), description: 'Kept for each location target; older versions are removed.' },
			{ name: 'Warehouse row counts', value: days(r.statistics_days), description: 'Row counts and sizes sampled from the warehouse.' },
			{ name: 'Platform problems', value: days(r.platform_alert_days), description: 'Late schedules, failed launches and locations that stopped checking in.' },
			{ name: 'Location check-ins', value: days(r.location_health_days), description: 'Health reports from location processes.' },
			{ name: 'Sensor checks', value: days(r.sensor_evaluation_days), description: 'Each sensor check. Where each sensor is up to is always kept.' },
			{ name: 'Server heartbeats', value: days(r.heartbeat_days), description: 'Check-ins from the servers themselves.' }
		]
	};
}
