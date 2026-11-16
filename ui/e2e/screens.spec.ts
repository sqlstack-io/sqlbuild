import { test } from '@playwright/test';

// Screenshots for looking at pages by eye, not a check: skipped unless SHOTS names a directory.
// SHOTS=/some/dir npx playwright test e2e/screens.spec.ts
const dir = process.env.SHOTS;
const PAGES: { name: string; path: string; click?: string }[] = [
	{ name: 'group-page', path: '/jobs/warehouse_nightly?mock' },
	{ name: 'group-page-members', path: '/jobs/warehouse_nightly?mock', click: 'Folders' },
	{ name: 'lineage', path: '/lineage?mock' },
	{ name: 'job-page', path: '/jobs/core_build?mock' }
];

test.describe('screenshots', () => {
	test.skip(!dir, 'SHOTS is not set');
	test.use({ viewport: { width: 1600, height: 900 } });
	for (const { name, path, click } of PAGES) {
		test(name, async ({ page }) => {
			await page.goto(path);
			await page.waitForLoadState('networkidle');
			// Each page here draws a graph: wait for it to be drawn and fitted.
			await page.locator('.graph-canvas:not(.unfitted)').first().waitFor({ timeout: 15_000 });
			if (click) {
				await page.getByRole('button', { name: click }).click();
				await page.waitForTimeout(800);
			}
			await page.waitForTimeout(1000);
			await page.screenshot({ path: `${dir}/${name}.png` });
		});
	}
});
