import { defineConfig } from '@playwright/test';

export default defineConfig({
	testDir: './tests',
	use: {
		baseURL: process.env.SITE_URL || 'http://127.0.0.1:4174',
		deviceScaleFactor: 2,
		launchOptions: process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : {},
	},
	webServer: process.env.SITE_URL ? undefined : {
		command: 'npm run preview -- --host 127.0.0.1 --port 4174 --ignore-lock',
		url: 'http://127.0.0.1:4174',
	},
});
