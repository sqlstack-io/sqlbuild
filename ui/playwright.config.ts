import { defineConfig } from '@playwright/test';

// The UI's browser suite against the mock client with awkward data; it is not part of `npm test`.
// CI builds the UI (`npm run build`) and runs the layout and work-budget projects as required steps,
// and the perf project in its own job, which reports its timings but does not block merges.
export default defineConfig({
	testDir: './e2e',
	timeout: 120_000,
	fullyParallel: true,
	// Timing guards run once the rest has finished, so they have the machine to themselves.
	projects: [
		{ name: 'layout', testIgnore: /(perf|budgets)\.spec\.ts/ },
		{ name: 'budgets', testMatch: /budgets\.spec\.ts/ },
		{ name: 'perf', testMatch: /perf\.spec\.ts/, dependencies: ['layout'] }
	],
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
