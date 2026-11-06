// The launchable verbs and flags, generated from the CLI's command table by
// `npm run gen:command-table`. Never hand-edit the generated JSON.

import type { LaunchVerb } from '$lib/api';
import table from './command-table.generated.json';

export type FlagType = 'bool' | 'string' | 'int' | 'timestamp' | 'selector';

export type FlagSpec = {
	flag: string;
	aliases: string[];
	type: FlagType;
	choices: string[] | null;
	repeatable: boolean;
	rewritesHistory: boolean;
	description: string;
};

export type VerbSpec = { name: LaunchVerb; description: string; flags: FlagSpec[] };

export const VERBS = table.verbs as VerbSpec[];

export function verbSpec(name: LaunchVerb): VerbSpec {
	return VERBS.find((v) => v.name === name) ?? VERBS[0];
}

export function flagSpec(verb: LaunchVerb, flag: string): FlagSpec | undefined {
	return verbSpec(verb).flags.find((f) => f.flag === flag || f.aliases.includes(flag));
}
