import { browser } from '$app/environment';
import type { ApiClient, Scope } from '$lib/api';

/**
 * How long a click waits on the current page for the next page's data, so a quick answer shows
 * whole instead of empty for a moment; longer than this, the page switches and shows its own loading state.
 */
const HOLD_MS = 200;
/** A prefetched read older than this, such as from a hover with no click, is read again instead. */
const KEEP_MS = 5000;

type Read = (client: ApiClient, scope: Scope, match: RegExpMatchArray, search: URLSearchParams) => Promise<unknown>[];

const started = new Map<string, { at: number; promise: Promise<unknown> }>();
let base: ApiClient | null = null;

const keyOf = (method: PropertyKey, args: unknown[]) => `${String(method)}|${JSON.stringify(args)}`;

/**
 * Wraps the client so a page's first reads, already started by `prefetch` before the switch, are
 * taken over by the page instead of being sent again.
 */
export function withPrefetch(client: ApiClient): ApiClient {
	base = client;
	return new Proxy(client, {
		get(target, method, receiver) {
			const value = Reflect.get(target, method, receiver);
			if (typeof value !== 'function') return value;
			return (...args: unknown[]) => {
				const key = keyOf(method, args);
				const kept = started.get(key);
				if (kept) {
					started.delete(key);
					if (Date.now() - kept.at < KEEP_MS) return kept.promise;
				}
				return value.apply(target, args);
			};
		}
	});
}

/**
 * Starts the reads the page at `pathname` makes first and waits for them briefly, so a fast answer
 * arrives with the page. Errors are left for the page to show.
 */
export async function prefetch(scope: Scope | null, pathname: string, search: URLSearchParams, reads: [RegExp, Read][]): Promise<void> {
	if (!browser || !base || !scope) return;
	const found = reads.map(([pattern, read]) => [pathname.match(pattern), read] as const).find(([match]) => match);
	if (!found) return;
	const client = base;
	const recorder = new Proxy(client, {
		get(target, method, receiver) {
			const value = Reflect.get(target, method, receiver);
			if (typeof value !== 'function' || method === 'supports') return value;
			return (...args: unknown[]) => {
				if (!client.supports(method as keyof ApiClient)) return Promise.resolve();
				const promise = Promise.resolve(value.apply(target, args));
				promise.catch(() => {});
				started.set(keyOf(method, args), { at: Date.now(), promise });
				return promise;
			};
		}
	});
	const pending = Promise.allSettled(found[1](recorder, scope, found[0]!, search));
	await Promise.race([pending, new Promise((resolve) => setTimeout(resolve, HOLD_MS))]);
}
