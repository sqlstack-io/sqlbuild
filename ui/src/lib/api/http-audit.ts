import type { components } from '$control/schema.d.ts';
import { locationKey } from './http-client';
import type { AuditEntry, AuditQuery } from './types/platform';

type Wire = components['schemas'];

const DENIED = 'authorization.denied';

function fieldsOf(value: unknown): Record<string, unknown> {
	return value && typeof value === 'object' && !Array.isArray(value) ? (value as Record<string, unknown>) : {};
}

function textOf(value: unknown): string | null {
	return typeof value === 'string' && value !== '' ? value : null;
}

function jsonText(value: unknown): string | null {
	return value === null || value === undefined ? null : JSON.stringify(value);
}

/** The scope's location in UI terms, and its remaining fields as the subject. */
function scopeOf(scope: Record<string, unknown>): AuditEntry['scope'] {
	const { location_id: location, target, action: _action, kind: _kind, ...rest } = scope;
	const locationId = textOf(location);
	const subject = Object.entries(rest)
		.filter(([, value]) => value !== null && !(Array.isArray(value) && value.length === 0))
		.map(([key, value]) => `${key}=${typeof value === 'string' ? value : JSON.stringify(value)}`)
		.join(' ');
	return {
		locationId: locationId && textOf(target) ? locationKey(locationId, target as string) : locationId,
		subject: subject || null
	};
}

/** A denial is listed under the action it refused, with its decision ID and reason. */
export function toAuditEntry(wire: Wire['SecurityAuditEvent']): AuditEntry {
	const denied = wire.action === DENIED;
	const scope = fieldsOf(wire.scope_json);
	const after = fieldsOf(wire.after_json);
	const name = wire.actor_name ?? wire.actor;
	return {
		id: wire.event_id,
		at: wire.occurred_at,
		actor: { kind: wire.actor_name ? 'user' : 'system', name, displayName: name },
		action: denied ? (textOf(scope.action) ?? DENIED) : wire.action,
		outcome: denied ? 'denied' : 'allowed',
		scope: scopeOf(scope),
		grantingRole: denied ? null : wire.granting_role,
		decisionId: denied ? textOf(after.decision_id) : null,
		evidenceRead: wire.action === 'evidence.read',
		summary: denied ? (textOf(after.reason) ?? '') : '',
		before: jsonText(wire.before_json),
		after: denied ? null : jsonText(wire.after_json),
		ip: wire.ip_address ?? '',
		requestId: wire.request_id
	};
}

export function auditMatches(entry: AuditEntry, query: AuditQuery): boolean {
	return (
		(!query.actor || entry.actor.name === query.actor) &&
		(!query.action || entry.action === query.action) &&
		(!query.outcome || entry.outcome === query.outcome) &&
		(!query.evidenceOnly || entry.evidenceRead)
	);
}
