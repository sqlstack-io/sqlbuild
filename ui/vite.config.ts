import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig, loadEnv } from 'vite';

// `SQB_SERVER_URL` proxies `/api` to a running `sqb server` during development.
// The server only accepts browser sessions from its configured HTTPS origin, so
// `SQB_SERVER_ORIGIN` names that origin and the proxy presents it upstream.
export default defineConfig(({ mode }) => {
	const env = loadEnv(mode, process.cwd(), 'SQB_');
	const target = env.SQB_SERVER_URL;
	const origin = env.SQB_SERVER_ORIGIN;
	return {
		plugins: [tailwindcss(), sveltekit()],
		server: target
			? {
					proxy: {
						'/api': {
							target,
							headers: origin ? { host: new URL(origin).host, origin } : undefined
						}
					}
				}
			: undefined
	};
});
