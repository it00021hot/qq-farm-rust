<script setup lang="ts">
import { NCard } from 'naive-ui';
import MysteryShopPanel from './panel.vue';

defineOptions({ name: 'FarmMysteryShop' });
</script>

<template>
  <div class="h-full min-h-500px flex-col-stretch gap-16px overflow-auto">
    <NCard :bordered="false" class="card-wrapper" title="神秘商人">
      <MysteryShopPanel />
    </NCard>
  </div>
</template>
