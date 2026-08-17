<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import {
  fetchAddFarmGameConfigItem,
  fetchGetFarmGameConfigItemTypes,
  fetchModifyFarmGameConfigItem
} from '@/service/api';
import { useFormRules, useNaiveForm } from '@/hooks/common/form';
import { $t } from '@/locales';
import { priceIdOptions, rarityOptions } from '../shared';

defineOptions({ name: 'FarmGameConfigItemDrawer' });

interface Props {
  operateType: NaiveUI.TableOperateType;
  rowData?: Api.Farm.GameConfigItem | null;
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
const typeOptions = ref<{ label: string; value: number }[]>([]);

type Model = Api.Farm.GameConfigItemWriteParams;

const model = ref<Model>(createDefaultModel());

const title = computed(() =>
  props.operateType === 'add' ? $t('page.farm.gameConfig.addItem') : $t('page.farm.gameConfig.editItem')
);

const canUseOptions = [
  { label: '否', value: 0 },
  { label: '是', value: 1 }
];

const rules = {
  id: defaultRequiredRule,
  name: defaultRequiredRule,
  type: defaultRequiredRule
};

function createDefaultModel(): Model {
  return {
    id: 0,
    type: 7,
    name: '',
    price: 0,
    priceId: 0,
    interactionType: '',
    canUse: 0,
    desc: '',
    effectDesc: '',
    rarity: 0,
    maxCount: 9999,
    level: 0,
    assetName: ''
  };
}

async function loadTypes() {
  const { data, error } = await fetchGetFarmGameConfigItemTypes();
  if (error || !data) {
    typeOptions.value = [];
    return;
  }
  typeOptions.value = data.map((t: any) => ({
    label: `${t.value} - ${t.label}`,
    value: t.value
  }));
}

watch(visible, async val => {
  if (!val) return;
  restoreValidation();
  await loadTypes();
  if (props.operateType === 'edit' && props.rowData) {
    const row = props.rowData;
    model.value = {
      id: row.id,
      type: row.type,
      name: row.name,
      price: row.price || 0,
      priceId: row.priceId || 0,
      interactionType: row.interactionType || '',
      canUse: row.canUse || 0,
      desc: row.desc || '',
      effectDesc: row.effectDesc || '',
      rarity: row.rarity || 0,
      maxCount: row.maxCount || 9999,
      level: row.level || 0,
      assetName: row.assetName || ''
    };
  } else {
    model.value = createDefaultModel();
  }
});

async function handleSubmit() {
  await validate();
  submitting.value = true;
  const api = props.operateType === 'add' ? fetchAddFarmGameConfigItem : fetchModifyFarmGameConfigItem;
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
      <NForm ref="formRef" :model="model" :rules="rules" label-placement="left" :label-width="110">
        <NFormItem :label="$t('page.farm.gameConfig.itemId')" path="id">
          <NInputNumber
            v-model:value="model.id"
            class="w-full"
            :min="1"
            :disabled="operateType === 'edit'"
            :show-button="false"
          />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.itemType')" path="type">
          <NSelect v-model:value="model.type" :options="typeOptions" filterable />
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
        <NFormItem :label="$t('page.farm.gameConfig.canUse')">
          <NSelect v-model:value="model.canUse" :options="canUseOptions" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.requiredLevel')">
          <NInputNumber v-model:value="model.level" class="w-full" :min="0" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.maxCount')">
          <NInputNumber v-model:value="model.maxCount" class="w-full" :min="0" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.interactionType')">
          <NInput v-model:value="model.interactionType" />
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
