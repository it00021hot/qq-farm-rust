<script setup lang="ts">
import { onMounted } from 'vue';
import { useFarmAccountStore } from '@/store/modules/farm-account';
import { useAppStore } from '@/store/modules/app';
import { $t } from '@/locales';

defineOptions({ name: 'FarmAccountSwitcher' });

const farmAccountStore = useFarmAccountStore();
const appStore = useAppStore();

async function reload() {
  await farmAccountStore.loadAccounts();
}

onMounted(() => {
  farmAccountStore.initFromStorage();
  void reload();
});

function handleUpdate(value: number | null) {
  farmAccountStore.setAccountId(value);
}
</script>

<template>
  <div class="px-12px py-8px">
    <NSelect
      :value="farmAccountStore.currentAccountId"
      :options="farmAccountStore.accountOptions"
      :loading="farmAccountStore.loading"
      :placeholder="$t('page.farm.common.selectAccount')"
      :consistent-menu-width="false"
      filterable
      size="small"
      :show-arrow="!appStore.siderCollapse"
      @update:value="handleUpdate"
    />
  </div>
</template>

<style scoped></style>
