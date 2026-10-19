<script lang="ts">const sample=1;</script>
{#each [1] as item:typeof sample,index (item)}<p>{item}</p>{/each}
{#each [1] as ordinary,index (ordinary)}<p>{(()=>{let value=ordinary;value=2;return value.toFixed()})()}</p>{/each}
{#each [{value:1,extra:2}] as {value}:{value:number}}<p>{value.toFixed()}</p>{/each}
