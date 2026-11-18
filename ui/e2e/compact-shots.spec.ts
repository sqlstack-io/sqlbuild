import { test, type Page } from '@playwright/test';
import { zoomTo } from './zoom';

// Photographs of the zoomed-out pills, and how many names are cut off, in the pills' font (IBM Plex
// Mono) and in Inter, for choosing between them. Runs only with SHOTS set:
//   SHOTS=/some/dir npx playwright test e2e/compact-shots.spec.ts --project=layout
const shots = process.env.SHOTS;
test.use({ viewport: { width: 1600, height: 900 } });

const PAGES = [
	{ name: 'group', path: '/jobs/warehouse_nightly?mock' },
	{ name: 'group-open', path: '/jobs/warehouse_nightly?mock', click: 'Expand all' },
	{ name: 'lineage', path: '/lineage?mock&large' },
	{ name: 'lineage-open', path: '/lineage?mock&large', click: 'Expand all' }
];

/** Pills wholly in view, and those whose name is cut off. */
const cutNames = () => {
	const flow = document.querySelector('.svelte-flow')!.getBoundingClientRect();
	let shown = 0;
	let cut = 0;
	for (const pill of document.querySelectorAll<HTMLElement>('.cface .pill')) {
		const b = pill.getBoundingClientRect();
		if (b.left < flow.left || b.right > flow.right || b.top < flow.top || b.bottom > flow.bottom) continue;
		const name = [...pill.querySelectorAll<HTMLElement>('.nm')].find((n) => getComputedStyle(n).display !== 'none');
		if (!name) continue;
		shown++;
		if (name.scrollWidth > name.clientWidth + 0.5) cut++;
	}
	return { shown, cut };
};

async function open(page: Page, path: string, click: string | undefined, font: 'mono' | 'inter') {
	// Set once the page is parsed and before the app's scripts measure anything.
	if (font === 'inter')
		await page.addInitScript(() =>
			document.addEventListener('readystatechange', () => {
				if (document.readyState === 'interactive') document.head.appendChild(Object.assign(document.createElement('style'), { textContent: 'html:root { --font-node-compact: var(--font-sans); }' }));
			})
		);
	await page.goto(path);
	await page.locator('.graph-canvas:not(.unfitted) .svelte-flow__node').first().waitFor({ timeout: 20_000 });
	if (click) await page.getByRole('button', { name: click }).click();
	await page.waitForTimeout(800);
}

for (const { name, path, click } of PAGES)
	for (const font of ['mono', 'inter'] as const)
		test(`${name} in ${font}`, async ({ page }) => {
			test.skip(!shots, 'SHOTS is not set');
			test.setTimeout(120_000);
			await open(page, path, click, font);
			const counts: string[] = [];
			for (const zoom of [0.7, 0.5]) {
				await zoomTo(page, zoom);
				await page.evaluate(() => document.fonts.ready);
				await page.waitForTimeout(300);
				const { shown, cut } = await page.evaluate(cutNames);
				counts.push(`${zoom}: ${cut} of ${shown} cut`);
				const z = `z0${Math.round(zoom * 100)}`;
				if (font === 'mono') await page.screenshot({ path: `${shots}/b2-${name}-${z}.png` });
				if (zoom === 0.5) await page.screenshot({ path: `${shots}/b2-${name}-${z}-${font}.png` });
			}
			console.info(`CUT ${name} ${font}: ${counts.join('; ')}`);
		});

test.describe('close-up', () => {
	// Four device pixels to each pixel, so the ring and the triangle's centring can be seen.
	test.use({ deviceScaleFactor: 4 });
	test('of a job pill', async ({ page }) => {
		test.skip(!shots, 'SHOTS is not set');
		await open(page, '/jobs/warehouse_nightly?mock', undefined, 'mono');
		await zoomTo(page, 0.7);
		const pill = page.locator('.cface .pill[data-kind="job"]').first();
		const b = (await pill.boundingBox())!;
		await page.screenshot({ path: `${shots}/b2-job-pill-closeup.png`, clip: { x: b.x - 10, y: b.y - 10, width: b.width + 20, height: b.height + 20 } });
	});
});
