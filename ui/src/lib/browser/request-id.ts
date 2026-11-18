// IDs for the requests the UI makes (runs, events, plans). `crypto.randomUUID` exists only in secure
// contexts (HTTPS or localhost), and the UI is also served over plain HTTP behind a proxy, so without it
// the ID is built from `crypto.getRandomValues`, which every context has. Never `Math.random`: these IDs
// make retried requests apply once, so they must not collide.

/** A random RFC 4122 version 4 UUID. */
export function newRequestId(source: Pick<Crypto, 'getRandomValues'> & { randomUUID?: () => string } = crypto): string {
	if (typeof source.randomUUID === 'function') return source.randomUUID();
	const bytes = source.getRandomValues(new Uint8Array(16));
	bytes[6] = (bytes[6] & 0x0f) | 0x40; // version 4
	bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant 10xx
	const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, '0')).join('');
	return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}
