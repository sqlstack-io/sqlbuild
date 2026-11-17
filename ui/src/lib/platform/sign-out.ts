import type { Bootstrap } from '$lib/api/types';

/** How a person signs out: end the server session, follow the proxy's sign-out link, or not at all. */
export type SignOut = { kind: 'session' } | { kind: 'link'; href: string } | null;

/**
 * A trusted proxy owns its sign-in, so only its sign-out URL can end it; without one there is no
 * sign-out. Local development has no sign-in.
 */
export function signOutFor(bootstrap: Pick<Bootstrap, 'authMode' | 'logoutUrl'> | null): SignOut {
	if (!bootstrap || bootstrap.authMode === 'disabled') return null;
	if (bootstrap.authMode !== 'trusted_proxy') return { kind: 'session' };
	return bootstrap.logoutUrl ? { kind: 'link', href: bootstrap.logoutUrl } : null;
}
