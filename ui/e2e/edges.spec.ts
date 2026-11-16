import { expect, test } from '@playwright/test';

// Edges, plain or highlighted, are drawn beneath every card and are hit only on their stroke, so a
// card under an edge can always be clicked. The library raises edges from inside an open box above
// the cards outside it, where they took a card's clicks.
test.use({ viewport: { width: 1600, height: 900 } });

const EDGE = 'group:seeds/marts-group:customers';
const CARD = 'group:orders';

/** The middle of an edge's path, in page coordinates. */
function middle(id: string): { x: number; y: number } {
	const path = document.querySelector<SVGPathElement>(`.svelte-flow__edge[data-id="${id}"] path.svelte-flow__edge-path`)!;
	const m = path.getScreenCTM()!;
	const p = path.getPointAtLength(path.getTotalLength() / 2);
	return { x: m.a * p.x + m.e, y: m.d * p.y + m.f };
}

test('a card under a highlighted edge from an open box is clicked, not the edge', async ({ page }) => {
	await page.goto('/lineage?mock&large');
	await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__node').first().waitFor({ timeout: 20_000 });
	await page.waitForTimeout(800);
	// Open a folder: edges from inside it are raised above the closed cards.
	await page.locator('.svelte-flow__node[data-id="group:seeds"]').click();
	await expect(page.locator(`.svelte-flow__edge[data-id="${EDGE}"]`)).toHaveCount(1);
	await page.waitForTimeout(800);

	// Drag a closed folder's card so its centre lies on that edge.
	const card = page.locator(`.svelte-flow__node[data-id="${CARD}"]`);
	const on = await page.evaluate(middle, EDGE);
	let centre = { x: 0, y: 0 };
	// A drag falls a little short of the pointer; a few drags of what's left bring the card there.
	for (let attempt = 0; attempt < 4; attempt++) {
		const box = (await card.boundingBox())!;
		centre = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
		if (Math.hypot(centre.x - on.x, centre.y - on.y) < 6) break;
		await page.mouse.move(centre.x, centre.y);
		await page.mouse.down();
		for (let i = 1; i <= 10; i++) await page.mouse.move(centre.x + ((on.x - centre.x) * i) / 10, centre.y + ((on.y - centre.y) * i) / 10);
		await page.mouse.up();
		await page.waitForTimeout(300);
	}
	expect(Math.hypot(centre.x - on.x, centre.y - on.y), 'the card lies on the edge').toBeLessThan(6);

	// Hover the edge's source, which highlights it, then click the card's centre: it opens the folder.
	const source = (await page.locator('.svelte-flow__node[data-id="group:seeds/marts"]').boundingBox())!;
	await page.mouse.move(source.x + source.width / 2, source.y + source.height / 2);
	await page.waitForTimeout(200);
	const under = await page.evaluate(({ x, y }) => document.elementFromPoint(x, y)?.closest('.svelte-flow__node, .svelte-flow__edge')?.getAttribute('data-id') ?? '', centre);
	expect(under, 'what is under the card centre').toBe(CARD);
	await page.mouse.click(centre.x, centre.y);
	await expect(page.locator('.svelte-flow__node[data-id="folder:orders"]'), 'the folder opened').toHaveCount(1);
});

test('an edge is hit on its stroke only, never on its bounding box', async ({ page }) => {
	await page.goto('/jobs/warehouse_nightly?mock');
	await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__edge').first().waitFor({ timeout: 20_000 });
	const styles = await page.evaluate(() =>
		[...document.querySelectorAll('.svelte-flow__edge')].flatMap((edge) => [
			['svg', getComputedStyle(edge.closest('svg')!).pointerEvents],
			['edge', getComputedStyle(edge).pointerEvents],
			...[...edge.querySelectorAll('path')].map((p) => [p.getAttribute('class') ?? 'path', getComputedStyle(p).pointerEvents])
		])
	);
	const wrong = styles.filter(([what, value]) => (what === 'svg' ? value !== 'none' : !['stroke', 'visiblestroke'].includes(value.toLowerCase())));
	expect(wrong).toEqual([]);
});
