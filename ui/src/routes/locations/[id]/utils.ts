// Pure helpers for the location detail page.

import type { LocationInstance } from '$lib/api';
import type { MissingProviderSecrets } from '$lib/api/types/platform';

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

/** "a", "a and b", "a, b and c". */
const andList = (parts: string[]) => (parts.length < 2 ? parts.join('') : `${parts.slice(0, -1).join(', ')} and ${parts.at(-1)}`);

/** A snapshot node key's name: `raw_payments` for `source:raw_payments`. */
export const nodeName = (key: string) => key.slice(key.indexOf(':') + 1);

/** A provider's unset variables: "ORDERS_API_KEY", or "A and B". */
export const variablesOf = (p: MissingProviderSecrets) => andList(p.settings.map((s) => s.variable));

/** The one-line summary of the providers missing secrets the target doesn't allow: "orders_api needs ORDERS_API_KEY". */
export function secretsSummary(problems: MissingProviderSecrets[]): string {
	return problems.map((p) => `${p.provider} needs ${variablesOf(p)}`).join(' · ');
}

/** What an allowed provider's missing secrets mean: its nodes still can't run here. */
export function allowedSecretsNote(p: MissingProviderSecrets): string {
	const nodes = p.nodes.length ? `; runs of ${andList(p.nodes.map(nodeName))} stop before the warehouse` : '';
	return `${p.provider} is missing ${variablesOf(p)}, which this target allows (allow_missing_secrets)${nodes}`;
}
