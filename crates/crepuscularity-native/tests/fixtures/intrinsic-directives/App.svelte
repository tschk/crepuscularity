<script lang="ts">
import {setContext,onDestroy} from 'svelte';
import Child from './Child.svelte';
let {id}:{id:string}=$props();let value=$state(1);let shown=$state(true);
const fixture=globalThis as unknown as {directiveTrace:unknown[][];directiveApis:Record<string,{set(n:number):void;hide():void}>};
setContext('label','context-'+id);
onDestroy(()=>fixture.directiveTrace.push(['app-destroy',id]));
function action(node:HTMLElement,n:number){fixture.directiveTrace.push(['action-setup',id,n,node.isConnected]);return {update(next:number){fixture.directiveTrace.push(['action-update',id,next,node.textContent])},destroy(){fixture.directiveTrace.push(['action-destroy',id,node.isConnected])}};}
fixture.directiveApis[id]={set(n){value=n},hide(){shown=false}};
</script>
<section data-instance={id}>
 {#snippet row()}<b use:action={value}>{value}</b><Child {id}/>{/snippet}
 {#if shown}{@render row()}{/if}
</section>
