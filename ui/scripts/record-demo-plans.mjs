// Records the static demo's plan previews again from DuckShop on `sqb dev`, through the server's real
// plan reads: each recorded plan command is planned anew, and its summary, every page of its sections
// and its models' diffs replace the recorded answers. The rest of the recording is kept.
//
//   node scripts/record-demo-plans.mjs <recording.json.gz> --server <url> [--replace FROM=TO]... [--ask FROM=TO]...
//
// The server's location and request ids are written as the recording's, so the replay matches the
// demo's plan requests as before. `--replace` rewrites text everywhere, as record-demo.mjs does;
// `--ask` rewrites only the command sent to the server, such as a job renamed since the recording.

import { readFileSync, writeFileSync } from 'node:fs';
import { gunzipSync, gzipSync } from 'node:zlib';
import { randomUUID } from 'node:crypto';

const USAGE = 'usage: node scripts/record-demo-plans.mjs <recording.json.gz> --server <url> [--replace FROM=TO]... [--ask FROM=TO]...';
const [path, ...options] = process.argv.slice(2);
let server = null;
const replacements = [];
const asked = [];
for (let i = 0; i < options.length; i++) {
	const value = options[i + 1];
	if (options[i] === '--server' && value) server = options[++i].replace(/\/$/, '');
	else if (options[i] === '--replace' && value?.includes('=')) replacements.push([value.slice(0, value.indexOf('=')), options[++i].slice(value.indexOf('=') + 1)]);
	else if (options[i] === '--ask' && value?.includes('=')) asked.push([value.slice(0, value.indexOf('=')), options[++i].slice(value.indexOf('=') + 1)]);
	else {
		console.error(USAGE);
		process.exit(2);
	}
}
if (!path || !server) {
	console.error(USAGE);
	process.exit(2);
}

/** Rows each section page read asks for: the plan page's pages, and the Checks tab's larger reads. */
const PAGE_SIZES = [50, 1000];
const PLANS = /^\/api\/control\/locations\/([^/]+)\/targets\/([^/]+)\/plans$/;
const POLL_MS = 500;

const recording = JSON.parse(gunzipSync(readFileSync(path)).toString('utf8'));
const asks = recording.responses.filter((r) => r.method === 'POST' && PLANS.test(r.url));
if (!asks.length) throw new Error('The recording has no plan requests to record again');

const get = async (url) => {
	const response = await fetch(server + url);
	if (response.status >= 500) throw new Error(`${url} answered ${response.status}`);
	return { status: response.status, contentType: response.headers.get('content-type') ?? 'application/json', text: await response.text() };
};
const bootstrap = JSON.parse((await get('/api/control/bootstrap')).text);
const location = bootstrap.access.locations[0].location_id;

const fresh = [];
for (const ask of asks) {
	const [, recordedLocation, target] = PLANS.exec(ask.url);
	const recordedId = ask.body.request_id;
	const id = randomUUID();
	const base = `/api/control/locations/${location}/targets/${target}/plans`;
	const posted = await fetch(server + base, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ ...ask.body, command: ask.body.command.map((token) => asked.reduce((out, [from, to]) => (out === from ? to : out), token)), request_id: id }) });
	if (!posted.ok) throw new Error(`planning ${JSON.stringify(ask.body.command)} answered ${posted.status}: ${await posted.text()}`);
	let summary;
	for (;;) {
		summary = await get(`${base}/${id}/summary`);
		if (summary.status !== 200) throw new Error(`the summary of ${JSON.stringify(ask.body.command)} answered ${summary.status}: ${summary.text}`);
		if (JSON.parse(summary.text).outcome) break;
		await new Promise((resolve) => setTimeout(resolve, POLL_MS));
	}
	// Written as the recording's location and request, so the replay's id mapping finds them.
	const ours = (text) => replacements.reduce((out, [from, to]) => out.split(from).join(to), text.split(location).join(recordedLocation).split(id).join(recordedId));
	const keep = (url, answer) => fresh.push({ method: 'GET', url: ours(url), ...answer, text: ours(answer.text) });
	keep(`${base}/${id}/summary`, summary);
	const document = JSON.parse(summary.text).outcome.document ?? {};
	if (document.error || !document.sections) throw new Error(`${ask.body.command.join(' ')} planned no summary: ${summary.text.slice(0, 400)}`);
	const models = new Set();
	for (const { section, total } of document.sections ?? []) {
		for (const limit of PAGE_SIZES) {
			for (let offset = 0; offset < total; offset += limit) {
				const url = `${base}/${id}/rows?section=${section}&offset=${offset}&limit=${limit}`;
				const page = await get(url);
				keep(url, page);
				for (const row of JSON.parse(page.text).rows ?? []) if (row && typeof row === 'object' && row.diff_stats) models.add(row.name);
			}
		}
	}
	for (const model of models) {
		const url = `${base}/${id}/diff?model=${encodeURIComponent(model)}`;
		keep(url, await get(url));
	}
	console.log(`Planned ${ask.body.command.join(' ')}: ${(document.sections ?? []).length} sections, ${models.size} diffs.`);
}

// The plan reads the page no longer makes go; every plan read is the fresh one.
const planRead = /\/plans\/[^/?]+(\/(summary|rows|diff))?(\?|$)/;
const kept = recording.responses.filter((r) => !(r.method === 'GET' && planRead.test(r.url)));
const responses = [...kept, ...fresh].sort((a, b) => a.url.localeCompare(b.url) || a.method.localeCompare(b.method));
writeFileSync(path, gzipSync(`${JSON.stringify({ recordedAt: recording.recordedAt, responses })}\n`, { level: 9 }));
console.log(`Recorded ${fresh.length} plan reads again; ${responses.length} responses in ${path}.`);
