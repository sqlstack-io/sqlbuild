<script lang="ts" generics="T extends string">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';

	// A styled choice of one value from a short list, in place of a native <select>.
	type Option = { value: T; label: string; hint?: string; disabled?: boolean; title?: string };
	type Props = {
		value: T;
		options: readonly Option[];
		onchange: (value: T) => void;
		/** The accessible name, e.g. "Role of orders-deploy". */
		label: string;
		disabled?: boolean;
		/** Fill the width of its container, as in a form; otherwise as wide as the choice. */
		block?: boolean;
		testid?: string;
	};
	let { value, options, onchange, label, disabled = false, block = false, testid }: Props = $props();
	const current = $derived(options.find((o) => o.value === value));
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger class="menu-select {block ? 'block' : ''}" aria-label={label} {disabled} data-testid={testid}>
		<span class="current">{current?.label ?? value}</span>
		<ChevronDownIcon size={13} />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="start" class="w-auto min-w-(--bits-dropdown-menu-anchor-width)">
		<DropdownMenu.RadioGroup {value} onValueChange={(next) => next !== value && onchange(next as T)}>
			{#each options as o (o.value)}
				<DropdownMenu.RadioItem value={o.value} disabled={o.disabled} title={o.title}>
					<span class="option">
						<span>{o.label}</span>
						{#if o.hint}<span class="hint">{o.hint}</span>{/if}
					</span>
				</DropdownMenu.RadioItem>
			{/each}
		</DropdownMenu.RadioGroup>
	</DropdownMenu.Content>
</DropdownMenu.Root>

<style>
	:global(.menu-select) {
		display: inline-flex;
		height: 28px;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		padding: 0 8px 0 10px;
		border: 1px solid var(--border);
		border-radius: var(--radius);
		background: transparent;
		color: var(--foreground);
		font-size: 12.5px;
	}
	:global(.menu-select.block) {
		width: 100%;
		height: 32px;
	}
	:global(.menu-select:hover:not(:disabled)) {
		background: var(--accent);
	}
	:global(.menu-select:disabled) {
		opacity: 0.6;
	}
	:global(.menu-select > svg) {
		color: var(--muted-foreground);
	}
	.option {
		display: flex;
		flex-direction: column;
		gap: 1px;
	}
	.hint {
		color: var(--muted-foreground);
		font-size: 11.5px;
	}
</style>
