<script setup lang="ts">
import { toRaw } from 'vue';
import { jsonClone } from '@sa/utils';
import { useNaiveForm } from '@/hooks/common/form';
import { $t } from '@/locales';
import { rarityOptions, seasonOptions, type GameConfigTab } from '../shared';

defineOptions({
  name: 'FarmGameConfigSearch'
});

interface Props {
  tab: GameConfigTab;
  itemTypeOptions?: { label: string; value: number }[];
}

withDefaults(defineProps<Props>(), {
  itemTypeOptions: () => []
});

interface Emits {
  (e: 'search'): void;
}

const emit = defineEmits<Emits>();

const { formRef, validate, restoreValidation } = useNaiveForm();

const model = defineModel<Api.Farm.GameConfigSearchParams>('model', { required: true });

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
</script>

<template>
  <NCard :bordered="false" size="small" class="card-wrapper">
    <NCollapse>
      <NCollapseItem :title="$t('common.search')" name="farm-game-config-search">
        <NForm ref="formRef" :model="model" label-placement="left" :label-width="80">
          <NGrid responsive="screen" item-responsive>
            <NFormItemGi span="24 s:12 m:6" :label="$t('page.farm.gameConfig.keyword')" path="keyword">
              <NInput v-model:value="model.keyword" :placeholder="$t('common.keywordSearch')" />
            </NFormItemGi>
            <NFormItemGi
              v-if="tab === 'seeds'"
              span="24 s:12 m:6"
              :label="$t('page.farm.gameConfig.seasons')"
              path="seasons"
            >
              <NSelect
                v-model:value="model.seasons"
                :options="seasonOptions"
                clearable
                :placeholder="$t('page.farm.gameConfig.seasons')"
              />
            </NFormItemGi>
            <NFormItemGi
              v-if="tab === 'fruits'"
              span="24 s:12 m:6"
              :label="$t('page.farm.gameConfig.rarity')"
              path="rarity"
            >
              <NSelect
                v-model:value="model.rarity"
                :options="rarityOptions"
                clearable
                :placeholder="$t('page.farm.gameConfig.rarity')"
              />
            </NFormItemGi>
            <NFormItemGi
              v-if="tab === 'items'"
              span="24 s:12 m:6"
              :label="$t('page.farm.gameConfig.itemType')"
              path="itemType"
            >
              <NSelect
                v-model:value="model.itemType"
                :options="itemTypeOptions"
                clearable
                :placeholder="$t('page.farm.gameConfig.itemType')"
              />
            </NFormItemGi>
            <NFormItemGi
              v-if="tab === 'items'"
              span="24 s:12 m:6"
              :label="$t('page.farm.gameConfig.rarity')"
              path="rarity"
            >
              <NSelect
                v-model:value="model.rarity"
                :options="rarityOptions"
                clearable
                :placeholder="$t('page.farm.gameConfig.rarity')"
              />
            </NFormItemGi>
            <NFormItemGi suffix span="24 s:12 m:6" :show-feedback="false">
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
