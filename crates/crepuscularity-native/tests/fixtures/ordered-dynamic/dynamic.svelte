<script lang="ts">
  const fixture = globalThis as unknown as { dynamicApi: unknown };
  import First from './dynamic-first.svelte';
  import Second from './dynamic-second.svelte';
  import Child from './dynamic-child.svelte';
  let Current: typeof First | null = $state.raw(First);
  let name = $state('alpha');
  let box = $state<{ Current: typeof First | null }>({ Current: First });
  fixture.dynamicApi = {
    second: () => { Current = Second; box.Current = Second; },
    first: () => { Current = First; box.Current = First; },
    same: () => { const current = Current; const member = box.Current; Current = current; box.Current = member; },
    clear: () => { Current = null; box.Current = null; },
    rename: () => name = 'beta'
  };
</script>
<div data-dynamic>
  <div data-dynamic-host="direct"><Current name={name} /></div>
  <div data-dynamic-host="member"><box.Current name={name} /></div>
  <div data-dynamic-host="prop"><Child Current={Current} name={name} /></div>
</div>
