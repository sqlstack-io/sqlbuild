// Search tokens for the runs list, e.g. `job:core_build status:failed orders`.
// Pure: parses the box into a structured filter and edits the token string.

import { RUN_KINDS, RUN_ORIGINS, RUN_STATUSES, type RunFilter, type RunKind, type RunOrigin, type RunStatus } from '$lib/api';
import { TOKEN_KEYS, type FilterToken, type TokenKey } from '../types';

const TOKEN_RE = /^([a-z]+):(.+)$/;

/** Splits the search text into recognised `key:value` tokens and the remaining free text. */
export function parseTokens(q: string): { tokens: FilterToken[]; text: string } {
	const tokens: FilterToken[] = [];
	const text: string[] = [];
	for (const part of q.trim().split(/\s+/).filter(Boolean)) {
		const m = part.match(TOKEN_RE);
		const key = m?.[1] as TokenKey | undefined;
		if (m && key && (TOKEN_KEYS as readonly string[]).includes(key)) {
			for (const value of m[2].split(',').filter(Boolean)) tokens.push({ key, value });
		} else {
			text.push(part);
		}
	}
	return { tokens, text: text.join(' ') };
}

const pick = <T extends string>(values: string[], allowed: readonly T[]): T[] =>
	values.map((v) => v.toLowerCase()).filter((v): v is T => (allowed as readonly string[]).includes(v));

/** The structured filter the server takes. Unknown status, kind or origin values are dropped. */
export function toFilter(q: string): RunFilter {
	const { tokens, text } = parseTokens(q);
	const values = (key: TokenKey) => tokens.filter((t) => t.key === key).map((t) => t.value);
	const statuses = values('status').flatMap((v) => (v === 'lost' ? ['presumed_failed'] : [v]));
	const filter: RunFilter = {
		kinds: pick<RunKind>(values('kind').map((v) => (v === 'ad-hoc' ? 'adhoc' : v)), RUN_KINDS),
		origins: pick<RunOrigin>(values('origin'), RUN_ORIGINS),
		statuses: pick<RunStatus>(statuses, RUN_STATUSES),
		jobKeys: values('job'),
		schedules: values('schedule'),
		sensors: values('sensor'),
		nodeKeys: values('node'),
		ids: values('id'),
		requestedBy: values('by'),
		text: text || undefined
	};
	return Object.fromEntries(Object.entries(filter).filter(([, v]) => (Array.isArray(v) ? v.length : v))) as RunFilter;
}

/** Tokens that don't narrow anything, e.g. `status:bogus`, so the UI can flag them. */
export function invalidTokens(q: string): FilterToken[] {
	const allowed: Partial<Record<TokenKey, readonly string[]>> = {
		kind: [...RUN_KINDS, 'ad-hoc'],
		origin: RUN_ORIGINS,
		status: [...RUN_STATUSES, 'lost']
	};
	return parseTokens(q).tokens.filter((t) => allowed[t.key] && !allowed[t.key]!.includes(t.value.toLowerCase()));
}

/** Adds `key:value` unless an equal token is already there. */
export function addToken(q: string, key: TokenKey, value: string): string {
	const { tokens } = parseTokens(q);
	if (tokens.some((t) => t.key === key && t.value.toLowerCase() === value.toLowerCase())) return q;
	return `${q.trim()} ${key}:${value}`.trim();
}

/** Removes one `key:value`, also from comma lists like `status:failed,partial`. */
export function removeToken(q: string, key: TokenKey, value: string): string {
	return q
		.trim()
		.split(/\s+/)
		.filter(Boolean)
		.map((part) => {
			const m = part.match(TOKEN_RE);
			if (!m || m[1] !== key) return part;
			const rest = m[2].split(',').filter((v) => v.toLowerCase() !== value.toLowerCase());
			return rest.length ? `${key}:${rest.join(',')}` : '';
		})
		.filter(Boolean)
		.join(' ');
}

export const TOKEN_HELP: Record<TokenKey, string> = {
	kind: 'kind:job, kind:group or kind:adhoc',
	origin: 'origin:manual, origin:schedule, origin:sensor or origin:rerun',
	status: 'status:running, status:failed, status:lost …',
	job: 'job:<job or group name>',
	schedule: 'schedule:<schedule name>',
	sensor: 'sensor:<sensor name>',
	node: 'node:<model or source>: runs that build it',
	id: 'id:<run ID or its start>',
	by: 'by:<person>'
};
