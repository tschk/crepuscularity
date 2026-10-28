<script lang="ts">
import {setContext,onDestroy,type Snippet} from 'svelte';import Read from './Read.svelte';
let {id,content,value}:{id:string;content:Snippet<[number]>;value:number}=$props();
const api=globalThis as unknown as {constContextTrace:unknown[][]};
setContext('key','B:'+id);api.constContextTrace.push(['consumer',id]);
onDestroy(()=>api.constContextTrace.push(['consumer-destroy',id]));
</script>
<article>{@render content(value)}<Read label={id+':consumer-after-snippet'}/></article>
