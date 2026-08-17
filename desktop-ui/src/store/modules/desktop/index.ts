import { computed, ref } from 'vue';
import { defineStore } from 'pinia';
import { useLoading } from '@sa/hooks';
import {
  fetchDesktopAccounts,
  fetchDesktopSettings,
  fetchDesktopSnapshot,
  listenDesktopAppEvent
} from '@/service/tauri';
import { SetupStoreId } from '@/enum';

export const useDesktopStore = defineStore(SetupStoreId.Desktop, () => {
  const { loading, startLoading, endLoading } = useLoading();

  const snapshot = ref<Desktop.DesktopSnapshot | null>(null);
  const accounts = ref<Desktop.AccountSummary[]>([]);
  const settings = ref<Desktop.SettingsSummary | null>(null);
  const lastEvent = ref<Desktop.AppEventPayload | null>(null);
  const errorMessage = ref('');

  let unlisten: (() => void) | undefined;

  const runningCount = computed(() => accounts.value.filter(item => item.running).length);

  async function loadSnapshot() {
    startLoading();
    errorMessage.value = '';
    try {
      snapshot.value = await fetchDesktopSnapshot();
      accounts.value = snapshot.value.accounts;
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : String(error);
    } finally {
      endLoading();
    }
  }

  async function loadAccounts() {
    startLoading();
    errorMessage.value = '';
    try {
      accounts.value = await fetchDesktopAccounts();
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : String(error);
    } finally {
      endLoading();
    }
  }

  async function loadSettings(accountId?: string) {
    startLoading();
    errorMessage.value = '';
    try {
      settings.value = await fetchDesktopSettings(accountId);
    } catch (error) {
      errorMessage.value = error instanceof Error ? error.message : String(error);
    } finally {
      endLoading();
    }
  }

  async function startEventListen() {
    if (unlisten) {
      return;
    }
    unlisten = await listenDesktopAppEvent(payload => {
      lastEvent.value = payload;
    });
  }

  function stopEventListen() {
    unlisten?.();
    unlisten = undefined;
  }

  return {
    loading,
    snapshot,
    accounts,
    settings,
    lastEvent,
    errorMessage,
    runningCount,
    loadSnapshot,
    loadAccounts,
    loadSettings,
    startEventListen,
    stopEventListen
  };
});
