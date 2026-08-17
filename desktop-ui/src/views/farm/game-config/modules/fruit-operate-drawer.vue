<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import {
  fetchAddFarmGameConfigFruit,
  fetchGetFarmGameConfigPlants,
  fetchModifyFarmGameConfigFruit
} from '@/service/api';
import { useFormRules, useNaiveForm } from '@/hooks/common/form';
import { $t } from '@/locales';
import { priceIdOptions, rarityOptions } from '../shared';

defineOptions({ name: 'FarmGameConfigFruitDrawer' });

interface Props {
  operateType: NaiveUI.TableOperateType;
  rowData?: Api.Farm.GameConfigFruit | null;
}

const props = defineProps<Props>();

interface Emits {
  (e: 'submitted'): void;
}

const emit = defineEmits<Emits>();

const visible = defineModel<boolean>('visible', { default: false });

const { formRef, validate, restoreValidation } = useNaiveForm();
const { defaultRequiredRule } = useFormRules();

const submitting = ref(false);
const plantOptions = ref<{ label: string; value: number }[]>([]);

type Model = Api.Farm.GameConfigFruitWriteParams;

const model = ref<Model>(createDefaultModel());

const title = computed(() =>
  props.operateType === 'add' ? $t('page.farm.gameConfig.addFruit') : $t('page.farm.gameConfig.editFruit')
);

const rules = {
  name: defaultRequiredRule,
  plantId: defaultRequiredRule
};

function createDefaultModel(): Model {
  return {
    id: undefined,
    plantId: undefined,
    name: '',
    price: 0,
    priceId: 0,
    desc: '',
    effectDesc: '',
    rarity: 0,
    maxCount: 9999,
    level: 0,
    fruitCount: 200,
    assetName: ''
  };
}

async function loadPlants() {
  const { data, error } = await fetchGetFarmGameConfigPlants();
  if (error || !data) {
    plantOptions.value = [];
    return;
  }
  plantOptions.value = data.map((p: any) => ({
    label: `${p.name} (种子${p.seedId ?? '-'} / 植物${p.plantId})`,
    value: p.plantId
  }));
}

function handlePlantChange(plantId: number | null) {
  if (!plantId) return;
  const opt = plantOptions.value.find(o => o.value === plantId);
  if (opt && !model.value.name) {
    model.value.name = opt.label.split(' (')[0] || '';
  }
}

watch(visible, async val => {
  if (!val) return;
  restoreValidation();
  await loadPlants();
  if (props.operateType === 'edit' && props.rowData) {
    const row = props.rowData;
    model.value = {
      id: row.id,
      plantId: row.plantId ?? undefined,
      name: row.name,
      price: row.price || 0,
      priceId: row.priceId || 0,
      desc: row.desc || '',
      effectDesc: row.effectDesc || '',
      rarity: row.rarity || 0,
      maxCount: row.maxCount || 9999,
      level: row.level || 0,
      fruitCount: 200,
      assetName: row.assetName || ''
    };
  } else {
    model.value = createDefaultModel();
  }
});

async function handleSubmit() {
  await validate();
  submitting.value = true;
  const api = props.operateType === 'add' ? fetchAddFarmGameConfigFruit : fetchModifyFarmGameConfigFruit;
  const { error } = await api(model.value);
  submitting.value = false;
  if (!error) {
    window.$message?.success($t('common.updateSuccess'));
    visible.value = false;
    emit('submitted');
  }
}
</script>

<template>
  <NDrawer v-model:show="visible" display-directive="show" :width="480">
    <NDrawerContent :title="title" :native-scrollbar="false" closable>
      <NForm ref="formRef" :model="model" :rules="rules" label-placement="left" :label-width="100">
        <NFormItem v-if="operateType === 'edit'" :label="$t('page.farm.gameConfig.fruitId')">
          <NInputNumber v-model:value="model.id" class="w-full" disabled :show-button="false" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.plant')" path="plantId">
          <NSelect
            v-model:value="model.plantId"
            :options="plantOptions"
            filterable
            :disabled="operateType === 'edit'"
            @update:value="handlePlantChange"
          />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.name')" path="name">
          <NInput v-model:value="model.name" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.price')">
          <NInputNumber v-model:value="model.price" class="w-full" :min="0" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.priceId')">
          <NSelect v-model:value="model.priceId" :options="priceIdOptions" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.rarity')">
          <NSelect v-model:value="model.rarity" :options="rarityOptions" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.maxCount')">
          <NInputNumber v-model:value="model.maxCount" class="w-full" :min="0" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.harvestCount')">
          <NInputNumber v-model:value="model.fruitCount" class="w-full" :min="0" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.assetName')">
          <NInput v-model:value="model.assetName" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.desc')">
          <NInput v-model:value="model.desc" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.effectDesc')">
          <NInput v-model:value="model.effectDesc" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" />
        </NFormItem>
      </NForm>
      <template #footer>
        <NSpace :size="16">
          <NButton @click="visible = false">{{ $t('common.cancel') }}</NButton>
          <NButton type="primary" :loading="submitting" @click="handleSubmit">
            {{ $t('common.confirm') }}
          </NButton>
        </NSpace>
      </template>
    </NDrawerContent>
  </NDrawer>
</template>
