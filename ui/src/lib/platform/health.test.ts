import { describe, expect, it } from 'vitest';
import type { HealthComponent, ServerHealth } from '$lib/api';
import { degradedLine, degradedParts, outageOf, shellHealth } from './health';

const part = (component: HealthComponent['component'], name: string, status: HealthComponent['status'], reason: string | null = null): HealthComponent => ({
	component,
	name,
	locationId: null,
	status,
	since: '2026-11-25T17:42:00Z',
	reason,
	case: null,
	lastSuccess: null,
	lastFailure: null
});
const report = (status: ServerHealth['status'], components: HealthComponent[]): ServerHealth => ({
	status,
	checkedAt: '2026-11-25T18:00:00Z',
	retrySeconds: 10,
	server: { version: '0.262.0', startedAt: '2026-11-25T17:00:00Z', uptimeSeconds: 3600 },
	components
});
const DNS = "Can't reach the control database: db.internal doesn't resolve (DNS)";

describe('shell health', () => {
	it('reads Live, Degraded or Unavailable from the report, and Unavailable when the server does not answer', () => {
		expect({
			healthy: shellHealth(report('healthy', []), null),
			degraded: shellHealth(report('degraded', []), null),
			unavailable: shellHealth(report('unavailable', []), null),
			unread: shellHealth(null, null),
			unreachable: shellHealth(report('healthy', []), { reason: "Can't reach the SQLBuild server", since: null })
		}).toEqual({ healthy: 'live', degraded: 'degraded', unavailable: 'unavailable', unread: null, unreachable: 'unavailable' });
	});

	it('names the control database as the outage while it is unavailable', () => {
		const down = report('unavailable', [part('server', 'Server', 'healthy'), part('control_database', 'Control database', 'unavailable', DNS)]);
		expect({ down: outageOf(down, null), up: outageOf(report('degraded', [part('control_database', 'Control database', 'healthy')]), null) }).toEqual({
			down: { reason: DNS, since: '2026-11-25T17:42:00Z' },
			up: null
		});
	});

	it('lists the degraded parts and names the first in the banner', () => {
		const degraded = report('degraded', [part('server', 'Server', 'healthy'), part('scheduler', 'Scheduler', 'degraded', 'Starting'), part('location', 'orders', 'degraded', "Hasn't reported since 17:40")]);
		expect({ parts: degradedParts(degraded).map((p) => p.name), line: degradedLine(degradedParts(degraded)), none: degradedLine([]) }).toEqual({
			parts: ['Scheduler', 'orders'],
			line: 'Scheduler: Starting (and 1 more)',
			none: null
		});
	});
});
