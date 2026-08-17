import { computed, reactive, ref } from 'vue';
import { useRoute } from 'vue-router';
import { defineStore } from 'pinia';
import { useLoading } from '@sa/hooks';
import { localStg } from '@/utils/storage';
import { SetupStoreId } from '@/enum';
import { useRouteStore } from '../route';
import { useTabStore } from '../tab';
import { clearAuthStorage, getToken } from './shared';

/** Personal free desktop: always LocalOwner, no panel login. */
export const useAuthStore = defineStore(SetupStoreId.Auth, () => {
  const route = useRoute();
  const authStore = useAuthStore();
  const routeStore = useRouteStore();
  const tabStore = useTabStore();
  const { loading: loginLoading, startLoading, endLoading } = useLoading();

  const token = ref('local-owner');

  const userInfo: Api.Auth.UserInfo = reactive({
    userId: 'local',
    userName: 'LocalOwner',
    roles: ['R_SUPER'],
    buttons: ['*']
  });

  const isStaticSuper = computed(() => true);
  const isLogin = computed(() => true);

  function ensureLocalSession() {
    localStg.set('token', 'local-owner');
    localStg.set('refreshToken', 'local-owner');
    token.value = 'local-owner';
    Object.assign(userInfo, {
      userId: 'local',
      userName: 'LocalOwner',
      roles: ['R_SUPER'],
      buttons: ['*']
    });
  }

  async function resetStore() {
    clearAuthStorage();
    ensureLocalSession();
    authStore.$reset();
    ensureLocalSession();
    if (!route.meta.constant) {
      // stay in app — no login redirect
    }
    tabStore.cacheTabs();
    routeStore.resetStore();
  }

  async function login(_userName?: string, _password?: string, _redirect = true) {
    startLoading();
    ensureLocalSession();
    endLoading();
  }

  async function initUserInfo() {
    ensureLocalSession();
    const maybeToken = getToken();
    if (!maybeToken) {
      ensureLocalSession();
    }
  }

  ensureLocalSession();

  return {
    token,
    userInfo,
    isStaticSuper,
    isLogin,
    loginLoading,
    resetStore,
    login,
    initUserInfo,
    ensureLocalSession
  };
});
