<script lang="ts">
	import SearchIcon from '@lucide/svelte/icons/search';
	import MinusIcon from '@lucide/svelte/icons/minus';
	import ClearInputButton from '$lib/components/clear-input-button.svelte';
	import type { SelectionVocabulary } from '$lib/api';
	import type { ChoiceOption } from '$lib/list/choice';
	import { backspace, choose, comma, commit, draftToken, fieldOf, finished, type FieldState } from '$lib/selection/field';
	import { parsePart, parseToken } from '$lib/selection/syntax';
	import { suggest, VALUES_OF, type NameEntry, type SuggestionGroup, type SuggestionItem } from '$lib/selection/suggest';
	import type { RecentSelection } from '$lib/state/selection.svelte';
	import SelectionChip from './selection-chip.svelte';
	import SelectorIcon from './selector-icon.svelte';

	// One row of the selection bar: SQLBuild's selection syntax typed as text, each finished token
	// drawn as chips in place (comma parts grouped with "and"). Space, Tab or Enter finishes a token,
	// a comma joins the next part to it, Backspace turns the last chip back into text. Suggestions
	// follow what is typed. `value` is the finished selection; the token being typed joins it when
	// it is finished or the field is left.
	let {
		value = $bindable(''),
		tone = 'select',
		label,
		placeholder = '',
		vocabulary,
		names,
		recent = [],
		runs = [],
		problem = null,
		onrecent,
		onleave
	}: {
		value?: string;
		tone?: 'select' | 'except';
		label: string;
		placeholder?: string;
		vocabulary: SelectionVocabulary | null;
		names: NameEntry[];
		recent?: RecentSelection[];
		/** Run selectors to offer first, such as `failed:<id>+` on the rerun plan. */
		runs?: SuggestionItem[];
		/** The token and parts `sqb` refuses, and its first fix. */
		problem?: { token: number; parts: number[]; fix: string | null } | null;
		onrecent?: (r: RecentSelection) => void;
		onleave?: () => void;
	} = $props();

	let field = $state<FieldState>(fieldOf(value));
	let focused = $state(false);
	let active = $state(0);
	let input = $state<HTMLInputElement | null>(null);

	// A selection set from outside (a link, a recent selection, a fix) replaces what is drawn.
	$effect(() => {
		const v = value;
		if (v !== finished(field)) field = { ...fieldOf(v), parts: [], text: '' };
	});

	function update(next: FieldState) {
		field = next;
		value = finished(next);
	}
	/** Whether the field holds anything to clear: finished tokens or text being typed. */
	const holds = $derived(field.tokens.length > 0 || field.parts.length > 0 || field.text.length > 0);
	function clear() {
		update(fieldOf(''));
		input?.focus();
	}
	/** A change made by a chip: the finished tokens, keeping what is being typed. */
	const setTokens = (tokens: string[]) => update({ ...field, tokens });

	const tokens = $derived(field.tokens.map((t) => parseToken(t)));
	const draftParts = $derived(field.parts.map(parsePart));
	const groups = $derived<SuggestionGroup[]>(focused ? suggest(field.text, vocabulary, names, field.tokens.length || field.parts.length || tone === 'except' ? [] : recent.map((r) => ({ select: r.exclude ? `${r.select} --exclude ${r.exclude}` : r.select, count: r.count })), tone === 'except' ? [] : runs) : []);
	const items = $derived(groups.flatMap((g) => g.items));
	const typed = $derived(field.text.replace(/^\+/, '').replace(/^[a-z_]+:/, '').toLowerCase());

	// The values each type takes, for the chips' pickers: built once per vocabulary, not per chip.
	const nameOptions = $derived<ChoiceOption[]>(names.map((n) => ({ value: n.value, label: n.value, count: n.count })));
	const typeOptions = $derived.by(() => {
		const out = new Map<string, ChoiceOption[]>();
		for (const [type, list] of Object.entries(VALUES_OF)) out.set(type, (vocabulary?.[list] ?? []).map((e) => ({ value: e.value, label: e.value, count: e.count })));
		return out;
	});
	const optionsOf = (type: string | null, value: string) => (type ? (typeOptions.get(type) ?? []) : value.includes('/') ? (typeOptions.get('path') ?? []) : nameOptions);

	function apply(item: SuggestionItem, groupName: string) {
		if (groupName === 'Recent') {
			const r = recent.find((x) => (x.exclude ? `${x.select} --exclude ${x.exclude}` : x.select) === item.insert);
			if (r) onrecent?.(r);
			return;
		}
		update(choose(field, item.insert, item.complete));
		active = 0;
	}

	function onkeydown(e: KeyboardEvent) {
		const text = field.text;
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			if (!items.length) return;
			e.preventDefault();
			active = (active + (e.key === 'ArrowDown' ? 1 : -1) + items.length) % items.length;
		} else if (e.key === 'Escape') {
			// Esc clears the field and keeps focus; an empty field's Esc closes its suggestions.
			if (holds) {
				e.preventDefault();
				e.stopPropagation();
				clear();
			} else if (focused && items.length) {
				e.preventDefault();
				e.stopPropagation();
				input?.blur();
			}
		} else if (e.key === 'Tab') {
			if (!text && problem?.fix && !e.shiftKey) {
				e.preventDefault();
				fix();
			} else if (text.trim()) {
				e.preventDefault();
				const item = items[active];
				if (item && text !== item.insert) pick(item);
				else update(commit(field));
			}
		} else if (e.key === 'Enter') {
			e.preventDefault();
			const item = items[active];
			if (item && text.trim() && text !== item.insert) pick(item);
			else if (item && !text.trim() && !field.parts.length && groups[0]?.name === 'Recent') pick(item);
			else update(commit(field));
		} else if (e.key === ' ') {
			e.preventDefault();
			if (text.trim() || field.parts.length) update(commit(field));
		} else if (e.key === ',') {
			e.preventDefault();
			update(comma(field));
		} else if (e.key === 'Backspace' && !text && (field.parts.length || field.tokens.length)) {
			e.preventDefault();
			update(backspace(field));
		}
	}
	function pick(item: SuggestionItem) {
		const group = groups.find((g) => g.items.includes(item));
		apply(item, group?.name ?? '');
	}

	function oninput() {
		active = 0;
		// A paste with spaces: every token but the last is finished.
		if (/\s/.test(field.text)) {
			const ends = /\s$/.test(field.text);
			const words = field.text.split(/\s+/).filter(Boolean);
			const last = ends ? '' : (words.pop() ?? '');
			update({ ...commit({ ...field, text: words.join(' ') }), text: last });
		}
	}

	function fix() {
		if (!problem?.fix) return;
		const next = [...field.tokens];
		next.splice(problem.token, 1, ...problem.fix.split(/\s+/).filter(Boolean));
		update({ ...field, tokens: next });
	}

	function leave() {
		focused = false;
		if (draftToken(field)) update(commit(field));
		onleave?.();
	}

	/** `label` with the typed fragment marked. */
	function marked(text: string): [string, string, string] {
		const at = typed ? text.toLowerCase().indexOf(typed) : -1;
		return at < 0 ? [text, '', ''] : [text.slice(0, at), text.slice(at, at + typed.length), text.slice(at + typed.length)];
	}
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="sel-field {tone}" class:focus={focused} class:err={!!problem} onclick={(e) => e.target === e.currentTarget && input?.focus()}>
	{#if tone === 'select'}<SearchIcon size={14} class="lead-icon" />{:else}<MinusIcon size={14} class="lead-icon" />{/if}
	{#each tokens as t, ti (ti)}
		<span class="grp" class:and={t.parts.length > 1}>
			{#each t.parts as p, pi (pi)}
				{#if pi}<span class="joint">and</span>{/if}
				<SelectionChip part={p} tokens={field.tokens} token={ti} index={pi} options={optionsOf(p.type, p.value)} {tone} bad={problem?.token === ti && problem.parts.includes(pi)} onchange={setTokens} />
			{/each}
		</span>
	{/each}
	<span class="grp draft" class:and={draftParts.length > 0}>
		{#each draftParts as p, pi (pi)}
			<SelectionChip part={p} tokens={[]} token={0} index={pi} {tone} readonly />
			<span class="joint">and</span>
		{/each}
		<input
			bind:this={input}
			bind:value={field.text}
			{oninput}
			{onkeydown}
			onfocus={() => (focused = true)}
			onblur={leave}
			placeholder={field.tokens.length || field.parts.length ? '' : placeholder}
			aria-label={label}
			autocomplete="off"
			spellcheck="false"
			class="syntax"
		/>
	</span>
	{#if holds}<ClearInputButton label="Clear {label.toLowerCase()}" onclear={clear} />{/if}
	{#if focused && groups.length}
		<div class="sel-suggest" role="listbox" aria-label="{label} suggestions" tabindex="-1" onmousedown={(e) => e.preventDefault()}>
			{#each groups as g (g.name)}
				<div class="sg-group">
					<div class="sg-head"><span>{g.name}</span>{#if g.counts}<span>nodes</span>{/if}</div>
					{#each g.items as item (item.insert)}
						{@const i = items.indexOf(item)}
						{@const [a, m, b] = marked(item.label)}
						<div class="sg-row" class:active={i === active} role="option" aria-selected={i === active} tabindex="-1" onclick={() => apply(item, g.name)} onpointermove={() => (active = i)}>
							<SelectorIcon type={item.type} folder={item.label.includes('/')} recent={g.name === 'Recent'} />
							<span class="syntax">
								{#if item.type && item.complete}<span class="t-sel">{item.insert.startsWith('+') ? '+' : ''}{item.type}:</span>{:else if item.insert.startsWith('+') && item.complete}<span class="t-op">+</span>{/if}{a}{#if m}<mark>{m}</mark>{/if}{b}
							</span>
							{#if item.hint}<span class="hint">{item.hint}</span>{/if}
							{#if item.count != null}<span class="tail">{item.count.toLocaleString('en')}</span>{/if}
							{#if i === active && field.text}<span class="tail"><kbd>Tab</kbd></span>{/if}
						</div>
					{/each}
				</div>
			{/each}
			<div class="sg-foot">
				<span><code><b>+</b>x</code> upstream</span>
				<span><code>x<b>+</b></code> downstream</span>
				<span><code>a<b>~</b>b</code> path between</span>
				<span><code>a b</code> either</span>
				<span><code>a<b>,</b>b</code> both</span>
				<span><code>*</code> glob</span>
				<span class="keys"><kbd>Tab</kbd> make chip · <kbd>⌫</kbd> edit chip as text</span>
			</div>
		</div>
	{/if}
</div>

<style>
	.sel-field {
		position: relative;
		display: flex;
		flex: 1 1 auto;
		flex-wrap: wrap;
		align-items: center;
		gap: 5px;
		min-width: 0;
		min-height: 30px;
		padding: 3px 8px;
		border: 1px solid var(--input);
		border-radius: 2px;
		background: var(--background);
		cursor: text;
	}
	.sel-field.focus {
		border-color: var(--ring);
	}
	.sel-field.err {
		border-color: var(--sb-error);
	}
	.sel-field :global(.lead-icon) {
		flex: none;
		color: var(--sb-text-faint);
	}
	.grp {
		display: inline-flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 4px;
		min-width: 0;
		max-width: 100%;
	}
	.grp.and {
		padding: 1px;
		border: 1px dashed color-mix(in srgb, var(--sb-warning) 55%, transparent);
		border-radius: 3px;
	}
	.grp.draft {
		flex: 1 1 140px;
	}
	.joint {
		color: var(--sb-warning);
		font-size: 11px;
		font-weight: 600;
	}
	input {
		flex: 1;
		min-width: 120px;
		height: 22px;
		border: 0;
		outline: none;
		background: transparent;
		color: var(--foreground);
	}
	.syntax {
		font: 12.5px var(--font-mono);
	}
	.sel-suggest {
		position: absolute;
		top: calc(100% + 4px);
		left: 0;
		z-index: 50;
		width: min(640px, 100%);
		min-width: min(420px, 90vw);
		max-height: min(460px, 60vh);
		overflow-y: auto;
		border: 1px solid var(--border);
		border-radius: 4px;
		background: var(--popover);
		box-shadow: 0 8px 24px rgb(0 0 0 / 0.3);
		cursor: default;
	}
	.sg-group + .sg-group {
		border-top: 1px solid var(--border);
	}
	.sg-head {
		display: flex;
		justify-content: space-between;
		padding: 6px 10px 2px;
		color: var(--sb-text-faint);
		font-size: 11px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.03em;
	}
	.sg-row {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 28px;
		padding: 0 10px;
		font-size: 12.5px;
		cursor: pointer;
	}
	.sg-row.active {
		background: var(--sb-hover);
	}
	.sg-row .syntax {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.t-sel {
		color: color-mix(in srgb, var(--primary) 70%, var(--foreground));
	}
	.t-op,
	.sg-foot b {
		color: var(--sb-stale);
	}
	mark {
		background: color-mix(in srgb, var(--primary) 25%, transparent);
		color: inherit;
	}
	.hint {
		min-width: 0;
		overflow: hidden;
		color: var(--muted-foreground);
		font-size: 12px;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.tail {
		margin-left: auto;
		flex: none;
		color: var(--muted-foreground);
		font-size: 11.5px;
		font-variant-numeric: tabular-nums;
	}
	.tail + .tail {
		margin-left: 6px;
	}
	.sg-foot {
		display: flex;
		flex-wrap: wrap;
		gap: 4px 14px;
		padding: 7px 10px;
		border-top: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
	.sg-foot code {
		font: 11.5px var(--font-mono);
		color: var(--foreground);
	}
	.sg-foot .keys {
		margin-left: auto;
	}
	kbd {
		padding: 0 4px;
		border: 1px solid var(--border);
		border-radius: 2px;
		font: 11px var(--font-mono);
	}
</style>
