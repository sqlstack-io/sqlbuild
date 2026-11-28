// The Connections page's facts about checks, tables and sync. Pure.

import type { ConnectionTest, WarehouseConnection } from '$lib/api';
import { formatCount, lastFullSync } from '$lib/platform/connections';
import { testOrigin } from '$lib/platform/labels';
import { formatAgo, formatIn, formatStamp } from '$lib/format/time';

export type Table = WarehouseConnection['tables'][number];

/** e.g. `WAFFLE_SHOP.sqlbuild`. */
export const where = (s: { database: string | null; schema: string }) => (s.database ? `${s.database}.${s.schema}` : s.schema);

/** e.g. "waffle-shop · prod". */
export const targetName = (c: Pick<WarehouseConnection, 'location' | 'target'>) => `${c.location} · ${c.target}`;

/** e.g. "connected 3h ago", in the words every page uses. */
export const checkWords = (t: ConnectionTest, now: number) => `${t.outcome === 'ok' ? 'connected' : 'couldn’t connect'} ${formatAgo(t.at, now)}`;

/** e.g. "on deploy · 820 ms". */
export const checkOrigin = (t: ConnectionTest) => [testOrigin(t), t.durationMs > 0 ? `${t.durationMs.toLocaleString('en')} ms` : null].filter(Boolean).join(' · ');

/** The tooltip: exact time and the location's own message. */
export const checkTitle = (t: ConnectionTest) => `${formatStamp(t.at)}\n${t.message}`;

/** A table whose newest sync failed or waits for a build. */
export const isProblem = (t: Table) => !!t.failure;
export const failing = (tables: readonly Table[]) => tables.filter((t) => t.failure && !t.waiting).length;

/** Problems first, then by name. */
export const bySeverity = (tables: readonly Table[]) =>
	[...tables].sort((a, b) => Number(isProblem(b)) - Number(isProblem(a)) || Number(!b.waiting) - Number(!a.waiting) || a.table.localeCompare(b.table));

/** The oldest newest-sync among tables that synced cleanly, e.g. for "13 synced 4m ago". */
export function oldestClean(tables: readonly Table[]): string | null {
	const ok = tables.filter((t) => !t.failure).map((t) => t.at);
	return ok.length ? ok.reduce((a, b) => (Date.parse(a) <= Date.parse(b) ? a : b)) : null;
}

/** Records mirrored in total; null when any table has not reported. */
export function mirrored(tables: readonly Table[]): string | null {
	if (!tables.length || tables.some((t) => t.mirroredTotal === null)) return null;
	return formatCount(tables.reduce((sum, t) => sum + t.mirroredTotal!, 0));
}

export type Fact = { label: string; value: string; title?: string; tone?: string; help?: string };

/** The sync's facts as key-value rows, one fact each. */
export function syncFacts(c: WarehouseConnection, now: number): Fact[] {
	const full = lastFullSync(c.tables);
	const total = mirrored(c.tables);
	const facts: (Fact | null)[] = [
		{ label: 'Last full sync', value: full ? formatAgo(full, now) : 'None yet', title: full ? formatStamp(full) : undefined, help: 'When every table last synced cleanly' },
		total ? { label: 'Records mirrored', value: total } : null,
		c.halted
			? { label: 'Next sync', value: c.nextSyncAt ? `Stopped · retry ${formatIn(c.nextSyncAt, now)}` : 'Stopped', tone: 'var(--sb-error)', help: 'Failed reads in a row stopped the sync. It resumes at the retry time, or after the next run, a sync request or a deploy.' }
			: c.nextSyncAt
				? { label: 'Next sync', value: formatIn(c.nextSyncAt, now), title: formatStamp(c.nextSyncAt) }
				: null
	];
	return facts.filter((f): f is Fact => !!f);
}
