<script setup lang="ts">
/**
 * The touch layout's row menu: a long press opens it, so it is a sheet at the
 * bottom of the screen rather than a panel hanging off a trigger button that
 * does not exist.
 *
 * It exists because a finger has no hover to discover a control with, and
 * because assistive technology consumes horizontal drags — without this the
 * row's actions would be unreachable for both.
 */
import { onBeforeUnmount, onMounted } from "vue";
import { blockKeyboard } from "../lib/keyboard-blocks";
import type { MenuItem } from "../lib/menu";

defineProps<{
  /** What the menu acts on, so the sheet says which row it is about. */
  title: string;
  items: MenuItem[];
}>();

const emit = defineEmits<{
  select: [key: string];
  close: [];
}>();

// An open sheet owns the keyboard: the container must not move the entry
// cursor behind it.
let releaseKeyboard: (() => void) | null = null;

onMounted(() => {
  releaseKeyboard = blockKeyboard();
});

onBeforeUnmount(() => {
  releaseKeyboard?.();
  releaseKeyboard = null;
});
</script>

<template>
  <div class="touch-menu-overlay" @click.self="emit('close')">
    <div class="touch-menu" role="menu" aria-label="Entry actions">
      <p class="touch-menu-title">{{ title }}</p>
      <button
        v-for="item in items"
        :key="item.key"
        type="button"
        role="menuitem"
        class="touch-menu-item"
        :class="{ danger: item.danger }"
        @click="emit('select', item.key)"
      >
        {{ item.label }}
      </button>
      <button type="button" class="touch-menu-cancel" @click="emit('close')">
        Cancel
      </button>
    </div>
  </div>
</template>

<style scoped>
.touch-menu-overlay {
  position: fixed;
  inset: 0;
  z-index: 30;
  display: flex;
  align-items: flex-end;
  background: rgba(0, 0, 0, 0.32);
}
.touch-menu {
  width: 100%;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
  padding: 0.6rem 0.6rem calc(env(safe-area-inset-bottom, 0px) + 0.6rem);
  background: #fff;
  border-radius: 12px 12px 0 0;
  box-shadow: 0 -2px 16px rgba(0, 0, 0, 0.2);
}
.touch-menu-title {
  margin: 0.1rem 0.3rem 0.5rem;
  font-size: 0.8rem;
  color: #777;
  /* One line: the sheet names the row, it does not reproduce it. */
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.touch-menu-item,
.touch-menu-cancel {
  min-height: 44px;
  border: none;
  border-radius: 8px;
  background: transparent;
  font: inherit;
  font-size: 0.95rem;
  text-align: left;
  padding: 0 0.75rem;
  cursor: pointer;
}
.touch-menu-item:active,
.touch-menu-cancel:active {
  background: #e6ebf2;
}
.touch-menu-item.danger {
  color: #c0392b;
}
.touch-menu-cancel {
  margin-top: 0.3rem;
  color: #555;
  text-align: center;
}
</style>
