// Records the static demo from a click-through of the UI on `sqb dev`.
//
//   node scripts/record-demo.mjs <recording.json.gz> --server <url> [--replace FROM=TO]... <session.har>...
//
// The browser's HARs name the requests the pages make; each API read is asked again of the
// `sqb dev` server at --server, because a HAR often drops response bodies, and so are reads sent
// as POST, such as plan previews. `--replace` rewrites text everywhere, for example the project path.

import { readFileSync, writeFileSync } from 'node:fs';
import { gzipSync } from 'node:zlib';

const READ_POSTS = [/\/plans$/, /\/runs\/adhoc\/check$/];
// The location's change feed is a live stream; a trigger's change history is an ordinary read.
const SKIPPED = [/^\/api\/auth\//, /\/targets\/[^/]+\/changes$/, /^\/api\/openapi\.json$/];
const AUDIT = '/api/control/audit';
const USAGE = 'usage: node scripts/record-demo.mjs <recording.json.gz> --server <url> [--replace FROM=TO]... <session.har>...';

const [outPath, ...options] = process.argv.slice(2);
let server = null;
const replacements = [];
const hars = [];
for (let i = 0; i < options.length; i++) {
	const value = options[i + 1];
	if (options[i] === '--server' && value) server = options[++i].replace(/\/$/, '');
	else if (options[i] === '--replace' && value?.includes('=')) replacements.push([value.slice(0, value.indexOf('=')), options[++i].slice(value.indexOf('=') + 1)]);
	else if (!options[i].startsWith('--')) hars.push(options[i]);
	else server = undefined;
}
if (!outPath || !server || !hars.length) {
	console.error(USAGE);
	process.exit(2);
}
function keep(entry) {
	kept.set(JSON.stringify([entry.method, entry.url, entry.body ?? null]), entry);
}
const scrub = (text) => replacements.reduce((out, [from, to]) => out.split(from).join(to), text);

const kept = new Map();
let auditLimit = null;
for (const { request, response } of hars.flatMap((har) => JSON.parse(readFileSync(har, 'utf8')).log.entries)) {
	const url = new URL(request.url);
	const path = url.pathname + url.search;
	if (!url.pathname.startsWith('/api/') || SKIPPED.some((pattern) => pattern.test(url.pathname))) continue;
	if (url.pathname === AUDIT) {
		auditLimit = url.searchParams.get('limit');
		continue;
	}
	const body = request.postData?.text ? JSON.parse(request.postData.text) : undefined;
	let answer;
	if (request.method === 'GET') {
		const fresh = await fetch(server + path);
		answer = { status: fresh.status, contentType: fresh.headers.get('content-type') ?? 'application/json', text: await fresh.text() };
	} else if (READ_POSTS.some((pattern) => pattern.test(url.pathname))) {
		// A read sent as POST is asked again too; a plan request already taken keeps the HAR's answer.
		const fresh = await fetch(server + path, { method: 'POST', headers: { 'content-type': 'application/json' }, body: request.postData?.text });
		if (fresh.ok) answer = { status: fresh.status, contentType: fresh.headers.get('content-type') ?? 'application/json', text: await fresh.text() };
		else if (response.content?.text) answer = { status: response.status, contentType: response.content.mimeType || 'application/json', text: response.content.text };
		else continue;
	} else continue;
	if (answer.status >= 500) throw new Error(`${server}${path} answered ${answer.status}`);
	const entry = {
		method: request.method,
		url: scrub(path),
		...(body === undefined ? {} : { body: JSON.parse(scrub(JSON.stringify(body))) }),
		...answer,
		text: scrub(answer.text)
	};
	keep(entry);
}

// The plan previews asked again above add audit entries, so the browser's later audit pages start
// at the wrong entry. The audit log is read last, following its pages from the newest entry.
if (auditLimit) {
	for (let before = Number.MAX_SAFE_INTEGER; before; ) {
		const path = `${AUDIT}?before=${before}&limit=${auditLimit}`;
		const fresh = await fetch(server + path);
		const text = await fresh.text();
		if (!fresh.ok) throw new Error(`${server}${path} answered ${fresh.status}`);
		keep({ method: 'GET', url: path, status: fresh.status, contentType: fresh.headers.get('content-type') ?? 'application/json', text: scrub(text) });
		const page = JSON.parse(text);
		before = page.length === Number(auditLimit) ? page[page.length - 1].storage_id : 0;
	}
}

const responses = [...kept.values()].sort((a, b) => a.url.localeCompare(b.url) || a.method.localeCompare(b.method));
// Run output and statements make the recording several megabytes of text, so it is stored packed.
writeFileSync(outPath, gzipSync(`${JSON.stringify({ recordedAt: new Date().toISOString(), responses })}\n`, { level: 9 }));
console.log(`Recorded ${responses.length} responses to ${outPath}.`);
