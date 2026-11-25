import { describe, expect, it } from 'vitest';
import { toHealthDetails, toServerHealth } from './http-health';

describe('health mapping', () => {
	it('keeps each component with its state, since when, reason and case', () => {
		expect(
			toServerHealth({
				status: 'unavailable',
				checked_at: '2026-11-25T18:00:10Z',
				retry_seconds: 10,
				server: { version: '0.262.0', started_at: '2026-11-25T17:00:00Z', uptime_seconds: 3610 },
				components: [
					{ component: 'server', name: 'Server', status: 'healthy', since: '2026-11-25T17:00:00Z', reason: null, last_success: '2026-11-25T18:00:10Z', last_failure: null },
					{
						component: 'control_database',
						name: 'Control database',
						status: 'unavailable',
						since: '2026-11-25T17:42:00Z',
						reason: "Can't reach the control database: db.internal doesn't resolve (DNS)",
						case: 'name_lookup',
						last_success: '2026-11-25T17:41:55Z',
						last_failure: '2026-11-25T18:00:05Z'
					},
					{ component: 'location', name: 'orders', location_id: 'loc-1', status: 'degraded', since: '2026-11-25T17:40:00Z', reason: "Hasn't reported since 2026-11-25 17:40 UTC", last_success: '2026-11-25T17:40:00Z', last_failure: null }
				]
			})
		).toEqual({
			status: 'unavailable',
			checkedAt: '2026-11-25T18:00:10Z',
			retrySeconds: 10,
			server: { version: '0.262.0', startedAt: '2026-11-25T17:00:00Z', uptimeSeconds: 3610 },
			components: [
				{ component: 'server', name: 'Server', locationId: null, status: 'healthy', since: '2026-11-25T17:00:00Z', reason: null, case: null, lastSuccess: '2026-11-25T18:00:10Z', lastFailure: null },
				{
					component: 'control_database',
					name: 'Control database',
					locationId: null,
					status: 'unavailable',
					since: '2026-11-25T17:42:00Z',
					reason: "Can't reach the control database: db.internal doesn't resolve (DNS)",
					case: 'name_lookup',
					lastSuccess: '2026-11-25T17:41:55Z',
					lastFailure: '2026-11-25T18:00:05Z'
				},
				{ component: 'location', name: 'orders', locationId: 'loc-1', status: 'degraded', since: '2026-11-25T17:40:00Z', reason: "Hasn't reported since 2026-11-25 17:40 UTC", case: null, lastSuccess: '2026-11-25T17:40:00Z', lastFailure: null }
			]
		});
	});

	it('keeps where the database is, each cause and the failed requests for administrators', () => {
		expect(
			toHealthDetails({ components: [{ component: 'control_database', host: 'db.internal:5432', error_chain: ['failed to lookup address information'], request_ids: ['req-1'] }] })
		).toEqual([{ component: 'control_database', locationId: null, host: 'db.internal:5432', errorChain: ['failed to lookup address information'], requestIds: ['req-1'] }]);
	});
});
