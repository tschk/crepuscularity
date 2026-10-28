<script setup lang="ts">
import { ref, computed } from "vue";
const props = defineProps<{ id: string; initial: number; label: string }>();
const count = ref(props.initial);
const doubled = computed(() => count.value * 2);
function increment() {
  count.value++;
}
const api = {
  props: () => props,
  increment,
  value: () => count.value,
};
const fixture = globalThis as unknown as {
  vueSetupApis: Record<string, typeof api>;
};
fixture.vueSetupApis[props.id] = api;
</script>

<template>
  <button type="button" :data-vue-counter="props.id" @click="increment">
    {{ props.label }}:{{ count }}/{{ doubled }}
  </button>
</template>
