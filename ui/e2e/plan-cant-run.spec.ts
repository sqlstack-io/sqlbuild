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
	// Will run leaves out what can't run: only stg_customers is left of the 13 planned.
	await expect(page.getByTestId('index-will-run')).toHaveText('Will run1');
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

test('a sink whose provider is missing a secret stops the whole plan, with no way around it', async ({ page }) => {
	await page.goto(`/plan?mock&secrets=sink&cmd=${encodeURIComponent('sqb build --select stg_customers')}`);
	await expect(page.locator('.pane[data-section="cant_run"]')).toBeVisible({ timeout: 30_000 });
	await expect(page.getByTestId('index-cant-run')).toHaveText('Can’t runall');
	await expect(page.getByTestId('index-will-run')).toHaveText('Will run0');
	await expect(page.getByTestId('cant-run-sinks')).toHaveText('run_notifications: every run uses sinks, so nothing can run');
	await expect(page.locator('.foot .why')).toHaveText('Run is disabled: sink run_notifications needs CHAT_WEBHOOK_URL; every run uses sinks. Fix it and plan again.');
	await expect(page.getByRole('button', { name: 'Run', exact: true })).toBeDisabled();
	await expect(page.getByTestId('plan-without-blocked')).toHaveCount(0);
});
