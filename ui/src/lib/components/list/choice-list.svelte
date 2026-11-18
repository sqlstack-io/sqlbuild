<script lang="ts" generics="T extends string">
	import { untrack } from 'svelte';
	import CheckIcon from '@lucide/svelte/icons/check';
	import SearchIcon from '@lucide/svelte/icons/search';
	import Button from '$lib/components/ui/button/button.svelte';
	import { createVirtualRows } from './virtual-rows.svelte';
	import { clearMatching, isWindowed, matchOptions, selectedFirst, selectMatching, showsSearch, splitMiddle, toggleValue, type ChoiceOption } from '$lib/list/choice';

	// The body of an open choice filter, shared by the toolbar's `ChoiceFilter` and the selection
	// bar's chip picker: a search box (from 10 options), the options with their counts, selected ones
	// first. `multiple` ticks any number of options into `values`, applied as they are ticked;
	// otherwise one choice goes into `value`, `all` clears it and `onchosen` follows. Thousands of
	// options are drawn windowed. Keys: type to search, arrows move, Space or Enter ticks (Enter
	// alone takes the first match). It takes focus when it opens.
	let {
		label,
		value = $bindable('' as T | ''),
		values = $bindable([] as T[]),
		multiple = false,
		options,
		all = 'All',
		onchosen
	}: {
		label: string;
		value?: T | '';
		values?: T[];
		multiple?: boolean;
		options: ChoiceOption<T>[];
		all?: string;
		/** A single choice was made: the filter closes. */
		onchosen?: () => void;
	} = $props();

	const ROW_H = 28;
	const uid = $props.id();

	let query = $state('');
	const chosen = $derived<T[]>(multiple ? values : value ? [value] : []);
	const chosenSet = $derived(new Set<string>(chosen));
	/** The choices when the list opened: listed first, and kept in place while ticking. */
	const firstAtOpen: ReadonlySet<string> = untrack(() => new Set(chosen));
	let active = $state(-1);
	/** Arrows moved to an option, so Space ticks it rather than typing a space. */
	let keyed = $state(false);
	let input = $state<HTMLInputElement | null>(null);
	let scroller = $state<HTMLElement | null>(null);

	const searchable = $derived(showsSearch(options.length));
	const shown = $derived(matchOptions(selectedFirst(options, firstAtOpen), query));
	const windowed = $derived(isWindowed(shown.length));
	const rows = createVirtualRows(ROW_H, () => shown.length);
	const first = $derived(windowed ? rows.first : 0);
	const last = $derived(windowed ? rows.last : shown.length);

	function pick(o: ChoiceOption<T>) {
		if (multiple) values = toggleValue(values, o.value);
		else {
			value = o.value;
			onchosen?.();
		}
	}

	function moveTo(i: number) {
		active = Math.max(0, Math.min(shown.length - 1, i));
		keyed = true;
		const el = scroller;
		if (!el) return;
		const top = active * ROW_H;
		if (top < el.scrollTop) el.scrollTop = top;
		else if (top + ROW_H > el.scrollTop + el.clientHeight) el.scrollTop = top + ROW_H - el.clientHeight;
	}

	function onkeydown(e: KeyboardEvent) {
		if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
			e.preventDefault();
			if (shown.length) moveTo(active < 0 ? (e.key === 'ArrowDown' ? 0 : shown.length - 1) : active + (e.key === 'ArrowDown' ? 1 : -1));
		} else if (e.key === ' ' && (keyed || !searchable) && shown[active]) {
			e.preventDefault();
			pick(shown[active]);
		} else if (e.key === 'Enter') {
			e.preventDefault();
			const target = keyed && shown[active] ? shown[active] : shown[0];
			if (!target) return;
			// Enter on the search takes its first match; it never un-ticks one.
			if (multiple && !keyed && chosenSet.has(target.value)) return;
			pick(target);
		}
	}

	function onSearch() {
		keyed = false;
		active = query.trim() ? 0 : -1;
		if (scroller) scroller.scrollTop = 0;
	}

	/** Takes focus once drawn: the search box, or the list when there is none. */
	function focusOnOpen(el: HTMLElement) {
		requestAnimationFrame(() => (el.querySelector<HTMLElement>('.cf-search input') ?? el.querySelector<HTMLElement>('.cf-list'))?.focus());
	}
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="cf-body" {onkeydown} {@attach focusOnOpen}>
	{#if searchable}
		<label class="cf-search">
			<SearchIcon size={13} />
			<input
				bind:this={input}
				bind:value={query}
				oninput={onSearch}
				placeholder="Search {options.length.toLocaleString()} {label.toLowerCase()} options"
				aria-label="Search {label}"
				aria-controls="{uid}-list"
				aria-activedescendant={active >= 0 ? `${uid}-${active}` : undefined}
				autocomplete="off"
				spellcheck="false"
			/>
		</label>
	{/if}
	<div class="cf-actions">
		{#if multiple}
			<Button variant="ghost" size="sm" disabled={!shown.length} onclick={() => (values = selectMatching(values, shown))}>Select all{query.trim() ? ' matches' : ''}</Button>
			<Button variant="ghost" size="sm" disabled={!shown.some((o) => chosenSet.has(o.value))} onclick={() => (values = clearMatching(values, shown))}>Clear{query.trim() ? ' matches' : ''}</Button>
		{:else}
			<Button variant="ghost" size="sm" aria-pressed={!value} onclick={() => { value = ''; onchosen?.(); }}>{all}</Button>
		{/if}
		<span class="cf-tally">{query.trim() ? `${shown.length.toLocaleString()} of ${options.length.toLocaleString()}` : chosen.length ? `${chosen.length} selected` : ''}</span>
	</div>
	<div
		class="cf-list"
		id="{uid}-list"
		role="listbox"
		aria-label={label}
		aria-multiselectable={multiple}
		tabindex={searchable ? -1 : 0}
		bind:this={scroller}
		{@attach rows.attach}
	>
		{#if windowed && rows.before}<div style:height="{rows.before}px"></div>{/if}
		{#each shown.slice(first, last) as o, j (o.value)}
			{@const i = first + j}
			{@const on = chosenSet.has(o.value)}
			{@const parts = splitMiddle(o.label)}
			<div
				class="cf-row"
				class:multiple
				class:active={i === active}
				id="{uid}-{i}"
				role="option"
				aria-selected={on}
				tabindex="-1"
				title={o.title ? `${o.label}\n${o.title}` : o.label}
				onmousedown={(e) => e.preventDefault()}
				onclick={() => pick(o)}
				onkeydown={() => {}}
				onpointermove={() => (active = i)}
			>
				{#if multiple}<input type="checkbox" class="sb-check" tabindex="-1" aria-hidden="true" checked={on} />{:else}<span class="cf-mark">{#if on}<CheckIcon size={13} />{/if}</span>{/if}
				<span class="cf-label"><span class="cf-head">{parts.head}</span>{#if parts.tail}<span class="cf-tail">{parts.tail}</span>{/if}</span>
				{#if o.count != null}<span class="cf-count">{o.count.toLocaleString()}</span>{/if}
			</div>
		{:else}
			<div class="cf-empty">No {label.toLowerCase()} matches “{query.trim()}”.</div>
		{/each}
		{#if windowed && rows.after}<div style:height="{rows.after}px"></div>{/if}
	</div>
</div>

<style>
	.cf-search {
		display: flex;
		align-items: center;
		gap: 6px;
		height: 34px;
		padding: 0 10px;
		border-bottom: 1px solid var(--border);
		color: var(--sb-text-faint);
	}
	.cf-search input {
		flex: 1;
		min-width: 0;
		border: 0;
		outline: none;
		background: transparent;
		color: var(--foreground);
		font-size: 12.5px;
	}
	.cf-actions {
		display: flex;
		align-items: center;
		gap: 2px;
		padding: 3px 4px;
		border-bottom: 1px solid var(--border);
	}
	.cf-tally {
		margin-left: auto;
		padding-right: 6px;
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-variant-numeric: tabular-nums;
		white-space: nowrap;
	}
	.cf-list {
		max-height: min(336px, 50vh);
		overflow-y: auto;
		overscroll-behavior: contain;
		padding: 2px 0;
		outline: none;
	}
	.cf-row {
		display: grid;
		grid-template-columns: 14px minmax(0, 1fr) auto;
		align-items: center;
		gap: 8px;
		height: 28px;
		padding: 0 10px;
		cursor: pointer;
		font-size: 12.5px;
		user-select: none;
	}
	.cf-row.active {
		background: var(--sb-hover);
	}
	.cf-row :global(.sb-check) {
		pointer-events: none;
	}
	.cf-mark {
		display: inline-flex;
		color: var(--primary);
	}
	.cf-label {
		display: flex;
		min-width: 0;
		white-space: pre;
	}
	.cf-head {
		min-width: 0;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.cf-tail {
		flex: none;
	}
	.cf-count {
		color: var(--muted-foreground);
		font-size: 11.5px;
		font-variant-numeric: tabular-nums;
		text-align: right;
	}
	.cf-empty {
		padding: 10px 12px;
		color: var(--muted-foreground);
		font-size: 12.5px;
	}
</style>
