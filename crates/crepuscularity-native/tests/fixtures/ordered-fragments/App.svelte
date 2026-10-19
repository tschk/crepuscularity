<script lang="ts">
 import {setContext,getContext,onMount,onDestroy,tick} from 'svelte';
 import Read from './Read.svelte';
 let {id}:{id:string}=$props();let value=$state(1);let node:HTMLElement;
 const api=globalThis as any;setContext('label','parent-'+id);
 function observe(kind:string){api.orderTrace.push([kind,id,value,getContext('label')]);return kind+'-'+value}
 function reference(next:HTMLElement){node=next;api.orderTrace.push(['ref',id,!!next,next?.isConnected??false])}
 function action(element:HTMLElement,n:number){api.orderTrace.push(['action',id,n,element.isConnected,!!node]);return{update(n:number){api.orderTrace.push(['action-update',id,n,element.textContent])},destroy(){api.orderTrace.push(['action-destroy',id,element.isConnected])}}}
 onMount(()=>{api.orderTrace.push(['mount',id,node?.isConnected]);return()=>api.orderTrace.push(['mount-cleanup',id,node?.isConnected])});onDestroy(()=>api.orderTrace.push(['destroy',id]));api.orderApis[id]={update(){value++},tick};
</script>
<section data-id={id} title={observe('attr')} bind:this={()=>node,reference} use:action={value}>
 <span>{observe('text-before')}</span><Read label={id+':inner'}/><b>{observe('text-after')}</b>
 <svg><g>{#each [1] as item}<a data-probe="svg"><title>{id}</title></a>{/each}</g><foreignObject><div><a data-probe="html">html</a><svg><a data-probe="svg-again">svg</a></svg></div></foreignObject></svg>
 <textarea>{observe('textarea')}</textarea><select><option>{observe('option')}</option></select>
</section><Read label={id+':outside'}/>
