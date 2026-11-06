// The list's URL parameters, read and written in one place. Pure.

import { RUN_TABS, type RunRowQuery, type RunTab } from '$lib/api';
import type { ListParams, RangePreset, RunsView } from '../types';
import { toFilter } from './filter-tokens';

const VIEWS: readonly RunsView[] = ['list', 'timeline', 'queue'];
const RANGES: readonly RangePreset[] = ['any', '24h', '7d', '30d', 'custom'];

export function readParams(search: URLSearchParams): ListParams {
	const view = search.get('view');
	const tab = search.get('tab');
	const range = search.get('range');
	return {
		view: VIEWS.find((v) => v === view) ?? 'list',
		tab: RUN_TABS.find((t) => t === tab) ?? 'all',
		q: search.get('q') ?? '',
		range: RANGES.find((r) => r === range) ?? 'any',
		from: search.get('from') ?? '',
		to: search.get('to') ?? '',
		children: search.get('children') === '1'
	};
}

/** Writes non-default values only, so a plain `/runs` stays clean. */
export function writeParams(url: URL, params: ListParams): URL {
	const next = new URL(url);
	const set = (key: string, value: string, fallback: string) => {
		if (value && value !== fallback) next.searchParams.set(key, value);
		else next.searchParams.delete(key);
	};
	set('view', params.view, 'list');
	set('tab', params.tab, 'all');
	set('q', params.q.trim(), '');
	set('range', params.range, 'any');
	set('from', params.range === 'custom' ? params.from : '', '');
	set('to', params.range === 'custom' ? params.to : '', '');
	set('children', params.children ? '1' : '', '');
	return next;
}

const HOURS: Record<Exclude<RangePreset, 'any' | 'custom'>, number> = { '24h': 24, '7d': 168, '30d': 720 };

/** The server query for the current parameters (no cursor). */
export function toQuery(params: ListParams, now: number, tab: RunTab = params.tab): RunRowQuery {
	let since: string | undefined;
	let until: string | undefined;
	if (params.range === 'custom') {
		if (params.from) since = new Date(`${params.from}T00:00:00`).toISOString();
		if (params.to) until = new Date(Date.parse(`${params.to}T00:00:00`) + 86_400_000).toISOString();
	} else if (params.range !== 'any') {
		since = new Date(now - HOURS[params.range] * 3_600_000).toISOString();
	}
	return { tab, filter: toFilter(params.q), since, until, includeChildren: params.children };
}
