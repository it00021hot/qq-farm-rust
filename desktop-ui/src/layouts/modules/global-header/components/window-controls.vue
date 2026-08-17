<script setup lang="ts">
import { desktopClose, desktopMinimise, desktopToggleMaximise } from '@/utils/desktop';

defineOptions({
  name: 'WindowControls'
});
</script>

<template>
  <div class="window-controls desktop-no-drag">
    <button type="button" class="window-controls__btn" aria-label="最小化" @click="desktopMinimise">
      <span class="window-controls__icon minimise" />
    </button>
    <button type="button" class="window-controls__btn" aria-label="最大化" @click="desktopToggleMaximise">
      <span class="window-controls__icon maximise" />
    </button>
    <button type="button" class="window-controls__btn is-close" aria-label="关闭" @click="desktopClose">
      <span class="window-controls__icon close" />
    </button>
  </div>
</template>

<style scoped>
.window-controls {
  display: flex;
  align-items: stretch;
  align-self: stretch;
  height: 100%;
  margin-left: 4px;
}

.window-controls__btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 100%;
  min-height: 36px;
  border: 0;
  background: transparent;
  color: rgb(var(--base-text-color) / 0.65);
  cursor: pointer;
}

.window-controls__btn:hover {
  background: rgba(0, 0, 0, 0.06);
}

.window-controls__btn.is-close:hover {
  background: #e81123;
  color: #fff;
}

:global(html.dark) .window-controls__btn {
  color: rgba(255, 255, 255, 0.72);
}

:global(html.dark) .window-controls__btn:hover {
  background: rgba(255, 255, 255, 0.08);
}

:global(html.dark) .window-controls__btn.is-close:hover {
  background: #e81123;
  color: #fff;
}

.window-controls__icon {
  display: block;
  width: 10px;
  height: 10px;
  position: relative;
}

.window-controls__icon.minimise::before {
  content: '';
  position: absolute;
  left: 0;
  right: 0;
  top: 4px;
  height: 1px;
  background: currentColor;
}

.window-controls__icon.maximise::before {
  content: '';
  position: absolute;
  inset: 1px;
  border: 1px solid currentColor;
}

.window-controls__icon.close::before,
.window-controls__icon.close::after {
  content: '';
  position: absolute;
  left: 4px;
  top: 0;
  width: 1px;
  height: 12px;
  background: currentColor;
}

.window-controls__icon.close::before {
  transform: rotate(45deg);
}

.window-controls__icon.close::after {
  transform: rotate(-45deg);
}
</style>
