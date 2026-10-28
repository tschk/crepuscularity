<script lang="ts">
  const fixture = globalThis as unknown as { dynamicMetrics: { created: unknown[][]; destroyed: unknown[][] } };
  import { onDestroy } from 'svelte';
  let { name } = $props();
  let count = $state(0);
  const initial = name;
  fixture.dynamicMetrics.created.push(['second', initial]);
  onDestroy(() => fixture.dynamicMetrics.destroyed.push(['second', initial]));
</script>
<section data-dynamic-kind="second"><span>{name}</span><button onclick={() => count++}>{count}</button></section>
