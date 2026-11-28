<script lang="ts">
	import { app } from '$lib/state/app.svelte';

	// Pages update live, so the top bar shows no refresh while they do. This registers what the page reads
	// again when live updates drop and the person clicks Reconnect in the top bar.
	let { onclick }: { onclick: () => void } = $props();

	$effect(() => {
		app.setPageRefresh(onclick);
		return () => app.setPageRefresh(null);
	});
</script>
