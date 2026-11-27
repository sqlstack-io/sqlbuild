// The sync-controls preview only: the usual dev server, plus the design before the calm pass, built with
// `SQB_UI_BASE=/before VITE_SQB_MOCK=1` into `SQB_BEFORE_BUILD`, served at `/before/` with the app's
// fallback page for its routes. Not used by builds, tests or `sqb server`.
import { existsSync, readFileSync } from 'node:fs';
import { extname, join, normalize } from 'node:path';
import type { Plugin } from 'vite';
import { defineConfig, mergeConfig, type UserConfig } from 'vite';
import base from './vite.config';

const TYPES: Record<string, string> = { '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.png': 'image/png', '.svg': 'image/svg+xml', '.woff2': 'font/woff2', '.txt': 'text/plain' };

function before(dir: string): Plugin {
	return {
		name: 'sync-controls-before',
		configureServer(server) {
			server.middlewares.use('/before', (req, res) => {
				const path = normalize(decodeURIComponent((req.url ?? '/').split('?')[0])).replace(/^(\.\.[/\\])+/, '');
				const file = join(dir, path);
				const found = file.startsWith(dir) && existsSync(file) && extname(file) ? file : join(dir, 'index.html');
				res.setHeader('content-type', TYPES[extname(found)] ?? 'application/octet-stream');
				res.end(readFileSync(found));
			});
		}
	};
}

export default defineConfig((env) => {
	const config = (typeof base === 'function' ? base(env) : base) as UserConfig;
	const dir = process.env.SQB_BEFORE_BUILD;
	return mergeConfig(config, { plugins: dir ? [before(dir)] : [] });
});
