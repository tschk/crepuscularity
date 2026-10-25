<script lang="ts">
import{getContext}from'svelte';let{label}:{label:string}=$props();const value=getContext<string>('key');(globalThis as unknown as{constContextTrace:unknown[][]}).constContextTrace.push(['read',label,value]);
</script><i data-read={label}>{value}</i>
