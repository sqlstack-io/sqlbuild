// Lays out the static demo for a host without an SPA fallback, such as GitHub Pages.
//
//   node scripts/demo-pages.mjs <ui build> <recording.json.gz> <destination>
//
// Copies the build built with SQB_UI_BASE=/demo VITE_SQB_DEMO=1 and the recording, then writes
// the app page as `<page>.html` for every page and recorded item, so an address opened directly
// or reloaded loads the app instead of the site's 404 page.

import { cpSync, mkdirSync, readFileSync, readdirSync, statSync, writeFileSync } from 'node:fs';
import { dirname, join, relative } from 'node:path';
import { gunzipSync } from 'node:zlib';
import { fileURLToPath } from 'node:url';

const [build, recordingPath, destination] = process.argv.slice(2);
if (!build || !recordingPath || !destination) {
	console.error('usage: node scripts/demo-pages.mjs <ui build> <recording.json.gz> <destination>');
	process.exit(2);
}

/** Pages without a parameter, read from the routes, such as `runs` or `admin/audit`. */
function staticPages(directory, prefix = '') {
	return readdirSync(directory).flatMap((name) => {
		const path = join(directory, name);
		if (!statSync(path).isDirectory() || name.startsWith('[')) return [];
		const page = prefix ? `${prefix}/${name}` : name;
		const here = readdirSync(path).includes('+page.svelte') ? [page] : [];
		return [...here, ...staticPages(path, page)];
	});
}

/** Item pages, from the lists the recording answers: every job, run and trigger the demo links to. */
const ITEM_LISTS = [
	[/\/targets\/[^/]+\/jobs$/, 'jobs', (item) => item.job_key],
	[/\/targets\/[^/]+\/runs\?/, 'runs', (item) => item.run_id],
	[/\/targets\/[^/]+\/triggers$/, 'triggers', (item) => item.trigger_id],
];
/** Catalog and check pages only for the items the recording holds, since there are hundreds. */
const RECORDED_NODE = /\/targets\/[^/]+\/nodes\/([^/?]+)\/sql$/;
const RECORDED_CHECK = /\/targets\/[^/]+\/checks\/([^/?]+)$/;

const recording = JSON.parse(gunzipSync(readFileSync(recordingPath)).toString('utf8'));
const items = new Set();
for (const { url, text } of recording.responses) {
	const node = url.match(RECORDED_NODE);
	if (node) items.add(`catalog/${decodeURIComponent(node[1])}`);
	const check = url.match(RECORDED_CHECK);
	if (check) items.add(`checks/${decodeURIComponent(check[1])}`);
	for (const [pattern, page, key] of ITEM_LISTS) {
		if (!pattern.test(url)) continue;
		const list = JSON.parse(text);
		for (const item of Array.isArray(list) ? list : []) if (key(item)) items.add(`${page}/${key(item)}`);
	}
}

const routes = join(dirname(fileURLToPath(import.meta.url)), '../src/routes');
mkdirSync(destination, { recursive: true });
cpSync(build, destination, { recursive: true });
cpSync(recordingPath, join(destination, 'demo-recording.json.gz'));
const app = readFileSync(join(build, 'index.html'));
const pages = [...staticPages(routes).filter((page) => page !== 'login'), ...items];
// Hosts differ on whether `/runs` reads `runs.html` or `runs/index.html`, so both are written.
for (const page of pages) {
	for (const file of [join(destination, `${page}.html`), join(destination, page, 'index.html')]) {
		mkdirSync(dirname(file), { recursive: true });
		writeFileSync(file, app);
	}
}
console.log(`Wrote the demo to ${relative(process.cwd(), destination) || '.'}: ${pages.length} pages.`);
