<script lang="ts" generics="T extends string">
	// The UI's one segmented control: a choice of one option out of a few, as filters, views or modes.
	let {
		value = $bindable(),
		options,
		loading = false,
		label,
		onselect
	}: {
		value: T;
		options: { value: T; label: string; count?: number; title?: string; disabled?: boolean }[];
		/** Nothing loaded yet: counts would read as zero, so they wait for the list. */
		loading?: boolean;
		/** Names the group for screen readers. */
		label?: string;
		/** Called with the option picked, for a choice that does more than set `value`. */
		onselect?: (value: T) => void;
	} = $props();
</script>

<div class="segmented-filter" role="tablist" aria-label={label}>
	{#each options as o (o.value)}
		<button role="tab" aria-selected={value === o.value} class:on={value === o.value} title={o.title} disabled={o.disabled} onclick={() => ((value = o.value), onselect?.(o.value))}>
			{o.label}{#if o.count != null && loading}<span class="pending" aria-hidden="true"></span>{:else if o.count != null}<span>{o.count}</span>{/if}
		</button>
	{/each}
</div>

<style>
	/* Its options wrap onto another row before the control would run past the toolbar. */
	.segmented-filter {
		display: flex;
		width: fit-content;
		max-width: 100%;
		flex-wrap: wrap;
		gap: 2px;
		padding: 2px;
		border-radius: 2px;
		background: var(--sb-hover);
	}
	.segmented-filter button {
		display: inline-flex;
		height: 26px;
		align-items: center;
		gap: 6px;
		padding: 0 10px;
		border: 0;
		border-radius: 2px;
		background: transparent;
		color: var(--muted-foreground);
		font-size: 12.5px;
		white-space: nowrap;
	}
	.segmented-filter button:hover:not(:disabled) {
		color: var(--foreground);
	}
	.segmented-filter button:disabled {
		opacity: 0.45;
	}
	.segmented-filter button.on {
		background: var(--card);
		color: var(--foreground);
		box-shadow: 0 1px 2px rgba(0, 0, 0, 0.14);
	}
	/* Holds a count's place while the list loads, so the filters after it don't jump when it arrives. */
	.segmented-filter .pending {
		width: 2.4ch;
		height: 8px;
		border-radius: 2px;
		background: color-mix(in srgb, var(--foreground) 9%, transparent);
	}
	.segmented-filter span {
		color: var(--sb-text-faint);
		font-size: 11.5px;
		font-variant-numeric: tabular-nums;
	}
</style>
