import { expect, test } from '@playwright/test';

// A plan whose provider is missing a secret (`?mock&secrets`: the orders API has no key): the nodes that
// use it and those that read them are listed under Can't run, Run is disabled with the reason, and
// "Plan without these N nodes" plans the rest, which can run.
test.use({ viewport: { width: 1500, height: 900 } });

const PLAN = `/plan?mock&secrets&cmd=${encodeURIComponent('sqb build --select raw__orders+ stg_customers')}`;

test('a missing provider secret lists what can’t run, disables Run, and planning without those nodes runs the rest', async ({ page }) => {
	await page.goto(PLAN);
	const pane = page.locator('.pane[data-section="cant_run"]');
	await expect(pane).toBeVisible({ timeout: 30_000 });
	await expect(page.getByTestId('index-cant-run')).toHaveText('Can’t run12');
	const problem = page.getByTestId('cant-run-problem');
	await expect(problem).toContainText("Can't run: provider orders_api needs ORDERS_API_KEY");
	await expect(problem).toContainText('raw__orders and the 11 nodes that read it');
	await expect(problem).toContainText("set ORDERS_API_KEY in the location's environment (or its .env), restart the location, then plan again");
	await expect(pane.locator('tbody tr').first()).toContainText('provider orders_api: ORDERS_API_KEY not set');
	const row = (node: string) => pane.locator('tbody tr').filter({ has: page.getByRole('link', { name: node, exact: true }) });
	await expect(row('stg_orders')).toContainText('reads raw__orders');

	const run = page.getByRole('button', { name: 'Run', exact: true });
	await expect(run).toBeDisabled();
	await expect(page.locator('.foot .why')).toHaveText('Run is disabled: 1 missing secret. Fix it and plan again, or leave those 12 nodes out.');

	await page.getByTestId('plan-without-blocked').click();
	await expect(page.getByRole('textbox', { name: 'Command' })).toHaveValue(/--exclude raw__orders stg_orders fact_orders/);
	await expect(page.getByTestId('index-cant-run')).toHaveCount(0, { timeout: 30_000 });
	await expect(run).toBeEnabled({ timeout: 30_000 });
});
