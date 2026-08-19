<script setup lang="ts">
import { $t } from '@/locales';

defineOptions({
  name: 'GlobalLogo'
});

interface Props {
  /** Whether to show the title */
  showTitle?: boolean;
}

withDefaults(defineProps<Props>(), {
  showTitle: true
});
</script>

<template>
  <RouterLink to="/" class="w-full flex-center nowrap-hidden">
    <SystemLogo
      v-show="showTitle"
      class="text-16px text-primary font-bold transition duration-300 ease-in-out whitespace-nowrap"
    />
  </RouterLink>
</template>

<style scoped></style>
