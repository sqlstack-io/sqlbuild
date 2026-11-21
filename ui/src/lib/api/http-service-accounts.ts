import type { components } from '$control/schema.d.ts';
import type { IssuedToken, ServiceAccount, ServiceAccountKey } from './types/platform';

type Wire = components['schemas'];

function toKey(wire: Wire['ServiceKeySummary']): ServiceAccountKey {
	return {
		id: wire.key_id,
		prefix: wire.prefix,
		createdAt: wire.created_at,
		createdBy: wire.created_by ?? null,
		lastUsedAt: wire.last_used_at ?? null,
		expiresAt: wire.expires_at ?? null,
		origin: wire.origin
	};
}

/** A service account as the UI shows it; the server lists its working keys, the current one first. */
export function toServiceAccount(wire: Wire['ServiceAccountSummary']): ServiceAccount {
	return {
		kind: 'service',
		name: wire.name,
		displayName: wire.name,
		description: wire.description ?? null,
		role: wire.role ?? null,
		createdAt: wire.created_at,
		createdBy: wire.created_by ?? null,
		lastUsedAt: wire.last_used_at ?? null,
		disabled: wire.disabled_at != null,
		keys: wire.keys.map(toKey)
	};
}

/** A key the server returns only this once, with the account it belongs to. */
export function toIssuedKey(wire: Wire['IssuedServiceKey']): IssuedToken<ServiceAccount> {
	return { token: toServiceAccount(wire.account), secret: wire.key };
}
