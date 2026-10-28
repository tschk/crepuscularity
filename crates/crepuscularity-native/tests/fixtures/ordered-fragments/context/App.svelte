<script lang="ts">
  import { getContext, setContext, onDestroy, tick } from 'svelte';
  import Consumer from './Consumer.svelte';
  import Read from './Read.svelte';
  let { id }: { id: string } = $props();
  let rows = $state([{ id: 'a', value: 1 }]);
  let value = $state(10);
  const api = globalThis as unknown as { constContextTrace: unknown[][]; constContextApis: Record<string, {replace():void; value():void; hide():void; show():void; tick:typeof tick}> };
  setContext('key', 'A:' + id);
  onDestroy(() => api.constContextTrace.push(['component-destroy', id]));
  function probe(label: string, number: number) {
    const before = getContext<string>('key');
    setContext('key', id + ':' + label + ':' + number);
    const after = getContext<string>('key');
    api.constContextTrace.push(['const', id, label, before, after]);
    onDestroy(() => api.constContextTrace.push(['const-destroy', id, label, number]));
    return after;
  }
  api.constContextApis[id] = { replace(){rows=[{id:'a',value:2}]}, value(){value++}, hide(){rows=[]}, show(){rows=[{id:'a',value:3}]}, tick };
</script>
<section data-instance={id}>
{#each rows as row (row.id)}
  {@const result=probe('row:'+row.id,row.value)}
  <p data-row>{result}</p><Read label={id+':row-sibling'}/>
{/each}
<Read label={id+':after-rows'}/>
{#snippet content(input:number)}
  {@const result=probe('snippet',input)}
  <strong>{result}</strong><Read label={id+':snippet-sibling'}/>
{/snippet}
<Consumer {id} {content} {value}/>
<Read label={id+':after-consumer'}/>
</section>
