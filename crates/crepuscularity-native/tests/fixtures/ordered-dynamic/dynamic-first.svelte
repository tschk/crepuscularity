<script lang="ts">
  const fixture = globalThis as unknown as { dynamicMetrics: { created: unknown[][]; destroyed: unknown[][] } };
  import { onDestroy } from 'svelte';
  let { name } = $props();
  let count = $state(0);
  const initial = name;
  fixture.dynamicMetrics.created.push(['first', initial]);
  onDestroy(() => fixture.dynamicMetrics.destroyed.push(['first', initial]));
</script>
<section data-dynamic-kind="first"><span>{name}</span><button onclick={() => count++}>{count}</button></section>
