import { describe, expect, it } from 'vitest';
import { PROXY_CSRF_PROOF, csrfToken } from './http-client';

type CsrfCase = { description: string; cookies: string; expected: string };

const cases: CsrfCase[] = [
	{ description: 'a sign-in cookie supplies the token', cookies: 'theme=dark; __Host-sqb-csrf=abc=def', expected: 'abc=def' },
	{ description: 'behind a trusted proxy, with no cookie, writes send the proxy proof', cookies: 'theme=dark', expected: PROXY_CSRF_PROOF }
];

describe('csrfToken', () => {
	it.each(cases)('$description', ({ cookies, expected }) => {
		expect(csrfToken(cookies)).toBe(expected);
	});
});
