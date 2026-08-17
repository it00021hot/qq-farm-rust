<script setup lang="ts">
import { computed, h } from 'vue';
import type { DataTableColumns } from 'naive-ui';
import { NTag } from 'naive-ui';
import { useDesktopStore } from '@/store/modules/desktop';
import { $t } from '@/locales';

defineOptions({
  name: 'HomeAccountTable'
});

const desktopStore = useDesktopStore();

const columns = computed<DataTableColumns<Desktop.AccountSummary>>(() => [
  { title: 'ID', key: 'id', width: 100 },
  { title: $t('common.userCenter'), key: 'name', ellipsis: { tooltip: true } },
  { title: 'Nick', key: 'nick', ellipsis: { tooltip: true } },
  { title: 'Platform', key: 'platform', width: 100 },
  {
    title: $t('page.home.todo'),
    key: 'running',
    width: 120,
    render: row =>
      h(
        NTag,
        { type: row.running ? 'success' : 'default', size: 'small' },
        { default: () => (row.running ? $t('common.yesOrNo.yes') : $t('common.yesOrNo.no')) }
      )
  }
]);
</script>

<template>
  <NCard :bordered="false" :title="$t('page.home.projectNews.title')" class="card-wrapper">
    <template #header-extra>
      <NButton size="small" :loading="desktopStore.loading" @click="desktopStore.loadSnapshot()">
        {{ $t('common.refresh') }}
      </NButton>
    </template>
    <NDataTable
      :columns="columns"
      :data="desktopStore.accounts"
      :loading="desktopStore.loading"
      :bordered="false"
      size="small"
    />
  </NCard>
</template>

<style scoped></style>
