import { describe, expect, it } from 'vitest';
import { roleDetailOf, toRoleDefinition } from './http-roles';
import type { RoleAssignment } from './types/platform';

const support = {
	key: 'support',
	builtin: false,
	description: 'Answer order questions',
	grants: [{ nodes: ['tag:orders'], permissions: ['view' as const, 'view.sql' as const], matched: { 'tag:orders': ['customers', 'orders'] } }],
	holders: 2
};
const assignment = (id: string, role: string, locationId: string | null): RoleAssignment => ({
	id,
	principal: { kind: 'user', name: id, displayName: id },
	role,
	scope: { locationId },
	source: 'manual',
	ssoGroup: null,
	definedIn: null,
	assignedBy: null,
	assignedAt: null,
	stale: false
});

describe('toRoleDefinition', () => {
	it('describes built-in roles itself and keeps a custom role published description', () => {
		const viewer = toRoleDefinition({ key: 'viewer', builtin: true, grants: [{ nodes: [], permissions: ['view'], matched: {} }], holders: 0 });
		expect(viewer).toMatchObject({ source: 'built_in', extends: null, recentlyMatched: null });
		expect(viewer.description).toContain('Sees');
		expect(toRoleDefinition(support)).toMatchObject({ source: 'project', description: 'Answer order questions', holders: 2 });
	});
});

describe('roleDetailOf', () => {
	it('lists assignments everywhere, on the whole location, or on this target only', () => {
		const detail = roleDetailOf(
			support,
			[
				assignment('everywhere', 'support', null),
				assignment('location', 'support', 'l1'),
				assignment('target', 'support', 'l1:development'),
				assignment('other-target', 'support', 'l1:production'),
				assignment('other-location', 'support', 'l2'),
				assignment('other-role', 'viewer', null)
			],
			'l1:development'
		);
		expect(detail.assignments.map((a) => a.id)).toEqual(['everywhere', 'location', 'target']);
		expect(detail.selectors).toEqual([{ selector: 'tag:orders', nodes: ['customers', 'orders'], newSinceLastWeek: [] }]);
		expect(JSON.parse(detail.definition ?? '')).toEqual({ description: 'Answer order questions', grants: [{ nodes: ['tag:orders'], permissions: ['view', 'view.sql'] }] });
	});

	it('has no definition text for built-in roles', () => {
		expect(roleDetailOf({ key: 'admin', builtin: true, grants: [], holders: 1 }, [], 'l1:development').definition).toBeNull();
	});
});
