<script lang="ts">
	// A strip of recent outcomes, oldest on the left.
	let { outcomes, tones, caption }: { outcomes: string[]; tones: Record<string, string>; caption: string } = $props();
</script>

<div class="strip" title={caption}>
	{#each outcomes as o, i (i)}<i style:background={tones[o]} title={o}></i>{/each}
</div>
<div class="caption"><span>{caption}</span><span>latest</span></div>

<style>
	.strip {
		display: flex;
		gap: 2px;
		height: 16px;
	}
	.strip i {
		flex: 1;
		max-width: 18px;
		border-radius: 2px;
		opacity: 0.8;
	}
	.caption {
		display: flex;
		justify-content: space-between;
		margin-top: 5px;
		color: var(--sb-text-faint);
		font-size: 11.5px;
	}
</style>
