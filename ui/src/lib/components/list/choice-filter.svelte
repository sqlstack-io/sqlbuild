<script lang="ts" generics="T extends string">
	import ChevronDownIcon from '@lucide/svelte/icons/chevron-down';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';

	// A toolbar filter with one choice; `all` clears it.
	let {
		label,
		value = $bindable(),
		options,
		all = 'All'
	}: {
		label: string;
		value: T | '';
		options: { value: T; label: string; count?: number }[];
		all?: string;
	} = $props();

	const current = $derived(options.find((o) => o.value === value)?.label);
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger class="choice-filter {value ? 'set' : ''}">
		{label}{#if current}<b>{current}</b>{/if}
		<ChevronDownIcon size={13} />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content class="w-56">
		<DropdownMenu.RadioGroup value={value || '__all'} onValueChange={(v) => (value = v === '__all' ? '' : (v as T))}>
			<DropdownMenu.RadioItem value="__all">{all}</DropdownMenu.RadioItem>
			{#each options as o (o.value)}
				<DropdownMenu.RadioItem value={o.value}>
					<span class="flex-1">{o.label}</span>
					{#if o.count != null}<span class="text-muted-foreground text-[11.5px] tabular-nums">{o.count}</span>{/if}
				</DropdownMenu.RadioItem>
			{/each}
		</DropdownMenu.RadioGroup>
	</DropdownMenu.Content>
</DropdownMenu.Root>

<style>
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
	}
	:global(.choice-filter:hover) {
		background: var(--sb-hover);
		color: var(--foreground);
	}
	:global(.choice-filter b) {
		color: var(--foreground);
		font-weight: 500;
	}
	:global(.choice-filter svg) {
		color: var(--sb-text-faint);
	}
</style>
