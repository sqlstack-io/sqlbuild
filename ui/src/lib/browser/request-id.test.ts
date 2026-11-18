import { webcrypto } from 'node:crypto';
import { describe, expect, it } from 'vitest';
import { newRequestId } from './request-id';

const V4 = /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/;
/** A browser page that is not a secure context: random values, but no `randomUUID`. */
const insecure = { getRandomValues: <T extends ArrayBufferView | null>(array: T) => webcrypto.getRandomValues(array as Uint8Array) as unknown as T };

describe('request IDs', () => {
	it('builds RFC 4122 version 4 UUIDs from random values when randomUUID is missing', () => {
		const ids = Array.from({ length: 2000 }, () => newRequestId(insecure));
		for (const id of ids) expect(id).toMatch(V4);
		expect(new Set(ids).size).toBe(ids.length);
	});

	it('sets the version and variant bits whatever the random bytes are', () => {
		for (const fill of [0x00, 0xff]) {
			const id = newRequestId({ getRandomValues: <T extends ArrayBufferView | null>(a: T) => ((a as Uint8Array).fill(fill), a) });
			expect(id).toMatch(V4);
		}
		expect(newRequestId({ getRandomValues: <T extends ArrayBufferView | null>(a: T) => ((a as Uint8Array).fill(0), a) })).toBe('00000000-0000-4000-8000-000000000000');
	});

	it('uses randomUUID where the page has it', () => {
		expect(newRequestId({ ...insecure, randomUUID: () => 'from-randomUUID' })).toBe('from-randomUUID');
	});
});
