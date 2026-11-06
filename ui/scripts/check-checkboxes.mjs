// Fails when a native checkbox is missing the shared `sb-check` style.
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';

const root = new URL('../src', import.meta.url).pathname;
const failures = [];

function walk(dir) {
	for (const name of readdirSync(dir)) {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) walk(path);
		else if (path.endsWith('.svelte')) check(path);
	}
}

function check(path) {
	const lines = readFileSync(path, 'utf8').split('\n');
	lines.forEach((line, index) => {
		if (line.includes('type="checkbox"') && !/class="[^"]*\bsb-check\b/.test(line)) {
			failures.push(`${relative(process.cwd(), path)}:${index + 1}`);
		}
	});
}

walk(root);
if (failures.length > 0) {
	console.error('Checkboxes without the shared sb-check style:');
	for (const failure of failures) console.error(`  ${failure}`);
	process.exit(1);
}
console.log('All checkboxes use sb-check.');
