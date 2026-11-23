import { describe, expect, it } from 'vitest';
import { check, collapse, findings, findCopy } from './check';

const site = (kind: 'label' | 'value' | 'note' | 'empty', text: string, title?: string) => ({ file: 'x.svelte', line: 1, kind, text, title });

describe('copy rules', () => {
	it('keep labels, values and notes short', () => {
		expect(check(site('label', 'Last tick'))).toEqual([]);
		expect(check(site('label', 'What happened to the data'))).toEqual(['label over 4 words']);
		expect(check(site('value', '{n} incomplete'))).toEqual([]);
		expect(check(site('note', 'Sample rows'))).toEqual([]);
	});

	it('ban narration, clauses and too many segments', () => {
		expect(check(site('value', 'Fails if any rows break'))).toContain('says "fails if"');
		expect(check(site('value', 'Kept; removed later'))).toContain('has a ";" clause');
		expect(check(site('value', 'Waiting: run is busy'))).toContain('has a ": clause"');
		expect(check(site('value', 'a · b · c · d'))).toContain('more than 3 " · " segments');
		expect(check(site('value', 'Your access'))).toContain('addresses the reader');
	});

	it('stops a note restating its title', () => {
		expect(check(site('note', 'Recent results shown', 'Recent results'))).toEqual(['note restates the title']);
	});

	it('lets an empty state have a second sentence only for a consequence', () => {
		expect(check(site('empty', 'No runs yet.'))).toEqual([]);
		expect(check(site('empty', 'No jobs yet. Declare them in code.'))).toEqual(['second sentence without a consequence']);
		expect(check(site('empty', 'Nothing kept. Old data is lost.'))).toEqual([]);
	});

	it('collapses template tokens', () => {
		expect(collapse('Version {n} · ${ago}')).toBe('Version # · #');
	});
});

describe('copy sites and exceptions', () => {
	it('finds panel notes, empty states, headers and label-map values', () => {
		const svelte = '<Panel title="Evidence" note="Sample rows stored with the result">\n<DataTable empty={a ? \'No checks.\' : \'No checks yet. Add audits and deploy.\'} />\n<th>Why it waits for things</th>';
		expect(findCopy('a.svelte', svelte).map((s) => [s.kind, s.text])).toEqual([
			['label', 'Evidence'],
			['note', 'Sample rows stored with the result'],
			['empty', 'No checks.'],
			['empty', 'No checks yet. Add audits and deploy.'],
			['label', 'Why it waits for things']
		]);
		const labels = "export const kindLabel: Record<K, string> = {\n\ta: 'Fails if any rows break the rule'\n};\nexport const kindHelp: Record<K, string> = {\n\ta: 'Explains at length, as help may'\n};";
		expect(findCopy('labels.ts', labels, true).map((s) => s.text)).toEqual(['Fails if any rows break the rule']);
	});

	it('reads an empty state with nested elements as one text, without its buttons and help', () => {
		const block =
			'<div class="empty">\n<BotIcon />\n<div class="empty-title">No service accounts yet</div>\n<p>A service account lets a script call SQLBuild.</p>\n{#if canAdd}<Button>Add service account</Button>{/if}\n<HelpTip><code>sqb admin</code></HelpTip>\n</div>';
		expect(findCopy('a.svelte', block).map((s) => [s.kind, s.text])).toEqual([['empty', 'No service accounts yet A service account lets a script call SQLBuild.']]);
		const branches = '<p class="empty">{#if failed}Couldn\'t load: {error}{:else}No runs in {`${days} days`}.{/if}</p>';
		expect(findCopy('a.svelte', branches).map((s) => s.text)).toEqual(["Couldn't load: #", 'No runs in #.']);
		expect(findings('a.svelte', block).findings.map((f) => f.rule)).toContain('empty state over 8 words');
	});

	it('honours copy-allow with a reason, and reports a bare one', () => {
		const ok = '<!-- copy-allow: consequence warning -->\n<Panel title="A" note="This will replace every row in the table">';
		expect(findings('a.svelte', ok)).toEqual({ findings: [], allowed: 1 });
		const bare = '<!-- copy-allow -->\n<Panel title="A" note="This will replace every row in the table">';
		expect(findings('a.svelte', bare).findings.map((f) => f.rule)).toContain('copy-allow without a reason');
	});
});
