import { describe, expect, it } from 'vitest';
import { signOutFor, type SignOut } from './sign-out';
import type { Bootstrap } from '$lib/api/types';

type SignOutCase = {
	description: string;
	bootstrap: Pick<Bootstrap, 'authMode' | 'logoutUrl'> | null;
	expected: SignOut;
};

const cases: SignOutCase[] = [
	{ description: 'a password session ends on the server', bootstrap: { authMode: 'password' }, expected: { kind: 'session' } },
	{
		description: 'a trusted proxy with a sign-out URL links to it',
		bootstrap: { authMode: 'trusted_proxy', logoutUrl: 'https://sign-in.example.com/logout' },
		expected: { kind: 'link', href: 'https://sign-in.example.com/logout' }
	},
	{ description: 'a trusted proxy without one offers no sign-out', bootstrap: { authMode: 'trusted_proxy' }, expected: null },
	{ description: 'local development has no sign-in', bootstrap: { authMode: 'disabled' }, expected: null },
	{ description: 'before the bootstrap loads', bootstrap: null, expected: null }
];

describe('signOutFor', () => {
	it.each(cases)('$description', ({ bootstrap, expected }) => {
		expect(signOutFor(bootstrap)).toEqual(expected);
	});
});
