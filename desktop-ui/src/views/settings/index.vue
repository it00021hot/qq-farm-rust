<script setup lang="ts">
import { onMounted, ref, watch } from 'vue';
import { useDesktopStore } from '@/store/modules/desktop';
import SettingsSummaryCard from './modules/settings-summary.vue';
import AccountSelect from './modules/account-select.vue';

defineOptions({
  name: 'SettingsPage'
});

const desktopStore = useDesktopStore();
const selectedAccountId = ref('');

async function reloadSettings(accountId?: string) {
  await desktopStore.loadSettings(accountId);
}

onMounted(async () => {
  await desktopStore.loadAccounts();
  const first = desktopStore.accounts[0]?.id || '';
  selectedAccountId.value = first;
  await reloadSettings(first || undefined);
});

watch(selectedAccountId, async (value, oldValue) => {
  if (value === oldValue) {
    return;
  }
  await reloadSettings(value || undefined);
});
</script>

<template>
  <NSpace vertical :size="16">
    <NAlert v-if="desktopStore.errorMessage" type="error" :title="$t('common.error')">
      {{ desktopStore.errorMessage }}
    </NAlert>
    <AccountSelect v-model:value="selectedAccountId" />
    <SettingsSummaryCard />
  </NSpace>
</template>

<style scoped></style>
