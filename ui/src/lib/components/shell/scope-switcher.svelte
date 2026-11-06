<script lang="ts">
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import ChevronsUpDownIcon from '@lucide/svelte/icons/chevrons-up-down';
	import { healthLabel, healthTone, targetWords } from '$lib/platform/labels';
	import { app } from '$lib/state/app.svelte';

	const locations = $derived(app.bootstrap?.locations ?? []);
</script>

<div class="switchers">
	<DropdownMenu.Root>
		<DropdownMenu.Trigger class="switch" aria-label="Switch location">
			<span class="badge">{app.location?.name.slice(0, 1).toUpperCase() ?? '·'}</span>
			<span class="copy">
				<span class="value">{app.location?.name ?? '—'}</span>
				<small>{app.location ? targetWords(app.location.target) : 'Location'}</small>
			</span>
			{#if app.location?.health && app.location.health !== 'healthy'}
				<i class="dot" style:background={healthTone[app.location.health]} title={healthLabel[app.location.health]}></i>
			{/if}
			<ChevronsUpDownIcon size={13} class="chev" />
		</DropdownMenu.Trigger>
		<DropdownMenu.Content class="w-64">
			<DropdownMenu.Label>Locations</DropdownMenu.Label>
			<DropdownMenu.RadioGroup value={app.locationId} onValueChange={(id) => app.selectLocation(id)}>
				{#each locations as location (location.id)}
					<DropdownMenu.RadioItem value={location.id}>
						{#if location.health}
							<i class="dot" style:background={healthTone[location.health]} title={healthLabel[location.health]}></i>
						{/if}
						<span class="flex min-w-0 flex-col">
							<span class="truncate">{location.name}</span>
							<small class="text-muted-foreground text-[11px]">
								{targetWords(location.target)}{location.snapshot === undefined ? '' : location.snapshot ? ` · version ${location.snapshot.generation}` : ' · nothing deployed'}
							</small>
						</span>
					</DropdownMenu.RadioItem>
				{/each}
			</DropdownMenu.RadioGroup>
		</DropdownMenu.Content>
	</DropdownMenu.Root>
</div>

<style>
	.switchers {
		display: flex;
		flex-direction: column;
		overflow: hidden;
		margin-bottom: 12px;
		border: 1px solid var(--sidebar-border);
		border-radius: 6px;
	}
	.switchers :global(.switch) {
		display: flex;
		width: 100%;
		align-items: center;
		gap: 10px;
		padding: 7px 10px;
		text-align: left;
		font-size: 13px;
	}
	.switchers :global(.switch:hover),
	.switchers :global(.switch[data-state='open']) {
		background: var(--sb-hover);
	}
	.switchers :global(.chev) {
		margin-left: auto;
		flex-shrink: 0;
		color: var(--muted-foreground);
	}
	.badge {
		display: grid;
		width: 20px;
		height: 20px;
		flex-shrink: 0;
		place-items: center;
		border-radius: 2px;
		background: var(--secondary);
		color: var(--muted-foreground);
		font-size: 10px;
		font-weight: 600;
	}
	.copy {
		display: flex;
		min-width: 0;
		flex-direction: column;
		line-height: 1.2;
	}
	.copy small {
		color: var(--muted-foreground);
		font-size: 10.5px;
	}
	.value {
		overflow: hidden;
		font-weight: 500;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.dot {
		display: inline-block;
		width: 7px;
		height: 7px;
		flex-shrink: 0;
		border-radius: 50%;
	}
</style>
