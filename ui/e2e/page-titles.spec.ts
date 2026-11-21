import { expect, test } from '@playwright/test';

// Every page names itself in the browser tab, `<Page> · SQLBuild`, and a detail page names what it shows.
for (const [path, expected] of [
	['/?mock', 'Overview · SQLBuild'],
	['/admin/people?mock', 'People · SQLBuild'],
	['/runs?mock', 'Runs · SQLBuild'],
	['/runs/run-a7c1?mock', 'Run a7c1 · revenue_refresh · SQLBuild'],
	['/jobs/revenue_refresh?mock', 'revenue_refresh · Job · SQLBuild'],
	['/lineage?mock', 'Lineage · SQLBuild'],
	[`/plan?mock&cmd=${encodeURIComponent('sqb build --changed')}`, 'Plan · sqb build --changed · SQLBuild'],
	['/settings?mock', 'Settings · SQLBuild']
] as const) {
	test(`${path} is titled "${expected}"`, async ({ page }) => {
		await page.goto(path);
		await expect(page).toHaveTitle(expected, { timeout: 20_000 });
	});
}
