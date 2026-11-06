import type { RunRow, RunTab } from '$lib/api';

export type RunsView = 'list' | 'timeline' | 'queue';

export type RangePreset = 'any' | '24h' | '7d' | '30d' | 'custom';

/** Everything the list shows, as kept in the URL. */
export type ListParams = {
	view: RunsView;
	tab: RunTab;
	/** The search box: filter tokens plus free text. */
	q: string;
	range: RangePreset;
	/** `YYYY-MM-DD`, for the custom range. */
	from: string;
	to: string;
	/** Show group children as their own rows. */
	children: boolean;
};

export const TOKEN_KEYS = ['kind', 'origin', 'status', 'job', 'schedule', 'sensor', 'node', 'id', 'by'] as const;
export type TokenKey = (typeof TOKEN_KEYS)[number];
export type FilterToken = { key: TokenKey; value: string };

export type StatusTone = 'muted' | 'warn' | 'error' | 'live';
/** The plain second line under a run's status pill. */
export type StatusLine = { text: string; tone: StatusTone; title?: string };

export type BulkAction = 'cancel' | 'rerun' | 'from_failure';

/** A row in the virtualised list: a run, or one of a group's children shown inline. */
export type DisplayRow = { key: string; row: RunRow; child: boolean };

/** The list's live-update indicator. */
export type Freshness = 'live' | 'reconnecting' | 'stale';
