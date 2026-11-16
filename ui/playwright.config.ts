import { defineConfig } from '@playwright/test';

// The UI's browser suite: layout checks against the mock client with awkward data. CI builds the UI
// (`npm run build`) and runs `npm run test:e2e`; it is not part of `npm test`.
export default defineConfig({
	testDir: './e2e',
	timeout: 120_000,
	fullyParallel: true,
	use: {
		baseURL: process.env.UI_URL || 'http://127.0.0.1:4175',
		launchOptions: process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : {}
	},
	webServer: process.env.UI_URL
		? undefined
		: {
				command: 'npx vite preview --host 127.0.0.1 --port 4175 --strictPort',
				url: 'http://127.0.0.1:4175',
				reuseExistingServer: false
			}
});
