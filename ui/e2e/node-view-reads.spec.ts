import { expect, test, type Page } from '@playwright/test';

// The node view reads its detail once on a first visit, and once for each read of the shared checks after
// that: a burst of change signals reloads it once, never once for the signal and again for the new checks.

const reads = (page: Page) => page.evaluate(() => ({ ...((window as unknown as { __sqbMockReads?: Record<string, number> }).__sqbMockReads ?? {}) }));

async function openNode(page: Page, search: string, name?: string) {
	await page.goto(`/catalog?${search}`);
	const link = page.locator('table.sb-table tbody tr a', name ? { hasText: name } : {}).first();
	await link.waitFor();
	await link.click();
	await page.waitForURL(/\/catalog\/[^/?]+/);
	await page.getByText('Description').first().waitFor();
	await page.waitForLoadState('networkidle');
	await page.waitForTimeout(800);
}

test('a first visit to a node reads its detail once', async ({ page }) => {
	await openNode(page, 'mock&still');
	expect((await reads(page)).getCatalogNode).toBe(1);
});

test('signals reload the node detail at most once per read of the shared checks', async ({ page }) => {
	test.setTimeout(60_000);
	await openNode(page, 'mock', 'fact_orders');
	// Start a build of the node, then come back to it in history, which keeps the in-memory mock's run going.
	await page.getByRole('button', { name: 'Build' }).first().click();
	await page.getByRole('button', { name: 'Run', exact: true }).click();
	await page.waitForURL(/\/runs\//);
	await page.goBack();
	await page.goBack();
	await page.waitForURL(/\/catalog\/[^/?]+/);
	await page.getByText('Description').first().waitFor();
	// Reads the launch's own signal started settle first, so the count starts between bursts.
	await page.waitForTimeout(1000);
	const before = await reads(page);
	// The run advances and announces it: a few bursts of change signals.
	await page.waitForTimeout(10_000);
	const after = await reads(page);
	const delta = (name: string) => (after[name] ?? 0) - (before[name] ?? 0);
	console.info(`NODE READS listChecks ${delta('listChecks')} getCatalogNode ${delta('getCatalogNode')}`);
	expect(delta('listChecks'), 'the signals start checks reads').toBeGreaterThan(0);
	expect(delta('getCatalogNode')).toBeLessThanOrEqual(delta('listChecks'));
});
