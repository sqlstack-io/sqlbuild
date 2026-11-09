// Pure helpers for the location detail page.

import type { LocationInstance } from '$lib/api';

export type ProcessSummary = {
	/** Live processes, newest first. */
	live: LocationInstance[];
	/** The processes to list below the header line; empty when exactly one is running. */
	listed: LocationInstance[];
};

/**
 * One running process is the normal case and fits on the header line. Show a
 * list when none is running (the recent ones, to see when it stopped) or more
 * than one is (usually a deploy overlap while a new process loads).
 */
export function summariseProcesses(instances: readonly LocationInstance[]): ProcessSummary {
	const byStart = [...instances].sort((a, b) => (b.startedAt ?? b.lastSeenAt).localeCompare(a.startedAt ?? a.lastSeenAt));
	const live = byStart.filter((i) => i.state === 'live');
	if (live.length === 1) return { live, listed: [] };
	return { live, listed: live.length > 1 ? live : byStart };
}

/** The first line of a load error, for a one-line summary. */
export function firstLine(text: string): string {
	return text.split('\n', 1)[0].trim();
}
