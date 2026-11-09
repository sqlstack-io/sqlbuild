import { describe, expect, it } from 'vitest';
import { toAccount, toRoleAssignments, toServerSettings } from './http-people';

const person = (name: string, groups: string[], has_password = false) => ({
	principal_id: `p-${name}`,
	name,
	created_at: '2026-11-01T00:00:00Z',
	disabled_at: null,
	has_password,
	groups,
	active_sessions: 0,
	last_signed_in_at: null
});

describe('people', () => {
	it('expands a group role to its members only, and scopes it to the location key', () => {
		const people = [person('ada', ['data-admins']), person('vic', ['analysts']), person('sam', [], true)];
		const assignments = toRoleAssignments(
			{
				assignments: [{ assignment_id: 'a-1', principal_id: 'p-sam', name: 'sam', role_key: 'viewer', location_id: 'loc-1', target: 'prod' }],
				group_roles: [{ group_name: 'data-admins', role_key: 'admin', location_id: null, target: null }]
			},
			people
		);
		expect(assignments.map((a) => [a.principal.name, a.role, a.source, a.ssoGroup, a.scope.locationId])).toEqual([
			['sam', 'viewer', 'manual', null, 'loc-1:prod'],
			['ada', 'admin', 'sso', 'data-admins', null]
		]);
	});

	it('reports a sign-in source only when the server knows one', () => {
		expect([toAccount(person('sam', [], true)).source, toAccount(person('ada', ['g'])).source, toAccount(person('vic', [])).source]).toEqual([
			'password',
			'oidc',
			null
		]);
	});
});

describe('toServerSettings', () => {
	it('names each kept record with its own retention', () => {
		const settings = toServerSettings({
			version: '0.205.1',
			login_mode: 'password',
			scheduler: false,
			installation: { storage: 'sqlite', installed_at: '2026-11-01T00:00:00Z', schema_checksum: 'abc', schema_applied_at: '2026-11-01T00:00:00Z' },
			retention: { audit_days: 400, snapshots_kept: 10, statistics_days: 90, platform_alert_days: 30, location_health_days: 7, sensor_evaluation_days: 6, heartbeat_days: 1 }
		});
		expect([settings.authMode, settings.schedulerEnabled, settings.storage]).toEqual(['password', false, 'sqlite']);
		expect(settings.retention.map((r) => [r.name, r.value])).toEqual([
			['Security audit log', '400 days'],
			['Published versions', '10'],
			['Warehouse row counts', '90 days'],
			['Platform problems', '30 days'],
			['Location check-ins', '7 days'],
			['Sensor checks', '6 days'],
			['Server heartbeats', '1 day']
		]);
	});
});
