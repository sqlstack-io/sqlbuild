#!/usr/bin/env node
// Extracts the run panel's verbs and flags from the CLI's command table, so the
// panel offers exactly the flags `sqb` accepts.
//
//   node scripts/extract-command-table.mjs           # rewrite the generated file
//   node scripts/extract-command-table.mjs --check   # fail if it is out of date
//
// Flags the server controls or that only make sense on a local shell (target,
// output formats, project paths, dbt interop paths) are left out. Every name in
// the lists below must exist in the table, so a renamed flag fails the script
// instead of silently drifting.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const SOURCE = resolve(here, '../../crates/sqlbuild-cli/src/argv/command_table.json');
const OUTPUT = resolve(here, '../src/lib/run/command-table.generated.json');

/** Verbs a server run can launch, with the panel's one-line descriptions. */
const VERBS = {
	build: 'Build the selected nodes, running their tests and audits',
	test: 'Run unit and scenario tests for the selection',
	seed: 'Load seed files',
	load: 'Run loaders into their source tables',
	audit: 'Evaluate audits against what is already built',
	freshness: 'Check source freshness'
};

/** Set by the server from the location's target, the run token and the event stream. */
const SERVER_CONTROLLED = [
	'--help',
	'--target',
	'--json',
	'--json-output',
	'--event-output',
	'--project-dir',
	'--no-color',
	'--debug',
	'--manifest',
	'--select-file',
	'--selection-diagnostics',
	'--dbt-project-dir',
	'--dbt-profiles-dir',
	'--dbt-target',
	'--dbt-target-path',
	'--since',
	'--explain',
	'--inspect'
];

/** Flags that rewrite history and need `run.rewrite_history`. */
const REWRITES_HISTORY = [
	'--full-refresh',
	'--allow-snapshot-full-refresh',
	'--reload',
	'--start-cursor',
	'--end-cursor',
	'--start-cursor-ts',
	'--end-cursor-ts',
	'--start-cursor-int',
	'--end-cursor-int'
];

/** Descriptions for flags the table leaves undocumented. */
const DESCRIPTIONS = {
	'--select': 'Selector expression, e.g. +fact_orders, tag:finance, group:marts',
	'--exclude': 'Selector expression to leave out',
	'--defer-to': 'Read unselected upstream nodes from another target',
	'--defer-clone-from': 'Clone unselected upstream nodes from another target',
	'--defer-sources-to': 'Read sources from another target',
	'--no-python': 'Skip Python nodes',
	'--no-tests': 'Skip unit tests during the build',
	'--no-audits': 'Skip audits during the build',
	'--max-microbatches': 'Process at most this many microbatches per incremental model',
	'--load': 'Run loaders as part of the build',
	'--no-load': 'Skip loaders',
	'--reload': 'Reload loader sources from the beginning',
	'--fail-fast': 'Stop at the first failed node',
	'--full-refresh': 'Rebuild incremental models and snapshots from scratch',
	'--allow-snapshot-full-refresh': 'Allow a full refresh of snapshot models',
	'--allow-table-type-downgrade': 'Allow replacing a table with a less durable table type',
	'--allow-retention-decrease': 'Allow lowering a table retention setting',
	'--allow-missing-migration-origin': 'Allow a migration whose origin relation no longer exists',
	'--allow-snapshot-schema-change': 'Allow a schema change on snapshot models',
	'--concurrency': 'Nodes to run in parallel within this run',
	'--verbose': 'More detailed output',
	'--vars': 'Project variables as a JSON or key=value mapping',
	'--warehouse': 'Warehouse for this run, overriding the target default',
	'--no-sql-analysis': 'Disable SQL syntax, binding and type validation',
	'--no-cache': 'Bypass the compile analysis cache',
	'--case': 'Run one named parameter case',
	'--state': 'Record freshness results in the state tables',
	'--fail-on-error': 'Fail when a freshness check errors',
	'--fail-on-stale': 'Fail when a source is stale'
};

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
	const verbs = Object.entries(VERBS).map(([name, description]) => {
		const parser = parsers.find((p) => p.name === name);
		if (!parser) throw new Error(`Verb ${name} is not in the command table`);
		const flags = [];
		for (const action of parser.parser.actions) {
			if (!action.options) continue;
			for (const option of action.options) known.add(option);
			const flag = action.options.find((o) => o.startsWith('--'));
			if (!flag || action.options.some((o) => SERVER_CONTROLLED.includes(o))) continue;
			const help = typeof action.help === 'string' ? action.help : null;
			if (action.help && typeof action.help === 'object') continue;
			flags.push({
				flag,
				aliases: action.options.filter((o) => o !== flag),
				type: valueType(action),
				choices: action.choices ?? null,
				repeatable: action.kind === 'extend' || action.kind === 'append',
				rewritesHistory: REWRITES_HISTORY.includes(flag),
				description: help ?? DESCRIPTIONS[flag] ?? ''
			});
		}
		return { name, description, flags };
	});
	for (const flag of [...SERVER_CONTROLLED, ...REWRITES_HISTORY, ...Object.keys(DESCRIPTIONS)]) {
		if (!known.has(flag)) throw new Error(`${flag} is listed here but not in the command table for any launchable verb`);
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
