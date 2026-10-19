<script lang="ts">
  import {
    sharedReferenceStore,
    replacementReferenceStore,
  } from "./store-reference";

  let { id }: { id: string } = $props();
  let current = $state.raw(sharedReferenceStore);
  let shown = $state(true);
  const fixture = globalThis as unknown as {
    storeReferenceApis: Record<
      string,
      {
        show(value: boolean): void;
        replace(): void;
        notify(): void;
      }
    >;
  };
  // Fixture ids stay stable throughout each mounted instance.
  // svelte-ignore state_referenced_locally
  fixture.storeReferenceApis[id] = {
    show: (value) => {
      shown = value;
    },
    replace: () => {
      current = replacementReferenceStore;
    },
    notify: () => {
      current.notify();
    },
  };
</script>

<section data-store-reference={id}>
  {#if shown}<output>{$current.count}</output>{/if}
</section>
