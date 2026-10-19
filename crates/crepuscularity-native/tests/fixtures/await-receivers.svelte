<script lang="ts">
const \u005f_crepusAwaitUnbound=7;
function callback(this:unknown):string{return this===undefined?'free':'bound'}
function tag(this:unknown,_strings:TemplateStringsArray):string{return this===undefined?'free':'bound'}
function generic<T>(this:unknown,value:T):string{return `${this===undefined?'free':'bound'}:${value}`}
const object={name:'object',method(this:{name:string}){return this.name}};
const input={callback,tag,generic,object};
const empty:{nested?:string}={};
</script>
{#await input then {callback,tag,generic,object}}
<p data-calls>{(callback as ()=>string)()}/{(callback!)?.()}/{(callback satisfies ()=>string)()}/{(generic<number>)(3)}</p>
<p data-tags>{(tag as (strings:TemplateStringsArray)=>string)`text`}</p>
<p data-method>{object.method()}/{__crepusAwaitUnbound}</p>
{#await empty then {nested=(callback as ()=>string)()}}<p data-nested>{nested}</p>{/await}
{/await}
