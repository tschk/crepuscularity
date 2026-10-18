<script lang="ts">
 import {onDestroy} from 'svelte';
 let {first,second,destroyed}:{first:()=>number;second:()=>number;destroyed:()=>void}=$props();
 onDestroy(destroyed);
</script>
<section><p>{first()}</p><input value={second()}/></section>
