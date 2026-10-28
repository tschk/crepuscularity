<script lang="ts">
import {onMount,onDestroy,getContext} from 'svelte';
let {value}:{value:string}=$props();
const trace=(globalThis as any).__trace as string[];
const context=getContext<string>('mixed');
trace.push('child:setup:'+value+':'+context);
onMount(()=>{trace.push('child:mount:'+value);return()=>trace.push('child:mount-cleanup:'+value)});
onDestroy(()=>trace.push('child:destroy:'+value));
</script>
<b data-child>{value}:{context}</b>
