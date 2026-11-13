// The ad-hoc verbs and flags, generated from the CLI's command table and ad-hoc
// allowlist by `npm run gen:command-table`. Never hand-edit the generated JSON.

import type { Capability, LaunchVerb } from '$lib/api';
import table from './command-table.generated.json';

/** `attached` takes a value only as `--flag=value`; a bare flag is a switch. */
export type FlagType = 'bool' | 'attached' | 'string' | 'int' | 'timestamp' | 'selector';

export type FlagSpec = {
	flag: string;
	aliases: string[];
	type: FlagType;
	choices: string[] | null;
	repeatable: boolean;
	/** The permission beyond run.adhoc that setting this flag needs, as the server decides it. */
	requires: Capability | null;
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
