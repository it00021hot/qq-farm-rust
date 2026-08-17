import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import { SetupStoreId } from '@/enum';
import { fetchGetFarmAccountList } from '@/service/api';
import { localStg } from '@/utils/storage';

export const useFarmAccountStore = defineStore(SetupStoreId.FarmAccount, () => {
  const accounts = ref<Api.Farm.Account[]>([]);
  const currentAccountId = ref<number | null>(null);
  const loading = ref(false);

  const currentAccount = computed(() => {
    if (!currentAccountId.value) return null;
    return accounts.value.find(item => item.id === currentAccountId.value) || null;
  });

  const accountOptions = computed(() =>
    accounts.value.map(item => ({
      label: item.name || item.code || String(item.id),
      value: item.id
    }))
  );

  const hasAccount = computed(() => Boolean(currentAccountId.value));

  function setAccountId(id: number | string | null | undefined) {
    if (id === null || id === undefined || id === '') {
      currentAccountId.value = null;
      localStg.remove('farmAccountId');
      return;
    }
    const num = Number(id);
    if (!Number.isFinite(num) || num <= 0) {
      currentAccountId.value = null;
      localStg.remove('farmAccountId');
      return;
    }
    currentAccountId.value = num;
    localStg.set('farmAccountId', String(num));
  }

  function clear() {
    accounts.value = [];
    setAccountId(null);
  }

  async function loadAccounts() {
    loading.value = true;
    try {
      const { data, error } = await fetchGetFarmAccountList({ current: 1, size: 200 });
      if (error || !data) {
        accounts.value = [];
        return;
      }
      accounts.value = data.records || [];

      const cached = currentAccountId.value || Number(localStg.get('farmAccountId') || 0) || null;
      const valid = cached && accounts.value.some(item => item.id === cached);
      if (valid) {
        setAccountId(cached);
        return;
      }
      if (accounts.value.length > 0) {
        setAccountId(accounts.value[0].id);
      } else {
        setAccountId(null);
      }
    } finally {
      loading.value = false;
    }
  }

  function initFromStorage() {
    const cached = localStg.get('farmAccountId');
    if (cached) {
      currentAccountId.value = Number(cached) || null;
    }
  }

  return {
    accounts,
    currentAccountId,
    currentAccount,
    accountOptions,
    hasAccount,
    loading,
    setAccountId,
    clear,
    loadAccounts,
    initFromStorage
  };
});
