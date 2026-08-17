<script setup lang="ts">
import { useAuthStore } from '@/store/modules/auth';
import { $t } from '@/locales';

defineOptions({
  name: 'PwdLogin'
});

const authStore = useAuthStore();

async function handleEnter() {
  await authStore.login();
}
</script>

<template>
  <NSpace vertical :size="24">
    <NAlert type="info" :title="$t('common.tip')">
      {{ $t('page.login.pwdLogin.title') }} — LocalOwner
    </NAlert>
    <NButton type="primary" size="large" round block :loading="authStore.loginLoading" @click="handleEnter">
      {{ $t('common.confirm') }}
    </NButton>
  </NSpace>
</template>

<style scoped></style>
