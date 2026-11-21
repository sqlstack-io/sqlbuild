import type { AuditEntry, Principal, ServiceAccount, ServiceAccountKey } from '../types';
import { LOCATION_SHOP, LOCATION_SHOP_STAGING } from './fixtures';
import { ALEX, DEMO_USER } from './principals';

const MIN = 60_000;
const DAY = 24 * 60 * MIN;
const KEY_ALPHABET = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
/** Characters of a key after `sqbk_`; its prefix shows the first `PREFIX_CHARS` of them. */
const KEY_CHARS = 40;
const PREFIX_CHARS = 6;

/** The deploy script that republishes the waffle-shop project after each release. */
export const ORDERS_DEPLOY: Principal = { kind: 'service', name: 'orders-deploy', displayName: 'orders-deploy' };
export const CI_RUNNER: Principal = { kind: 'service', name: 'ci-runner', displayName: 'ci-runner' };
export const REPORT_EXPORT: Principal = { kind: 'service', name: 'report-export', displayName: 'report-export' };

/** A new key: the secret, shown once, and its non-secret prefix. */
export function newServiceKey(): { secret: string; prefix: string } {
	const bytes = new Uint8Array(KEY_CHARS);
	crypto.getRandomValues(bytes);
	const body = [...bytes].map((b) => KEY_ALPHABET[b % KEY_ALPHABET.length]).join('');
	return { secret: `sqbk_${body}`, prefix: `sqbk_${body.slice(0, PREFIX_CHARS)}` };
}

/**
 * Three service accounts: `orders-deploy` part-way through a rotation, whose old key still works for
 * 50 minutes and was used after the new one was issued; `ci-runner` with a key imported with the CLI;
 * and a disabled `report-export`. `none` has none yet.
 */
export function serviceAccountFixtures(now: number, which: 'default' | 'none'): ServiceAccount[] {
	if (which === 'none') return [];
	const ago = (minutes: number) => new Date(now - minutes * MIN).toISOString();
	const ahead = (minutes: number) => new Date(now + minutes * MIN).toISOString();
	const key = (id: string, prefix: string, createdMinutesAgo: number, usedMinutesAgo: number | null, extra: Partial<ServiceAccountKey> = {}): ServiceAccountKey => ({
		id,
		prefix,
		createdAt: ago(createdMinutesAgo),
		createdBy: DEMO_USER,
		lastUsedAt: usedMinutesAgo === null ? null : ago(usedMinutesAgo),
		expiresAt: null,
		origin: 'issued',
		...extra
	});
	return [
		{
			...ORDERS_DEPLOY,
			kind: 'service',
			description: 'Release pipeline: republishes waffle-shop after each deploy',
			role: 'developer',
			createdAt: ago((90 * DAY) / MIN),
			createdBy: DEMO_USER,
			lastUsedAt: ago(3),
			disabled: false,
			keys: [key('sak-4', 'sqbk_Q7m2xT', 10, null), key('sak-3', 'sqbk_b81LkD', (60 * DAY) / MIN, 3, { expiresAt: ahead(50) })]
		},
		{
			...CI_RUNNER,
			kind: 'service',
			description: 'Runs checks_only on every pull request',
			role: 'operator',
			createdAt: ago((30 * DAY) / MIN),
			createdBy: ALEX,
			lastUsedAt: ago(125),
			disabled: false,
			keys: [key('sak-2', 'sqbk_Zp0aW9', (30 * DAY) / MIN, 125, { createdBy: ALEX, origin: 'imported' })]
		},
		{
			...REPORT_EXPORT,
			kind: 'service',
			description: 'Old nightly export, replaced by the reporting job',
			role: 'viewer',
			createdAt: ago((200 * DAY) / MIN),
			createdBy: DEMO_USER,
			lastUsedAt: ago((41 * DAY) / MIN),
			disabled: true,
			keys: [key('sak-1', 'sqbk_m3TfQe', (200 * DAY) / MIN, (41 * DAY) / MIN)]
		}
	];
}

/** What the service accounts did lately: republishes by the deploy script (one refused), checks by CI, and the rotation now in its overlap. */
export function serviceAccountAudit(now: number): AuditEntry[] {
	const ago = (minutes: number) => new Date(now - minutes * MIN).toISOString();
	let seq = 0;
	const entry = (minutesAgo: number, e: Pick<AuditEntry, 'actor' | 'action' | 'scope' | 'summary'> & Partial<AuditEntry>): AuditEntry => {
		seq += 1;
		return { id: `aud-sa-${seq}`, at: ago(minutesAgo), outcome: 'allowed', grantingRole: null, decisionId: null, evidenceRead: false, before: null, after: null, ip: '10.0.2.14', requestId: `req-sa-${seq}`, ...e };
	};
	return [
		entry(3, { actor: ORDERS_DEPLOY, action: 'snapshot.republished', scope: { locationId: LOCATION_SHOP_STAGING, subject: 'waffle-shop/staging' }, grantingRole: 'developer', summary: 'Republished waffle-shop/staging with the old key sqbk_b81LkD…', before: 'snapshot 412', after: 'snapshot 413' }),
		entry(10, { actor: DEMO_USER, action: 'service_account.key_rotated', scope: { locationId: null, subject: 'orders-deploy' }, grantingRole: 'admin', summary: "Rotated orders-deploy's key; sqbk_b81LkD… works for 60 more minutes", before: 'sqbk_b81LkD', after: 'sqbk_Q7m2xT', ip: '10.0.4.17' }),
		entry(125, { actor: CI_RUNNER, action: 'job.run', scope: { locationId: LOCATION_SHOP_STAGING, subject: 'checks_only' }, grantingRole: 'operator', summary: 'Launched checks_only', after: 'run-7d21 queued', ip: '10.0.2.8' }),
		entry(250, { actor: ORDERS_DEPLOY, action: 'snapshot.republished', scope: { locationId: LOCATION_SHOP, subject: 'waffle-shop/prod' }, grantingRole: 'developer', summary: 'Republished waffle-shop/prod', before: 'snapshot 287', after: 'snapshot 288' }),
		entry(252, { actor: ORDERS_DEPLOY, action: 'location.manage', outcome: 'denied', decisionId: 'dec-9s4d1k', scope: { locationId: LOCATION_SHOP, subject: 'waffle-shop' }, summary: 'Denied: developer can’t change a location’s settings', ip: '10.0.2.14' }),
		entry(1440 + 30, { actor: ORDERS_DEPLOY, action: 'snapshot.republished', scope: { locationId: LOCATION_SHOP_STAGING, subject: 'waffle-shop/staging' }, grantingRole: 'developer', summary: 'Republished waffle-shop/staging', before: 'snapshot 411', after: 'snapshot 412' }),
		entry(41 * 1440, { actor: REPORT_EXPORT, action: 'view', scope: { locationId: LOCATION_SHOP, subject: 'fact_orders' }, grantingRole: 'viewer', summary: 'Read fact_orders for the nightly export' }),
		entry(40 * 1440, { actor: DEMO_USER, action: 'service_account.disabled', scope: { locationId: null, subject: 'report-export' }, grantingRole: 'admin', summary: 'Disabled service account report-export', before: 'active', after: 'disabled', ip: '10.0.4.17' })
	];
}
