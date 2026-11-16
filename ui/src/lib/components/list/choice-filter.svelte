<script lang="ts" generics="T extends string">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import XIcon from '@lucide/svelte/icons/x';
	import Button from '$lib/components/ui/button/button.svelte';
	import * as Popover from '$lib/components/ui/popover';
	import ChoiceList from './choice-list.svelte';
	import { choiceSummary, panelWidth, type ChoiceOption } from '$lib/list/choice';

	// A toolbar filter, like a spreadsheet's column filter: the open panel is `ChoiceList` (search,
	// options with counts, selected first, windowed). `multiple` ticks any number of options into
	// `values`, applied as they are ticked; otherwise one choice goes into `value` and `all` clears it.
	// Escape closes.
	let {
		label,
		value = $bindable('' as T | ''),
		values = $bindable([] as T[]),
		multiple = false,
		options,
		all = 'All'
	}: {
		label: string;
		/** The one choice, without `multiple`. */
		value?: T | '';
		/** The choices, with `multiple`. */
		values?: T[];
		multiple?: boolean;
		/** `title` explains a short label on hover. */
		options: ChoiceOption<T>[];
		/** The option that clears a single choice. */
		all?: string;
	} = $props();

	let open = $state(false);
	const chosen = $derived<T[]>(multiple ? values : value ? [value] : []);
	const summary = $derived(choiceSummary(chosen, options));
	const width = $derived(panelWidth(options, multiple));
	const fullTitle = $derived(chosen.length > 1 ? chosen.map((v) => options.find((o) => o.value === v)?.label ?? v).join('\n') : undefined);

	function clear() {
		if (multiple) values = [];
		else value = '';
	}
</script>

<span class="choice-filter-wrap">
	<Popover.Root bind:open>
		<Popover.Trigger class="choice-filter {chosen.length ? 'set' : ''}" title={fullTitle}>
			{label}{#if summary}:<b>{summary}</b>{/if}
			<ChevronDownIcon size={13} />
		</Popover.Trigger>
		<Popover.Content class="choice-panel" align="start" collisionPadding={8} style="width: {width}" onOpenAutoFocus={(e: Event) => e.preventDefault()}>
			<ChoiceList {label} bind:value bind:values {multiple} {options} {all} onchosen={() => (open = false)} />
		</Popover.Content>
	</Popover.Root>
	{#if chosen.length}<Button variant="ghost" size="icon-xs" class="choice-clear" aria-label="Clear {label} filter" title="Clear {label} filter" onclick={clear}><XIcon /></Button>{/if}
</span>

<style>
	.choice-filter-wrap {
		display: inline-flex;
		align-items: center;
		min-width: 0;
	}
	:global(.choice-filter) {
		display: inline-flex;
		height: 30px;
		align-items: center;
		gap: 5px;
		padding: 0 9px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 12.5px;
		white-space: nowrap;
	}
	:global(.choice-filter:hover) {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	:global(.choice-filter b) {
		max-width: 220px;
		overflow: hidden;
		text-overflow: ellipsis;
		color: var(--foreground);
		font-weight: 500;
	}
	:global(.choice-filter svg) {
		flex: none;
		color: var(--sb-text-faint);
	}
	:global(.choice-filter-wrap .choice-clear) {
		margin-left: -4px;
	}
	:global(.choice-panel) {
		gap: 0 !important;
		padding: 0 !important;
		max-width: min(520px, 40vw);
		min-width: min(280px, calc(100vw - 16px));
		overflow: hidden;
	}
</style>
