import { expect, test, type Page } from '@playwright/test';

// The catalog's folders as a tree: clicking anywhere on a folder's label opens or closes it, except on
// its name, which, like each count, opens those nodes in the list instead.
test.use({ viewport: { width: 1600, height: 1000 } });

const open = async (page: Page) => {
	await page.goto('/catalog?mock&large&view=folders');
	const tree = page.getByTestId('folder-tree');
	await expect(tree.locator('tr[data-path]').first()).toBeVisible({ timeout: 20_000 });
	return tree;
};
/** The first top-level folder with subfolders. */
const parentRow = (page: Page) => page.getByTestId('folder-tree').locator('tr[aria-level="1"]', { has: page.getByTestId('folder-toggle') }).first();

test('a click anywhere on a folder’s label, but its name, opens and closes it', async ({ page }) => {
	await open(page);
	const row = parentRow(page);
	const path = await row.getAttribute('data-path');
	const toggle = row.getByTestId('folder-toggle');
	const children = page.locator(`tr[data-path^="${path}/"]`);
	await expect(toggle).toHaveAttribute('aria-expanded', 'false');
	await expect(children).toHaveCount(0);

	// Empty space at the right of the label.
	const label = row.getByTestId('folder-label');
	const box = (await label.boundingBox())!;
	await page.mouse.click(box.x + box.width - 12, box.y + box.height / 2);
	await expect(toggle).toHaveAttribute('aria-expanded', 'true');
	expect(await children.count()).toBeGreaterThan(0);
	expect(await label.evaluate((td) => getComputedStyle(td).cursor)).toBe('pointer');

	// The folder icon.
	await label.locator('svg').nth(1).click();
	await expect(toggle).toHaveAttribute('aria-expanded', 'false');
	// The "N subfolders" text.
	await label.locator('small').click();
	await expect(toggle).toHaveAttribute('aria-expanded', 'true');

	// The chevron is a button that the keyboard works too.
	await toggle.focus();
	await page.keyboard.press('Enter');
	await expect(toggle).toHaveAttribute('aria-expanded', 'false');
	await page.keyboard.press('Space');
	await expect(toggle).toHaveAttribute('aria-expanded', 'true');
});

test('selecting text on a label does not open or close the folder', async ({ page }) => {
	await open(page);
	const row = parentRow(page);
	const small = row.getByTestId('folder-label').locator('small');
	const box = (await small.boundingBox())!;
	await page.mouse.move(box.x + 1, box.y + box.height / 2);
	await page.mouse.down();
	await page.mouse.move(box.x + box.width - 1, box.y + box.height / 2, { steps: 5 });
	await page.mouse.up();
	expect(await page.evaluate(() => window.getSelection()?.toString().length ?? 0)).toBeGreaterThan(0);
	await expect(row.getByTestId('folder-toggle')).toHaveAttribute('aria-expanded', 'false');
});

test('a folder’s name opens its nodes in the list, without opening or closing it', async ({ page }) => {
	await open(page);
	const row = parentRow(page);
	const path = (await row.getAttribute('data-path'))!;
	// Watch the toggle: the tree goes away once the list opens, so a toggle would only show as a change.
	await row.getByTestId('folder-toggle').evaluate((button) => {
		const w = window as unknown as { toggled: number };
		w.toggled = 0;
		new MutationObserver(() => (w.toggled += 1)).observe(button, { attributes: true, attributeFilter: ['aria-expanded'] });
	});
	// Opened in a new tab, the tree stays, so a toggle would show directly.
	const [tab] = await Promise.all([page.context().waitForEvent('page'), row.getByTestId('folder-name').click({ modifiers: ['Control'] })]);
	await tab.close();
	await expect(row.getByTestId('folder-toggle')).toHaveAttribute('aria-expanded', 'false');
	await row.getByTestId('folder-name').click();
	await expect(page.getByTestId('folder-tree')).toHaveCount(0);
	expect(await page.evaluate(() => (window as unknown as { toggled: number }).toggled)).toBe(0);
	await expect(page).not.toHaveURL(/view=folders/);
	// The selection is evaluated a moment later; then the list holds just the folder's nodes.
	await expect(page.getByText(/^\d+ of \d+ nodes$/)).toHaveText(/^(\d+) of (?!\1 )\d+ nodes$/);
	const shown = (await page.locator('table.catalog tbody tr .cell-name a').allInnerTexts()).map((s) => s.trim());
	expect(shown.length).toBeGreaterThan(0);
	expect(shown.filter((name) => !name.startsWith(path.split('/')[0])), path).toEqual([]);
});

test('a folder’s count opens just those nodes', async ({ page }) => {
	const tree = await open(page);
	const count = tree.getByTestId('folder-count-changed').first();
	const n = Number(await count.innerText());
	await count.click();
	await expect(page.getByTestId('folder-tree')).toHaveCount(0);
	await expect(page.getByText(new RegExp(`^${n} of \\d+ nodes$`))).toBeVisible();
	await expect(page.locator('table.catalog tbody tr')).toHaveCount(n);
});
