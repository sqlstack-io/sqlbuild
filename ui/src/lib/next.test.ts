import { describe, expect, it } from 'vitest';
import { safeNext } from './next';

describe('safeNext', () => {
	it.each([
		['/runs?status=failed#latest', '/runs?status=failed#latest'],
		['/catalog/model:orders', '/catalog/model:orders'],
		[null, '/'],
		['', '/']
	])('keeps the same-site path %s', (next, expected) => {
		expect(safeNext(next)).toBe(expected);
	});

	it.each([
		'https://evil.example/runs',
		'//evil.example/runs',
		'/\\evil.example/runs',
		'/\t/evil.example/runs',
		'javascript:alert(1)',
		'runs'
	])('refuses %s', (next) => {
		expect(safeNext(next)).toBe('/');
	});
});
