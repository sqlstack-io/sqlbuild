// Awkward data for layout tests: every list the mock returns is stretched to thousands of rows, and
// every name, description and selection made very long. Turned on with `?mock&awkward`; the layout
// test opens each page this way. Pure apart from wrapping the client it is given.

import type { ApiClient } from '../client';

/** Fields that name or identify an item: copies get a long, unique value. */
const NAMES = ['id', 'key', 'name', 'jobKey', 'runId', 'eventId', 'nodeKey', 'migrationEventId', 'invocationId', 'runKey', 'table', 'group', 'displayName'];
/** Fields that point at another item, such as a check's node: copies point at one of `REFERENCED` long names, shared by a few rows each, so a filter over them has thousands of options. */
const REFS = ['node'];
const REFERENCED = 2000;
/** Fields of free text: made very long. */
const TEXTS = ['description', 'select', 'summary', 'message', 'title', 'text', 'cadence', 'className', 'sourceFile', 'path'];
const FILLER = ' with a deliberately long tail of words that keeps going well past any column width a list could give it';
const LONG_ID = '_with_an_unusually_long_identifier_that_no_column_is_wide_enough_to_show';

/** `items` stretched to `count` rows: the originals first, unchanged in their names, then copies with long, unique names; text made long throughout. */
export function stretch<T>(items: readonly T[], count: number): T[] {
	if (!items.length || typeof items[0] !== 'object' || items[0] === null) return [...items];
	return Array.from({ length: Math.max(count, items.length) }, (_, i) => {
		const copy = { ...(items[i % items.length] as Record<string, unknown>) };
		for (const field of NAMES) if (i >= items.length && typeof copy[field] === 'string') copy[field] = `${copy[field]}${LONG_ID}_${i}`;
		// The same reference number always names the same item, so there are at most `REFERENCED` of them.
		const ref = i % REFERENCED;
		const base = items[ref % items.length] as Record<string, unknown>;
		for (const field of REFS) if (i >= items.length && typeof copy[field] === 'string' && typeof base[field] === 'string') copy[field] = ref < items.length ? base[field] : `${base[field]}${LONG_ID}_${ref}`;
		for (const field of TEXTS) if (typeof copy[field] === 'string' && copy[field]) copy[field] = `${copy[field]}${FILLER}`;
		return copy as T;
	});
}

/** The client with every `list…` read that answers an array stretched to `count` rows. */
export function awkward(client: ApiClient, count = 5000): ApiClient {
	return new Proxy(client, {
		get(target, prop, receiver) {
			const value = Reflect.get(target, prop, receiver);
			if (typeof value !== 'function' || typeof prop !== 'string' || !prop.startsWith('list')) return typeof value === 'function' ? value.bind(target) : value;
			return async (...args: unknown[]) => {
				const answer = await value.apply(target, args);
				return Array.isArray(answer) ? stretch(answer, count) : answer;
			};
		}
	});
}
