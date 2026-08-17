<script setup lang="ts">
import { useDesktopStore } from '@/store/modules/desktop';
import { $t } from '@/locales';

defineOptions({
  name: 'HomeEventPanel'
});

const desktopStore = useDesktopStore();
</script>

<template>
  <NCard :bordered="false" :title="$t('page.home.message')" class="card-wrapper">
    <NEmpty v-if="!desktopStore.lastEvent" :description="$t('common.noData')" />
    <NDescriptions v-else label-placement="left" :column="1" size="small">
      <NDescriptionsItem label="kind">
        {{ desktopStore.lastEvent.kind }}
      </NDescriptionsItem>
      <NDescriptionsItem label="account">
        {{ desktopStore.lastEvent.accountName || desktopStore.lastEvent.accountId || '-' }}
      </NDescriptionsItem>
      <NDescriptionsItem :label="$t('common.tip')">
        {{ desktopStore.lastEvent.message || '-' }}
      </NDescriptionsItem>
    </NDescriptions>
  </NCard>
</template>

<style scoped></style>
