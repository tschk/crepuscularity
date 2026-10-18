<script lang="ts">
import {getContext,onMount,onDestroy} from 'svelte';
let {id}:{id:string}=$props();const label=getContext<string>('label');
const fixture=globalThis as unknown as {directiveTrace:unknown[][]};
fixture.directiveTrace.push(['child-setup',id,label]);
onMount(()=>{fixture.directiveTrace.push(['child-mount',id]);return()=>fixture.directiveTrace.push(['child-mount-cleanup',id]);});
onDestroy(()=>fixture.directiveTrace.push(['child-destroy',id]));
</script><i>{label}</i>
