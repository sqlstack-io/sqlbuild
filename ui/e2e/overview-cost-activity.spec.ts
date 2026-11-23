import { expect, test } from '@playwright/test';

// The Overview's top row has a 7-day cost card for a location whose adapter reports cost, and its
// Activity lists the jobs active in the window first, folding the idle ones.
test.use({ viewport: { width: 1600, height: 1000 } });

test('a Snowflake location shows Cost · 7 days in the top row, linking to the breakdown', async ({ page }) => {
	await page.goto('/?mock');
	const card = page.getByTestId('cost-card');
	await expect(card).toContainText('Cost · 7 days', { timeout: 20_000 });
	await card.getByRole('link', { name: 'Breakdown' }).click();
	await expect(page).toHaveURL(/#cost$/);
	await expect(page.locator('#cost')).toBeInViewport();
});

test('a location whose adapter reports no cost has no cost card', async ({ page }) => {
	await page.goto('/?mock');
	await expect(page.getByTestId('cost-card')).toBeVisible({ timeout: 20_000 });
	await page.getByRole('button', { name: 'Switch location' }).click();
	await page.getByRole('menuitemradio', { name: /waffle-shop-dev/ }).click();
	await expect(page.getByRole('button', { name: 'Switch location' })).toContainText('waffle-shop-dev');
	await expect(page.locator('.metric').first()).toBeVisible({ timeout: 20_000 });
	await expect(page.getByTestId('cost-card')).toHaveCount(0);
});

test('Activity folds idle jobs, opens them in place, and a search finds them folded or not', async ({ page }) => {
	await page.goto('/?mock');
	const activity = page.locator('.surface-panel', { has: page.getByRole('heading', { name: 'Activity' }) });
	const fold = activity.getByRole('button', { name: /^\d+ idle jobs?$/ });
	await expect(fold).toBeVisible({ timeout: 20_000 });
	const idle = Number((await fold.innerText()).match(/\d+/)?.[0]);
	const rows = activity.locator('.tl-body .row:not(.fold)');
	const folded = await rows.count();
	await fold.click();
	await expect(fold).toHaveAttribute('aria-expanded', 'true');
	await expect(rows).toHaveCount(folded + idle);
	await fold.click();
	await expect(rows).toHaveCount(folded);
	// A search lists every match, idle or not, without the fold.
	const name = (await activity.locator('.tl-body .row:not(.fold) .job-name').first().innerText()).trim();
	await activity.getByPlaceholder('Search jobs').fill(name);
	await expect(activity.getByRole('button', { name: /idle job/ })).toHaveCount(0);
});
