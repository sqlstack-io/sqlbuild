import type { components } from '$control/schema.d.ts';
import type { HealthComponentDetails, ServerHealth } from './types/platform';

type Wire = components['schemas'];

/** The server's health report in the UI's shape. */
export function toServerHealth(wire: Wire['HealthReport']): ServerHealth {
	return {
		status: wire.status,
		checkedAt: wire.checked_at,
		retrySeconds: wire.retry_seconds,
		server: { version: wire.server.version, startedAt: wire.server.started_at, uptimeSeconds: wire.server.uptime_seconds },
		components: wire.components.map((c) => ({
			component: c.component,
			name: c.name,
			locationId: c.location_id ?? null,
			status: c.status,
			since: c.since ?? null,
			reason: c.reason ?? null,
			case: c.case ?? null,
			lastSuccess: c.last_success ?? null,
			lastFailure: c.last_failure ?? null
		}))
	};
}

/** Administrators' health details in the UI's shape. */
export function toHealthDetails(wire: Wire['HealthDetails']): HealthComponentDetails[] {
	return wire.components.map((c) => ({ component: c.component, locationId: c.location_id ?? null, host: c.host ?? null, errorChain: c.error_chain, requestIds: c.request_ids }));
}
