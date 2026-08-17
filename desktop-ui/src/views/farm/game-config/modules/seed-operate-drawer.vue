<script setup lang="ts">
import { computed, ref, watch } from 'vue';
import { fetchAddFarmGameConfigSeed, fetchModifyFarmGameConfigSeed } from '@/service/api';
import { useFormRules, useNaiveForm } from '@/hooks/common/form';
import { $t } from '@/locales';
import { growPhaseTemplates, priceIdOptions, seasonOptions, sizeOptions } from '../shared';

defineOptions({ name: 'FarmGameConfigSeedDrawer' });

interface Props {
  operateType: NaiveUI.TableOperateType;
  rowData?: Api.Farm.GameConfigSeed | null;
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

type Model = Api.Farm.GameConfigSeedWriteParams;

const model = ref<Model>(createDefaultModel());
const phaseTemplate = ref<string | null>(null);

const title = computed(() =>
  props.operateType === 'add' ? $t('page.farm.gameConfig.addSeed') : $t('page.farm.gameConfig.editSeed')
);

const rules = {
  seedId: defaultRequiredRule,
  name: defaultRequiredRule
};

function createDefaultModel(): Model {
  return {
    seedId: 0,
    name: '',
    growPhases: growPhaseTemplates[0].value,
    landLevelNeed: 1,
    seasons: 1,
    fruitCount: 200,
    price: 0,
    priceId: 0,
    exp: 0,
    size: 0
  };
}

function handleTemplateChange(value: string | null) {
  if (value) model.value.growPhases = value;
}

watch(visible, val => {
  if (!val) return;
  phaseTemplate.value = null;
  restoreValidation();
  if (props.operateType === 'edit' && props.rowData) {
    const row = props.rowData;
    model.value = {
      seedId: row.seedId,
      name: row.name,
      growPhases: row.growPhases || '',
      landLevelNeed: row.requiredLevel || 1,
      seasons: row.seasons || 1,
      fruitCount: row.harvestCount || 200,
      price: row.price || 0,
      priceId: row.priceId || 0,
      exp: row.exp || 0,
      size: row.size || 0
    };
  } else {
    model.value = createDefaultModel();
  }
});

async function handleSubmit() {
  await validate();
  submitting.value = true;
  const api = props.operateType === 'add' ? fetchAddFarmGameConfigSeed : fetchModifyFarmGameConfigSeed;
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
        <NFormItem :label="$t('page.farm.gameConfig.seedId')" path="seedId">
          <NInputNumber
            v-model:value="model.seedId"
            class="w-full"
            :min="1"
            :disabled="operateType === 'edit'"
            :show-button="false"
          />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.name')" path="name">
          <NInput v-model:value="model.name" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.phaseTemplate')">
          <NSelect
            v-model:value="phaseTemplate"
            :options="growPhaseTemplates"
            clearable
            @update:value="handleTemplateChange"
          />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.growPhases')" path="growPhases">
          <NInput v-model:value="model.growPhases" type="textarea" :autosize="{ minRows: 2, maxRows: 4 }" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.requiredLevel')">
          <NInputNumber v-model:value="model.landLevelNeed" class="w-full" :min="0" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.seasons')">
          <NSelect v-model:value="model.seasons" :options="seasonOptions" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.harvestCount')">
          <NInputNumber v-model:value="model.fruitCount" class="w-full" :min="0" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.price')">
          <NInputNumber v-model:value="model.price" class="w-full" :min="0" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.priceId')">
          <NSelect v-model:value="model.priceId" :options="priceIdOptions" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.exp')">
          <NInputNumber v-model:value="model.exp" class="w-full" :min="0" />
        </NFormItem>
        <NFormItem :label="$t('page.farm.gameConfig.size')">
          <NSelect v-model:value="model.size" :options="sizeOptions" />
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
