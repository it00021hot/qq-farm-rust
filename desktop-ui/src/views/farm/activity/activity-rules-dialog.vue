<script setup lang="ts">
import { NCard, NModal } from 'naive-ui';
import { $t } from '@/locales';

export type ActivityRulesDto = {
  title?: string;
  paragraphs?: string[];
};

const props = withDefaults(
  defineProps<{
    open: boolean;
    rules?: ActivityRulesDto | null;
    title?: string;
  }>(),
  {
    rules: null,
    title: ''
  }
);

const emit = defineEmits<{ close: [] }>();

function handleShowChange(show: boolean) {
  if (!show) emit('close');
}
</script>

<template>
  <NModal
    :show="props.open"
    :mask-closable="true"
    :close-on-esc="true"
    :auto-focus="false"
    @update:show="handleShowChange"
  >
    <NCard
      class="w-520px max-w-[calc(100vw-32px)]"
      role="dialog"
      aria-modal="true"
      :bordered="false"
      :title="props.title || props.rules?.title || $t('page.farm.activity.qixiRules')"
      :closable="true"
      @close="emit('close')"
    >
      <div class="max-h-68vh overflow-y-auto">
        <slot name="guide" />
        <slot>
          <div v-if="props.rules?.paragraphs?.length" class="text-14px leading-relaxed">
            <p v-for="(paragraph, index) in props.rules.paragraphs" :key="index" class="mb-12px whitespace-pre-line">
              {{ paragraph }}
            </p>
          </div>
          <p v-else class="py-32px text-center text-gray-400">
            {{ $t('page.farm.activity.rulesEmpty') }}
          </p>
        </slot>
      </div>
    </NCard>
  </NModal>
</template>
