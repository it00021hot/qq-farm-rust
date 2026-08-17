<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';
import { useDesktopStore } from '@/store/modules/desktop';
import SnapshotCards from './modules/snapshot-cards.vue';
import AccountTable from './modules/account-table.vue';
import EventPanel from './modules/event-panel.vue';

defineOptions({
  name: 'HomePage'
});

const desktopStore = useDesktopStore();

onMounted(async () => {
  await desktopStore.loadSnapshot();
  await desktopStore.startEventListen();
});

onUnmounted(() => {
  desktopStore.stopEventListen();
});
</script>

<template>
  <NSpace vertical :size="16">
    <NAlert v-if="desktopStore.errorMessage" type="error" :title="$t('common.error')">
      {{ desktopStore.errorMessage }}
    </NAlert>
    <SnapshotCards />
    <AccountTable />
    <EventPanel />
  </NSpace>
</template>

<style scoped></style>
