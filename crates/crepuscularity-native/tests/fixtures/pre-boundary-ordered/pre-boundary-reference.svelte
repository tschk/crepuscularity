<script lang="ts">
  import Child from './pre-boundary-reference-child.svelte';
  let { id, initial = 'one', visible = true, markup = '<b>raw</b>' }: { id: string; initial?: string; visible?: boolean; markup?: string } = $props();
  // svelte-ignore state_referenced_locally
  let value = $state(initial);
  // svelte-ignore state_referenced_locally
  let shown = $state(visible);
  // svelte-ignore state_referenced_locally
  let html = $state(markup);
  const fixture = globalThis as unknown as {
    preBoundaryApis: Record<string, { hide: () => void; empty: () => void; set: (value: string) => void }>;
  };
  fixture.preBoundaryApis[id] = { hide() { shown = false; }, empty() { value = ''; html = ''; }, set(next) { value = next; } };
</script>

{#snippet word()}<i>{value}</i>{/snippet}
<section data-pre-boundary={id}>
  <pre data-pre-case="direct">

{value}</pre>
  <pre data-pre-case="conditional">

{#if shown}{value}{/if}</pre>
  <pre data-pre-case="key">

{#key value}{value}{/key}</pre>
  <pre data-pre-case="snippet">

{@render word()}</pre>
  <pre data-pre-case="raw">

{@html html}</pre>
  <pre data-pre-case="component">

<Child {value}/></pre>
  <textarea data-pre-case="textarea">

{value}</textarea>
</section>
