import { ApiError } from '$lib/api/client';

export const SERVER_DOCS = 'https://sqlbuild.com/docs/cli/server/';

/** What the shell shows when the bootstrap fails. */
export interface BootFailure {
	title: string;
	detail: string;
	/** Retrying cannot help; the server's configuration must change. */
	configuration: boolean;
}

/**
 * A server without a person sign-in mode has no bootstrap endpoint, so a 404 there means the
 * UI cannot be used on this server, not that the server is unreachable.
 */
export function bootFailure(error: unknown): BootFailure {
	if (error instanceof ApiError && error.status === 404) {
		return {
			title: 'This server has no person sign-in configured',
			detail: 'Start sqb server with password, OIDC or trusted-proxy sign-in to use this UI.',
			configuration: true
		};
	}
	return {
		title: "Can't reach the SQLBuild server",
		detail: error instanceof Error ? error.message : String(error),
		configuration: false
	};
}
