<script setup lang="ts">
import { computed } from 'vue';
import { useDesktopStore } from '@/store/modules/desktop';
import { $t } from '@/locales';

defineOptions({
  name: 'SettingsSummaryCard'
});

const desktopStore = useDesktopStore();

const rows = computed(() => {
  const s = desktopStore.settings;
  if (!s) {
    return [];
  }
  return [
    { label: $t('common.config'), value: s.strategy },
    { label: 'preferredSeed', value: String(s.preferredSeed) },
    {
      label: $t('page.home.schedule'),
      value: `${s.farmIntervalSec}s (${s.farmMinSec}-${s.farmMaxSec})`
    },
    { label: 'farm', value: s.automationFarm ? $t('common.yesOrNo.yes') : $t('common.yesOrNo.no') },
    { label: 'friend', value: s.automationFriend ? $t('common.yesOrNo.yes') : $t('common.yesOrNo.no') },
    { label: 'task', value: s.automationTask ? $t('common.yesOrNo.yes') : $t('common.yesOrNo.no') },
    { label: 'sell', value: s.automationSell ? $t('common.yesOrNo.yes') : $t('common.yesOrNo.no') }
  ];
});
</script>

<template>
  <NCard :bordered="false" :title="$t('common.config')" class="card-wrapper" :loading="desktopStore.loading">
    <template #header-extra>
      <NButton
        size="small"
        :loading="desktopStore.loading"
        @click="desktopStore.loadSettings(desktopStore.settings?.accountId || undefined)"
      >
        {{ $t('common.refresh') }}
      </NButton>
    </template>
    <NEmpty v-if="!desktopStore.settings" :description="$t('common.noData')" />
    <NDescriptions v-else label-placement="left" :column="1" size="small" bordered>
      <NDescriptionsItem v-for="item in rows" :key="item.label" :label="item.label">
        {{ item.value }}
      </NDescriptionsItem>
    </NDescriptions>
  </NCard>
</template>

<style scoped></style>
