import { expect, test, type Page } from '@playwright/test';
import { FakeControl } from './support/fake-control';

// The real Overview and its server client, against a control server faked by the test: an acknowledged
// condition that clears is withdrawn once, naming that acknowledgement, and never on a read that can't
// show it cleared: another target's, one taken during a sync, a failed one, or one already superseded.
test.use({ viewport: { width: 1600, height: 1000 } });

const health = (page: Page) => page.locator('section.health');
const item = (page: Page, text: string | RegExp) => health(page).locator('.issues .issue', { hasText: text });
/** Enough time for the page to read, judge and send whatever it would. */
const settle = (page: Page) => page.waitForTimeout(2_500);

async function acknowledge(page: Page, text: string | RegExp) {
	await expect(item(page, text)).toBeVisible({ timeout: 20_000 });
	await item(page, text).getByRole('button', { name: 'Acknowledge' }).click();
	await expect(item(page, text)).toHaveCount(0);
}

test('a cleared condition is withdrawn once, naming its acknowledgement, and comes back as new', async ({ page }) => {
	const server = new FakeControl({ shop: { changed: ['orders', 'order_items', 'customers'] } });
	await server.install(page);
	await page.goto('/');
	await acknowledge(page, '3 nodes out of date');
	const [ack] = server.targets.shop.acks;
	expect(ack.key).toBe('stale_models');

	server.targets.shop.changed = [];
	await page.reload();
	await expect.poll(() => server.withdrawals().length, { timeout: 15_000 }).toBe(1);
	await settle(page);
	const [withdrawal] = server.withdrawals();
	expect(withdrawal).toMatchObject({ target: 'shop', body: { key: 'stale_models', acknowledged: false, withdraws: ack.event_id, reason: 'automatic: the condition cleared' } });
	expect(server.withdrawals()).toHaveLength(1);
	expect(server.targets.shop.acks).toEqual([]);

	server.targets.shop.changed = ['orders'];
	await page.reload();
	await expect(item(page, '1 node out of date').getByRole('button', { name: 'Acknowledge' })).toBeVisible({ timeout: 20_000 });
	await expect(health(page).getByRole('button', { name: /^Acknowledged/ })).toHaveCount(0);
	expect(server.withdrawals()).toHaveLength(1);
});

test('a first read made before the sync status arrives is taken again once it does, and then withdraws', async ({ page }) => {
	const server = new FakeControl({ shop: { changed: ['orders', 'order_items'] } });
	await server.install(page);
	await page.goto('/');
	await acknowledge(page, '2 nodes out of date');
	server.targets.shop.changed = [];
	server.targets.shop.healthDelayMs = 3_000;
	await page.reload();
	await expect(item(page, 'out of date')).toHaveCount(0, { timeout: 20_000 });
	// Nothing else prompts a read: the page reads again by itself once it knows node state has settled.
	await expect.poll(() => server.withdrawals().length, { timeout: 15_000 }).toBe(1);
	await settle(page);
	expect(server.withdrawals()).toHaveLength(1);
});

test('a read taken during a sync withdraws nothing; a read after it settles does', async ({ page }) => {
	const server = new FakeControl({ shop: { changed: ['orders', 'order_items'] } });
	await server.install(page);
	await page.goto('/');
	await acknowledge(page, '2 nodes out of date');
	server.targets.shop.changed = [];
	server.targets.shop.sync = 'running';
	await page.reload();
	await expect(item(page, 'out of date')).toHaveCount(0, { timeout: 20_000 });
	await settle(page);
	expect(server.withdrawals()).toEqual([]);

	// The sync finishes: the page learns it, reads again, and only that new read withdraws.
	server.targets.shop.sync = 'settled';
	server.notify('all');
	await expect.poll(() => server.withdrawals().length, { timeout: 15_000 }).toBe(1);
});

test('a failed read withdraws nothing, even after a withdrawal that failed', async ({ page }) => {
	const server = new FakeControl({ shop: { changed: ['orders', 'order_items'] } });
	await server.install(page);
	await page.goto('/');
	await acknowledge(page, '2 nodes out of date');
	server.targets.shop.changed = [];
	server.targets.shop.failWithdrawals = true;
	await page.reload();
	await expect.poll(() => server.withdrawals().length, { timeout: 15_000 }).toBe(1);
	expect(server.withdrawals()[0].status).toBe(500);

	// The next read fails: what is still shown is not judged again.
	server.targets.shop.failReads = true;
	server.notify('runs');
	await expect(page.locator('.error-note')).toBeVisible({ timeout: 15_000 });
	await settle(page);
	expect(server.withdrawals()).toHaveLength(1);
});

test('switching target while a withdrawal is held back sends nothing to the new target', async ({ page }) => {
	const server = new FakeControl({ shop: { changed: ['orders', 'order_items'] }, returns: { changed: ['refunds'] } });
	await server.install(page);
	await page.goto('/');
	await acknowledge(page, '2 nodes out of date');
	// The withdrawal on shop fails, so it is held back, not marked as sent.
	server.targets.shop.changed = [];
	server.targets.shop.failWithdrawals = true;
	await page.reload();
	await expect.poll(() => server.withdrawals('shop').length, { timeout: 15_000 }).toBe(1);

	// returns holds an acknowledgement under the same key; its read is slow, so shop's data stays shown.
	server.targets.returns.acks = [{ event_id: '0190d1a0-0000-7000-8000-0000000000a2', key: 'stale_models', shape: { counts: { changed: 1, behind: 0 } }, acknowledged_at: new Date().toISOString() }];
	server.targets.returns.readDelayMs = 3_000;
	await page.getByRole('button', { name: 'Switch location' }).click();
	await page.getByRole('menuitemradio', { name: /returns/ }).click();
	await page.waitForTimeout(4_500);
	expect(server.withdrawals('returns')).toEqual([]);
	expect(server.withdrawals('shop')).toHaveLength(1);
});

test('a slow read that saw a condition clear withdraws nothing once a newer read shows it back', async ({ page }) => {
	const server = new FakeControl({ shop: { failedRun: true } });
	server.stopScheduler();
	await server.install(page);
	await page.goto('/');
	await acknowledge(page, /scheduler/i);

	// The next read sees the scheduler back, but slowly; meanwhile it stops again and a newer read shows it.
	for (const t of Object.values(server.targets)) t.schedulerLeader = true;
	server.targets.shop.readDelayMs = 3_000;
	server.notify('runs');
	await page.waitForTimeout(1_200);
	server.stopScheduler();
	server.targets.shop.readDelayMs = 0;
	// Acknowledging the failed run reads the Overview again at once, overtaking the slow read.
	await item(page, /failed/).getByRole('button', { name: 'Acknowledge' }).click();
	await page.waitForTimeout(4_000);
	expect(server.withdrawals()).toEqual([]);
});
