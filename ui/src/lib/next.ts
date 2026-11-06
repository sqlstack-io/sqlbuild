const PROBE = 'https://same-site.invalid';

/**
 * Where to go after sign-in: the `next` path when it stays on this site, else `/`.
 * Resolving against a fixed origin catches `//host`, `/\host` and control characters the
 * browser would strip, which a prefix check misses. Resolution also removes dot segments, so
 * `/.//host` comes out as `//host`; a result that starts with two slashes is refused too.
 */
export function safeNext(next: string | null): string {
	if (!next?.startsWith('/')) return '/';
	let url: URL;
	try {
		url = new URL(next, PROBE);
	} catch {
		return '/';
	}
	if (url.origin !== PROBE || url.pathname.startsWith('//')) return '/';
	return `${url.pathname}${url.search}${url.hash}`;
}
