<script setup lang="ts">
import { computed } from 'vue';
import { useDesktopStore } from '@/store/modules/desktop';
import { $t } from '@/locales';

defineOptions({
  name: 'HomeSnapshotCards'
});

const desktopStore = useDesktopStore();

const cardData = computed(() => [
  {
    key: 'accounts',
    title: $t('page.home.projectCount'),
    value: desktopStore.snapshot?.accountCount ?? 0,
    icon: 'mdi:account-multiple'
  },
  {
    key: 'running',
    title: $t('page.home.todo'),
    value: desktopStore.runningCount,
    icon: 'mdi:play-circle'
  },
  {
    key: 'workers',
    title: $t('page.home.message'),
    value: desktopStore.snapshot?.workerCount ?? 0,
    icon: 'mdi:robot'
  }
]);
</script>

<template>
  <NGrid cols="s:1 m:3" responsive="screen" :x-gap="16" :y-gap="16">
    <NGi v-for="item in cardData" :key="item.key">
      <NCard :bordered="false" class="card-wrapper" :loading="desktopStore.loading">
        <div class="flex items-center justify-between">
          <div>
            <p class="text-14px text-#666">{{ item.title }}</p>
            <p class="text-28px font-semibold">{{ item.value }}</p>
          </div>
          <SvgIcon :icon="item.icon" class="text-40px text-primary" />
        </div>
      </NCard>
    </NGi>
  </NGrid>
</template>

<style scoped></style>
