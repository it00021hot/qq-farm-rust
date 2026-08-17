<script setup lang="ts">
import { computed } from 'vue';
import { useDesktopStore } from '@/store/modules/desktop';
import { $t } from '@/locales';

defineOptions({
  name: 'SettingsAccountSelect'
});

const value = defineModel<string>('value', { default: '' });
const desktopStore = useDesktopStore();

const options = computed(() =>
  desktopStore.accounts.map(item => ({
    label: item.name || item.nick || item.id,
    value: item.id
  }))
);
</script>

<template>
  <NCard :bordered="false" :title="$t('common.userCenter')" class="card-wrapper">
    <NSelect
      v-model:value="value"
      :options="options"
      :loading="desktopStore.loading"
      clearable
      :placeholder="$t('common.keywordSearch')"
    />
  </NCard>
</template>

<style scoped></style>
