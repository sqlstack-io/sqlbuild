import { describe, expect, it } from 'vitest';
import { toIssuedKey, toServiceAccount } from './http-service-accounts';

const key = (prefix: string, expires_at: string | null = null) => ({
	key_id: `k-${prefix}`,
	prefix,
	origin: 'issued' as const,
	created_at: '2026-11-01T00:00:00Z',
	created_by: null,
	last_used_at: null,
	expires_at
});

const account = {
	principal_id: 'p-orders-deploy',
	name: 'orders-deploy',
	description: null,
	role: null,
	created_at: '2026-11-01T00:00:00Z',
	created_by: 'ada',
	disabled_at: '2026-11-02T00:00:00Z',
	last_used_at: null,
	keys: [key('sqbk_Q7m2xT'), key('sqbk_b81LkD', '2026-11-21T12:00:00Z')]
};

describe('service accounts', () => {
	it('keeps the server order of keys, a missing role and who made it, and reads disabled from its time', () => {
		const shown = toServiceAccount(account);
		expect([shown.kind, shown.role, shown.createdBy, shown.disabled]).toEqual(['service', null, 'ada', true]);
		expect(shown.keys.map((k) => [k.prefix, k.expiresAt, k.createdBy])).toEqual([
			['sqbk_Q7m2xT', null, null],
			['sqbk_b81LkD', '2026-11-21T12:00:00Z', null]
		]);
	});

	it('returns the key that is shown only once with its account', () => {
		const issued = toIssuedKey({ account, key: 'sqbk_secret' });
		expect([issued.secret, issued.token.name]).toEqual(['sqbk_secret', 'orders-deploy']);
	});
});
