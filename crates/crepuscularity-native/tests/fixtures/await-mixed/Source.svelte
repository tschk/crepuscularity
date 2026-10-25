<script lang="ts">
import Child from './Child.svelte';
import {setContext} from 'svelte';
const trace=(globalThis as any).__trace as string[];
setContext('mixed','root');
let input=$state<string|Promise<string>>('first');
let finish:(value:string)=>void=()=>{};
(globalThis as any).__driver={plain(){input='second'},pending(){input=new Promise<string>(resolve=>{finish=resolve})},resolve(){finish('resolved')}};
function action(node:HTMLElement,value:string){trace.push('action:init:'+value);return {update(next:string){trace.push('action:update:'+next)},destroy(){trace.push('action:destroy')}}}
</script>
{#await input}<p data-phase>pending</p>{:then value}<section use:action={value}><p data-phase>{value}</p><Child value={value}/></section>{:catch error}<p data-phase>error</p>{/await}
