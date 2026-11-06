<script lang="ts">
	let { tone, label, muted = false }: { tone: string; label: string; muted?: boolean } = $props();
</script>

<span class="status-label" class:muted><i style:background={tone}></i>{label}</span>

<style>
	.status-label {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		color: var(--foreground);
		font-size: 12.5px;
		font-weight: 500;
		white-space: nowrap;
	}
	.status-label.muted {
		color: var(--muted-foreground);
		font-weight: 400;
	}
	.status-label i {
		width: 8px;
		height: 8px;
		flex: 0 0 8px;
		border-radius: 50%;
	}
</style>
