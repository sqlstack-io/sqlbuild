import { expect, test } from '@playwright/test';

// The Jobs list: every row is one height, with name and description on two lines, and cells holding
// several items keep them on one line with a "+N" badge, at wide and narrow widths alike.
for (const width of [2000, 1280]) {
	test(`at ${width} px every job row has one height and nothing overflows sideways`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		await page.goto('/jobs?mock');
		const rows = page.locator('table tbody tr');
		await expect(rows.first()).toBeVisible({ timeout: 20_000 });
		const heights = await rows.evaluateAll((all) => all.map((r) => Math.round(r.getBoundingClientRect().height)));
		expect(new Set(heights).size, `row heights ${heights.join(', ')}`).toBe(1);
		const overflow = await page.evaluate(() => document.documentElement.scrollWidth - document.documentElement.clientWidth);
		expect(overflow).toBeLessThanOrEqual(0);
	});
}

test('a cell with more selectors than fit shows +N, and its popup lists them all', async ({ page }) => {
	await page.setViewportSize({ width: 1280, height: 900 });
	await page.goto('/jobs?mock');
	const badge = page.getByRole('button', { name: /^All \d+ selectors$/ }).first();
	await expect(badge).toBeVisible({ timeout: 20_000 });
	const total = Number((await badge.getAttribute('aria-label'))?.match(/\d+/)?.[0]);
	await badge.click();
	await expect(page.locator('.fit-pop .fit-pop-item')).toHaveCount(total);
});
