import { test } from '@playwright/test';

// Screenshots for looking at pages by eye, not a check: skipped unless SHOTS names a directory.
// SHOTS=/some/dir npx playwright test e2e/screens.spec.ts
const dir = process.env.SHOTS;
const PAGES = [
	{ name: 'group-page', path: '/jobs/warehouse_nightly?mock' },
	{ name: 'lineage', path: '/lineage?mock' }
];

test.describe('screenshots', () => {
	test.skip(!dir, 'SHOTS is not set');
	test.use({ viewport: { width: 1600, height: 900 } });
	for (const { name, path } of PAGES) {
		test(name, async ({ page }) => {
			await page.goto(path);
			await page.waitForLoadState('networkidle');
			await page.waitForTimeout(1000);
			await page.screenshot({ path: `${dir}/${name}.png` });
		});
	}
});
