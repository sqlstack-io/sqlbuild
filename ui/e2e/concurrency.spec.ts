import { expect, test } from '@playwright/test';

// A job's concurrency in plain words: the rule, what shares it, and the group as a detail. With SHOTS
// set, the job page is photographed: SHOTS=/some/dir npx playwright test e2e/concurrency.spec.ts
const shots = process.env.SHOTS;
test.use({ viewport: { width: 1600, height: 900 } });

for (const [job, others, group] of [
	['core_build', ['reporting_exports', 'close_of_business'], false],
	['close_of_business', ['core_build', 'reporting_exports'], true]
] as const) {
	test(`${job} states its concurrency rule and what it shares it with`, async ({ page }) => {
		await page.goto(`/jobs/${job}?mock`);
		// A group page opens on its graph, its details (Lineage's sidebar) closed.
		if (group) await page.getByRole('button', { name: 'Toggle details' }).click({ timeout: 20_000 });
		const row = page.locator('.concurrency');
		await expect(row).toHaveCount(1, { timeout: 20_000 });
		await expect(row.locator('dt')).toHaveText('Concurrency');
		await expect(row.locator('dd > span').first()).toHaveText(`One run at a time, shared with ${others.join(', ')}`);
		await expect(row.locator('.detail').first()).toHaveText('(concurrency group warehouse_heavy, limit 1)');
		for (const other of others) await expect(row.getByRole('link', { name: other })).toHaveAttribute('href', new RegExp(`/jobs/${other}`));
		await expect(page.getByText('Shared slots')).toHaveCount(0);
		if (shots) await row.scrollIntoViewIfNeeded().then(() => page.screenshot({ path: `${shots}/concurrency-${job}.png` }));
	});
}
