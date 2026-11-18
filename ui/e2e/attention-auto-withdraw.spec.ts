import { expect, test, type Page } from '@playwright/test';
import { FakeControl, type TargetName } from './support/fake-control';

// The real Overview and its server client, against a control server faked by the test: an acknowledged
// condition that clears is withdrawn once, naming that acknowledgement, and never on a read that can't
// show it cleared: another target's, one taken during a sync, a failed one, or one already superseded.
test.use({ viewport: { width: 1600, height: 1000 } });

const health = (page: Page) => page.locator('section.health');
const item = (page: Page, text: string | RegExp) => health(page).locator('.issues .issue', { hasText: text });
/** How long the page has, once a read is answered, to judge it and send what it would; the absence of a
 *  request can't be awaited, so this is the one fixed wait. */
const GRACE_MS = 750;

/** Waits until `count` Overview reads of `target` have been answered, then gives the page its grace. */
async function afterReads(page: Page, server: FakeControl, count: number, target: TargetName = 'shop') {
	await expect.poll(() => server.reads[target].answered, { timeout: 20_000 }).toBeGreaterThanOrEqual(count);
	await page.waitForTimeout(GRACE_MS);
}

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
	const [withdrawal] = server.withdrawals();
	expect(withdrawal).toMatchObject({ target: 'shop', body: { key: 'stale_models', acknowledged: false, withdraws: ack.event_id, reason: 'automatic: the condition cleared' } });
	expect(server.targets.shop.acks).toEqual([]);

	server.targets.shop.changed = ['orders'];
	const before = server.reads.shop.answered;
	await page.reload();
	await expect(item(page, '1 node out of date').getByRole('button', { name: 'Acknowledge' })).toBeVisible({ timeout: 20_000 });
	await expect(health(page).getByRole('button', { name: /^Acknowledged/ })).toHaveCount(0);
	await afterReads(page, server, before + 1);
	expect(server.withdrawals()).toHaveLength(1);
});

test('a first read made before the sync status arrives is taken again once it does, and then withdraws', async ({ page }) => {
	const server = new FakeControl({ shop: { changed: ['orders', 'order_items'] } });
	await server.install(page);
	await page.goto('/');
	await acknowledge(page, '2 nodes out of date');
	server.targets.shop.changed = [];
	server.targets.shop.healthDelayMs = 3_000;
	const before = server.reads.shop.answered;
	await page.reload();
	await expect(item(page, 'out of date')).toHaveCount(0, { timeout: 20_000 });
	// Nothing else prompts a read: the page reads again by itself once it knows node state has settled.
	await expect.poll(() => server.withdrawals().length, { timeout: 15_000 }).toBe(1);
	await afterReads(page, server, before + 2);
	expect(server.withdrawals()).toHaveLength(1);
});

test('a read taken during a sync is never judged, even once the sync has settled', async ({ page }) => {
	const server = new FakeControl({ shop: { changed: ['orders', 'order_items'] } });
	await server.install(page);
	await page.goto('/');
	await acknowledge(page, '2 nodes out of date');
	server.targets.shop.changed = [];
	server.targets.shop.sync = 'running';
	const before = server.reads.shop.answered;
	await page.reload();
	await expect(item(page, 'out of date')).toHaveCount(0, { timeout: 20_000 });
	await afterReads(page, server, before + 1);
	expect(server.withdrawals()).toEqual([]);

	// The sync settles, and the nodes are out of date again. The read after it is slow: in that gap the
	// read taken during the sync, which saw them cleared, is still the latest, and must not be judged.
	server.targets.shop.sync = 'settled';
	server.targets.shop.changed = ['orders'];
	server.targets.shop.readDelayMs = 2_000;
	const settledFrom = server.reads.shop.started;
	server.notify('all');
	await expect.poll(() => server.reads.shop.started, { timeout: 15_000 }).toBeGreaterThan(settledFrom);
	// Back, and smaller than when acknowledged, so it shows as acknowledged still.
	await expect(health(page).getByRole('button', { name: /^Acknowledged \(1\)$/ })).toBeVisible({ timeout: 20_000 });
	await afterReads(page, server, server.reads.shop.started);
	expect(server.withdrawals()).toEqual([]);
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
	const before = server.reads.shop.answered;
	server.notify('runs');
	await expect(page.locator('.error-note')).toBeVisible({ timeout: 15_000 });
	await afterReads(page, server, before + 1);
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
	server.targets.returns.readDelayMs = 2_000;
	await page.getByRole('button', { name: 'Switch location' }).click();
	await page.getByRole('menuitemradio', { name: /returns/ }).click();
	await expect(page.getByRole('button', { name: 'Switch location' })).toContainText('returns');
	await expect.poll(() => server.reads.returns.started, { timeout: 15_000 }).toBeGreaterThan(0);
	await afterReads(page, server, 1, 'returns');
	expect(server.withdrawals('returns')).toEqual([]);
	expect(server.withdrawals('shop')).toHaveLength(1);
});

test('a slow read that saw a condition clear withdraws nothing once a newer read shows it back', async ({ page }) => {
	const server = new FakeControl({ shop: { failedRun: true } });
	server.stopScheduler();
	await server.install(page);
	await page.goto('/');
	await acknowledge(page, /scheduler/i);

	// The next read sees the scheduler back, and is answered slowly.
	for (const t of Object.values(server.targets)) t.schedulerLeader = true;
	server.targets.shop.readDelayMs = 2_000;
	const slow = server.reads.shop.started + 1;
	server.notify('runs');
	await expect.poll(() => server.reads.shop.started, { timeout: 15_000 }).toBeGreaterThanOrEqual(slow);
	// Meanwhile it stops again, and acknowledging the failed run reads the Overview again at once,
	// overtaking the slow read, which then arrives with the condition cleared.
	server.stopScheduler();
	server.targets.shop.readDelayMs = 0;
	await item(page, /failed/).getByRole('button', { name: 'Acknowledge' }).click();
	await afterReads(page, server, slow + 1);
	expect(server.writes.filter((w) => w.path === '/acknowledgements')).toHaveLength(1);
	expect(server.withdrawals()).toEqual([]);
});
