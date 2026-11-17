// The Connections page: connection details, the last check, and the state history sync summary. Pure.

import type { ConnectionDetails, ConnectionTest, WarehouseConnection } from '$lib/api';
import { formatAgo, formatIn } from '$lib/format/time';
import { plural } from '$lib/format/plural';

const AUTH_LABEL: Record<string, string> = {
	key_pair: 'Key pair',
	password: 'Password',
	oauth: 'OAuth',
	external_browser: 'Browser sign-in',
	token: 'Token',
	none: 'None'
};

/** The sign-in method by name, e.g. "Key pair"; an unknown name is shown as it is. */
export const authMethodLabel = (method: string) => AUTH_LABEL[method] ?? method;

export type DetailRow = { label: string; value: string | null };

/**
 * The Connection card's detail rows, always in the same shape so the card keeps its layout before the
 * location publishes them; each value is null until reported.
 */
export function detailRows(details: ConnectionDetails | null): DetailRow[] {
	return [
		{ label: 'Adapter', value: details?.adapter ?? null },
		{ label: details?.account || !details?.host ? 'Account' : 'Host', value: details?.account ?? details?.host ?? null },
		{ label: 'User', value: details?.user ?? null },
		{ label: 'Role', value: details?.role ?? null },
		{ label: 'Warehouse', value: details?.warehouse ?? null },
		{ label: 'Default database', value: details?.database ?? null },
		{ label: 'Authentication', value: details?.authMethod ? authMethodLabel(details.authMethod) : null }
	];
}

const pad = (n: number) => String(n).padStart(2, '0');
const TRIGGER: Record<ConnectionTest['trigger'], string> = { startup: 'startup', publish: 'after deploy', manual: 'manual' };

/** e.g. "passed at 01:22 (startup) · 410 ms", in local time; a check with no recorded duration leaves it out. */
export function lastCheckLine(test: ConnectionTest): string {
	const d = new Date(test.at);
	const parts = [`${test.outcome === 'ok' ? 'passed' : 'failed'} at ${pad(d.getHours())}:${pad(d.getMinutes())} (${TRIGGER[test.trigger]})`];
	if (test.durationMs > 0) parts.push(`${test.durationMs.toLocaleString('en')} ms`);
	return parts.join(' · ');
}

/** e.g. 130k, 1.2M. */
export function formatCount(n: number): string {
	if (n < 1000) return String(n);
	if (n < 1_000_000) return `${n < 10_000 ? (n / 1000).toFixed(1).replace(/\.0$/, '') : Math.round(n / 1000)}k`;
	return `${(n / 1_000_000).toFixed(1).replace(/\.0$/, '')}M`;
}

/** When every table last synced cleanly: the oldest table's last clean sync; null while any table never has. */
export function lastFullSync(tables: WarehouseConnection['tables']): string | null {
	if (!tables.length || tables.some((t) => !t.lastSyncedAt)) return null;
	return tables.map((t) => t.lastSyncedAt!).reduce((a, b) => (Date.parse(a) <= Date.parse(b) ? a : b));
}

/**
 * The State history sync card's summary, e.g. "14 tables · last full sync 13m ago · 130k records
 * mirrored · next in 17m"; facts the server does not report are left out. A sync that failed reads
 * stopped says so, and when it resumes.
 */
export function syncSummary(c: Pick<WarehouseConnection, 'tables' | 'nextSyncAt' | 'halted'>, now: number): string {
	const parts = [`${plural(c.tables.length, 'table')}`];
	const full = lastFullSync(c.tables);
	parts.push(full ? `last full sync ${formatAgo(full, now)}` : 'no full sync yet');
	if (c.tables.length && c.tables.every((t) => t.mirroredTotal !== null)) {
		parts.push(`${formatCount(c.tables.reduce((sum, t) => sum + t.mirroredTotal!, 0))} records mirrored`);
	}
	if (c.halted) parts.push(c.nextSyncAt ? `stopped after failed reads, retrying ${formatIn(c.nextSyncAt, now)}` : 'stopped after failed reads until the next run or a sync request');
	else if (c.nextSyncAt) parts.push(`next ${formatIn(c.nextSyncAt, now)}`);
	return parts.join(' · ');
}

/** Tables whose newest sync failed, not counting a sync waiting for a build. */
export const failingTables = (tables: WarehouseConnection['tables']) => tables.filter((t) => t.failure && !t.waiting).length;

export type ConnectionGroup = { key: string; details: ConnectionDetails | null; users: WarehouseConnection[] };

/**
 * One Connection card per distinct published connection, listing the location targets that use it.
 * A target that has not published its details has a card of its own.
 */
export function groupConnections(list: readonly WarehouseConnection[]): ConnectionGroup[] {
	const groups = new Map<string, ConnectionGroup>();
	for (const c of list) {
		const d = c.details;
		const key = d ? JSON.stringify([d.adapter, d.account, d.host, d.user, d.role, d.warehouse, d.database, d.authMethod]) : `target:${c.id}`;
		const group = groups.get(key) ?? { key, details: d, users: [] };
		group.users.push(c);
		groups.set(key, group);
	}
	return [...groups.values()];
}
