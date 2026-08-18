<script setup lang="ts">
import { computed, toRaw } from 'vue';
import { jsonClone } from '@sa/utils';
import {
  farmAuthStatusOptions,
  farmPlatformOptions,
  farmRunStatusOptions,
  translateNumberOptions,
  translateStringOptions
} from '@/constants/business';
import { useNaiveForm } from '@/hooks/common/form';
import { $t } from '@/locales';

defineOptions({
  name: 'FarmAccountSearch'
});

interface Emits {
  (e: 'search'): void;
}

const emit = defineEmits<Emits>();

const { formRef, validate, restoreValidation } = useNaiveForm();

const model = defineModel<Api.Farm.AccountSearchParams>('model', { required: true });

const defaultModel = jsonClone(toRaw(model.value));

function resetModel() {
  Object.assign(model.value, defaultModel);
}

async function reset() {
  await restoreValidation();
  resetModel();
}

async function search() {
  await validate();
  emit('search');
}

const statusOptions = computed(() => translateStringOptions(farmAuthStatusOptions));
const runStatusOptions = computed(() => translateNumberOptions(farmRunStatusOptions));
const platformOptions = computed(() => translateStringOptions(farmPlatformOptions));
</script>

<template>
  <NCard :bordered="false" size="small" class="card-wrapper">
    <NCollapse>
      <NCollapseItem :title="$t('common.search')" name="farm-account-search">
        <NForm ref="formRef" :model="model" label-placement="left" :label-width="80">
          <NGrid responsive="screen" item-responsive>
            <NFormItemGi span="24 s:12 m:6" :label="$t('page.farm.account.keyword')" path="keyword">
              <NInput v-model:value="model.keyword" :placeholder="$t('common.keywordSearch')" />
            </NFormItemGi>
            <NFormItemGi span="24 s:12 m:6" :label="$t('page.farm.account.platform')" path="platform">
              <NSelect
                v-model:value="model.platform"
                :options="platformOptions"
                clearable
                :placeholder="$t('page.farm.account.platform')"
              />
            </NFormItemGi>
            <NFormItemGi span="24 s:12 m:6" :label="$t('page.farm.account.runStatus')" path="runStatus">
              <NSelect
                v-model:value="model.runStatus"
                :options="runStatusOptions"
                clearable
                :placeholder="$t('page.farm.account.runStatus')"
              />
            </NFormItemGi>
            <NFormItemGi span="24 s:12 m:6" :label="$t('page.farm.account.authStatus')" path="authStatus">
              <NSelect
                v-model:value="model.authStatus"
                :options="statusOptions"
                clearable
                :placeholder="$t('page.farm.account.authStatus')"
              />
            </NFormItemGi>
            <NFormItemGi span="24">
              <NSpace class="w-full" justify="end">
                <NButton @click="reset">
                  <template #icon>
                    <icon-ic-round-refresh class="text-icon" />
                  </template>
                  {{ $t('common.reset') }}
                </NButton>
                <NButton type="primary" ghost @click="search">
                  <template #icon>
                    <icon-ic-round-search class="text-icon" />
                  </template>
                  {{ $t('common.search') }}
                </NButton>
              </NSpace>
            </NFormItemGi>
          </NGrid>
        </NForm>
      </NCollapseItem>
    </NCollapse>
  </NCard>
</template>

<style scoped></style>
