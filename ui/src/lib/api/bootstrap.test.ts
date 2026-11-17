import { describe, expect, it } from 'vitest';
import { toBootstrap } from './http-client';

type BootstrapCase = {
	description: string;
	proxy: { display_name?: string; logout_url?: string };
	expected: { displayName: string; logoutUrl: string | undefined };
};

const cases: BootstrapCase[] = [
	{
		description: 'a proxy-reported display name and sign-out URL are shown',
		proxy: { display_name: 'Orders Viewer', logout_url: 'https://sign-in.example.com/logout' },
		expected: { displayName: 'Orders Viewer', logoutUrl: 'https://sign-in.example.com/logout' }
	},
	{
		description: 'without them the username is shown and there is no sign-out URL',
		proxy: {},
		expected: { displayName: 'orders-viewer', logoutUrl: undefined }
	}
];

describe('toBootstrap', () => {
	it.each(cases)('$description', ({ proxy, expected }) => {
		const bootstrap = toBootstrap({
			login_mode: 'trusted_proxy',
			access: { name: 'orders-viewer', principal_id: 'p', actions: [], locations: [] },
			...proxy
		});

		expect({ displayName: bootstrap.user.displayName, logoutUrl: bootstrap.logoutUrl }).toEqual(expected);
	});
});
