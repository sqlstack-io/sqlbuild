#!/usr/bin/env node
// Extracts the plan page's verbs and flags from the CLI's command table, so the
// panel offers exactly the flags an ad-hoc run accepts.
//
//   node scripts/extract-command-table.mjs           # rewrite the generated file
//   node scripts/extract-command-table.mjs --check   # fail if it is out of date
//
// The verbs and accepted destinations come from the CLI's ad-hoc allowlist, and
// the permissions a flag needs come from the control plane's rules, so the panel
// cannot offer a flag the server refuses or hide a gate it applies.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const SOURCE = resolve(here, '../../crates/sqlbuild-cli/src/argv/command_table.json');
const ALLOWLIST = resolve(here, '../../crates/sqlbuild-cli/src/adhoc/constants.rs');
const GATES = resolve(here, '../../crates/sqlbuild-orchestration/src/runs/_helpers/requests/permissions.rs');
const OUTPUT = resolve(here, '../src/lib/run/command-table.generated.json');

/** The panel's one-line description of each verb the allowlist accepts. */
const VERB_DESCRIPTIONS = {
	build: 'Build the selected nodes, running their tests and audits',
	test: 'Run unit and scenario tests for the selection',
	seed: 'Load seed files',
	audit: 'Evaluate audits against what is already built',
	freshness: 'Check source freshness'
};

/** Flags the planner refuses although their destination is accepted: `--no-load` shares `--load`'s. */
const REFUSED_FLAGS = ['--load'];

/** Descriptions for flags the table leaves undocumented. */
const DESCRIPTIONS = {
	'--select': 'Selector expression, e.g. +fact_orders, tag:finance, path:models/marts/',
	'--exclude': 'Selector expression to leave out',
	'--no-python': 'Skip Python nodes',
	'--no-tests': 'Skip unit tests during the build',
	'--no-audits': 'Skip audits during the build',
	'--max-microbatches': 'Process at most this many microbatches per incremental model',
	'--no-load': 'Skip loaders',
	'--fail-fast': 'Stop at the first failed node',
	'--full-refresh': 'Rebuild incremental models and snapshots from scratch',
	'--allow-snapshot-full-refresh': 'Allow a full refresh of snapshot models',
	'--allow-table-type-downgrade': 'Allow replacing a table with a less durable table type',
	'--allow-retention-decrease': 'Allow lowering a table retention setting',
	'--allow-snapshot-schema-change': 'Allow a schema change on snapshot models',
	'--concurrency': 'Nodes to run in parallel within this run',
	'--verbose': 'More detailed output',
	'--warehouse': 'Warehouse for this run, overriding the target default',
	'--case': 'Run one named parameter case',
	'--state': 'Record freshness results in the state tables',
	'--fail-on-error': 'Fail when a freshness check errors',
	'--fail-on-stale': 'Fail when a source is stale'
};

/** The quoted strings of a Rust `const NAME: [...] = [...];` array, which must exist. */
function rustStrings(path, name) {
	const source = readFileSync(path, 'utf8');
	const match = source.match(new RegExp(`const ${name}: \\[[^\\]]+\\] = \\[([\\s\\S]*?)\\];`));
	if (!match) throw new Error(`${name} is not a string array in ${path}`);
	return [...match[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

const ADHOC_VERBS = rustStrings(ALLOWLIST, 'ADHOC_VERBS');
const ACCEPTED = new Set([...rustStrings(ALLOWLIST, 'ALLOWED_DESTS'), ...rustStrings(ALLOWLIST, 'SELECTOR_DESTS')]);
/** Destinations whose setting needs a permission beyond run.adhoc, as the control plane decides it. */
const REQUIRES = new Map([
	...['full_refresh', ...rustStrings(GATES, 'CURSOR_SETTINGS'), ...rustStrings(GATES, 'ADHOC_DESTRUCTIVE_SETTINGS')].map((dest) => [dest, 'run.rewrite_history']),
	['warehouse', 'run.warehouse']
]);

function valueType(action) {
	const flag = action.options[0];
	if (action.kind === 'store_true' || action.kind === 'store_false') return 'bool';
	if (flag.endsWith('-ts')) return 'timestamp';
	if (flag.endsWith('-int') || flag === '--max-microbatches' || flag === '--concurrency') return 'int';
	if (action.kind === 'extend') return 'selector';
	return 'string';
}

function extract(table) {
	const parsers = table.actions.find((a) => a.kind === 'parsers')?.parsers ?? [];
	const known = new Set();
	const verbs = ADHOC_VERBS.map((name) => {
		const description = VERB_DESCRIPTIONS[name];
		if (!description) throw new Error(`Verb ${name} has no panel description`);
		const parser = parsers.find((p) => p.name === name);
		if (!parser) throw new Error(`Verb ${name} is not in the command table`);
		const flags = [];
		for (const action of parser.parser.actions) {
			if (!action.options) continue;
			for (const option of action.options) known.add(option);
			const flag = action.options.find((o) => o.startsWith('--'));
			if (!flag || !ACCEPTED.has(action.dest) || REFUSED_FLAGS.includes(flag)) continue;
			const help = typeof action.help === 'string' ? action.help : null;
			if (action.help && typeof action.help === 'object') continue;
			flags.push({
				flag,
				aliases: action.options.filter((o) => o !== flag),
				type: valueType(action),
				choices: action.choices ?? null,
				repeatable: action.kind === 'extend' || action.kind === 'append',
				requires: REQUIRES.get(action.dest) ?? null,
				description: help ?? DESCRIPTIONS[flag] ?? ''
			});
		}
		return { name, description, flags };
	});
	for (const flag of [...REFUSED_FLAGS, ...Object.keys(DESCRIPTIONS)]) {
		if (!known.has(flag)) throw new Error(`${flag} is described here but not in the command table for any ad-hoc verb`);
	}
	for (const dest of REQUIRES.keys()) {
		if (!ACCEPTED.has(dest)) throw new Error(`${dest} is gated by the control plane but not accepted by the ad-hoc allowlist`);
	}
	return { source: 'crates/sqlbuild-cli/src/argv/command_table.json', verbs };
}

const generated = JSON.stringify(extract(JSON.parse(readFileSync(SOURCE, 'utf8'))), null, '\t') + '\n';
if (process.argv.includes('--check')) {
	const current = readFileSync(OUTPUT, 'utf8');
	if (current !== generated) {
		console.error('src/lib/run/command-table.generated.json is out of date; run npm run gen:command-table');
		process.exit(1);
	}
} else {
	writeFileSync(OUTPUT, generated);
}
