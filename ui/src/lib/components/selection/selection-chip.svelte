<script lang="ts">
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import ArrowDownIcon from '@lucide/svelte/icons/arrow-down';
	import XIcon from '@lucide/svelte/icons/x';
	import Button from '$lib/components/ui/button/button.svelte';
	import * as Popover from '$lib/components/ui/popover';
	import ChoiceList from '$lib/components/list/choice-list.svelte';
	import { panelWidth, type ChoiceOption } from '$lib/list/choice';
	import { partText, type Part } from '$lib/selection/syntax';
	import { pickedValues, setValuesIn } from '$lib/selection/picker';
	import { replacePart, toggleExpansion, fieldOf } from '$lib/selection/field';
	import SelectorIcon from './selector-icon.svelte';

	// One comma part of a finished token, drawn as a chip: its `+` inside it, its type faint and its
	// value strong. Clicking it opens its value picker (the choice filter's list: search, ticks,
	// counts), the Upstream and Downstream toggles and Remove; every change rewrites the text.
	let {
		part,
		tokens,
		token,
		index,
		options = [],
		tone = 'select',
		bad = false,
		readonly = false,
		onchange
	}: {
		part: Part;
		/** Every finished token of the field, so ticking more values can add copies of this one. */
		tokens: string[];
		token: number;
		index: number;
		/** The values this chip's type takes, with node counts. */
		options?: ChoiceOption[];
		tone?: 'select' | 'except';
		/** `sqb` refuses this part: drawn red. */
		bad?: boolean;
		readonly?: boolean;
		onchange?: (tokens: string[]) => void;
	} = $props();

	let open = $state(false);
	const typeName = $derived(part.between ? 'Path between' : part.type ? part.type[0].toUpperCase() + part.type.slice(1) : part.value.includes('/') ? 'Folder' : 'Node');
	const label = $derived(part.type ? `${part.type}:${part.value}` : part.value);
	const finished = (next: string[]) => onchange?.(next);
	const picked = $derived(open ? pickedValues(tokens, token, index) : []);
	function setPicked(next: string[]) {
		if (!next.length) open = false;
		finished(setValuesIn(tokens, token, index, next));
	}
	const remove = () => {
		open = false;
		finished(replacePart(fieldOf(tokens.join(' ')), token, index, '').tokens);
	};
	const toggle = (side: 'up' | 'down') => finished(toggleExpansion(fieldOf(tokens.join(' ')), token, index, side).tokens);
</script>

<span class="sel-chip {tone}" class:bad class:open title={readonly ? partText(part) : 'Click to edit · Backspace in the field turns the last chip back into text'}>
	{#if readonly}
		<span class="chip-face">{@render face()}</span>
	{:else}
		<Popover.Root bind:open>
			<Popover.Trigger class="chip-face" aria-label="Edit {partText(part)}">{@render face()}</Popover.Trigger>
			<Popover.Content class="choice-panel chip-panel" align="start" collisionPadding={8} style="width: {panelWidth(options, true)}" onOpenAutoFocus={(e: Event) => e.preventDefault()}>
				<div class="chip-head">
					<SelectorIcon type={part.type} folder={part.value.includes('/')} />{typeName}
					<code>{partText(part)}</code>
				</div>
				{#if options.length}
					<ChoiceList label={typeName} multiple options={options} bind:values={() => picked, setPicked} />
				{/if}
				<div class="chip-dirs">
					Include
					<Button variant="outline" size="sm" aria-pressed={part.up} onclick={() => toggle('up')}><ArrowUpIcon />Upstream</Button>
					<Button variant="outline" size="sm" aria-pressed={part.down} onclick={() => toggle('down')}><ArrowDownIcon />Downstream</Button>
				</div>
				<div class="chip-foot">
					<Button variant="destructive" size="sm" onclick={remove}><XIcon />Remove</Button>
					<span><kbd>⌫</kbd> in the field edits the last chip as text</span>
				</div>
			</Popover.Content>
		</Popover.Root>
		<Button variant="ghost" size="icon-xs" class="chip-x" aria-label="Remove {label}" title="Remove {label}" onclick={remove}><XIcon /></Button>
	{/if}
</span>

{#snippet face()}
	{#if part.up}<span class="gop">+</span>{/if}{#if part.type}<span class="k">{part.type}:</span>{/if}<span class="v">{part.value}</span>{#if part.down}<span class="gop">+</span>{/if}
{/snippet}

<style>
	.sel-chip {
		display: inline-flex;
		align-items: center;
		gap: 1px;
		height: 22px;
		max-width: 100%;
		padding: 0 2px 0 0;
		border: 1px solid color-mix(in srgb, var(--primary) 32%, transparent);
		border-radius: 2px;
		background: color-mix(in srgb, var(--primary) 13%, transparent);
		font: 500 12px var(--font-mono);
		white-space: nowrap;
	}
	.sel-chip.except {
		border-color: color-mix(in srgb, var(--sb-error) 35%, transparent);
		background: color-mix(in srgb, var(--sb-error) 10%, transparent);
	}
	.sel-chip.bad {
		border-color: var(--sb-error);
		background: color-mix(in srgb, var(--sb-error) 12%, transparent);
	}
	.sel-chip.bad .v {
		text-decoration: underline wavy var(--sb-error);
		text-decoration-thickness: 1px;
		text-underline-offset: 3px;
	}
	.sel-chip.open {
		box-shadow: 0 0 0 2px color-mix(in srgb, var(--primary) 50%, transparent);
	}
	.sel-chip :global(.chip-face) {
		display: inline-flex;
		align-items: center;
		min-width: 0;
		height: 100%;
		padding: 0 4px 0 6px;
		border: 0;
		background: transparent;
		color: var(--foreground);
		font: inherit;
		cursor: pointer;
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.k {
		color: color-mix(in srgb, var(--primary) 70%, var(--foreground));
		font-weight: 400;
	}
	.except .k {
		color: color-mix(in srgb, var(--sb-error) 70%, var(--foreground));
	}
	.gop {
		color: var(--sb-stale);
		font-weight: 600;
	}
	.v {
		overflow: hidden;
		text-overflow: ellipsis;
	}
	.sel-chip :global(.chip-x) {
		width: 16px;
		height: 16px;
	}
	:global(.chip-panel) .chip-head {
		display: flex;
		align-items: center;
		gap: 7px;
		padding: 8px 10px;
		border-bottom: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.chip-head code {
		margin-left: auto;
		color: var(--foreground);
		font: 12px var(--font-mono);
	}
	.chip-dirs,
	.chip-foot {
		display: flex;
		align-items: center;
		gap: 6px;
		padding: 8px 10px;
		border-top: 1px solid var(--border);
		color: var(--muted-foreground);
		font-size: 12px;
	}
	.chip-foot span {
		margin-left: auto;
		font-size: 11.5px;
	}
	kbd {
		padding: 0 4px;
		border: 1px solid var(--border);
		border-radius: 2px;
		font: 11px var(--font-mono);
	}
</style>
