<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import { blockKeyboard } from "../lib/keyboard-blocks";

/** A single dropdown menu entry. */
interface RowMenuItem {
  key: string;
  label: string;
  /** Render in the destructive style (e.g. Delete, Remove). */
  danger?: boolean;
}

const props = defineProps<{
  items: RowMenuItem[];
  /** Accessible title for the ⋮ trigger button. */
  triggerTitle: string;
  disabled?: boolean;
  /** True on a touch layout: the trigger is sized for a fingertip and the
   *  menu has no hover to rely on. */
  touch?: boolean;
}>();

const emit = defineEmits<{ select: [key: string] }>();

const root = ref<HTMLElement | null>(null);
const open = ref(false);

function toggle() {
  if (!props.disabled) open.value = !open.value;
}

function choose(key: string) {
  open.value = false;
  emit("select", key);
}

function onDocumentClick(event: MouseEvent) {
  if (open.value && root.value && !root.value.contains(event.target as Node)) {
    open.value = false;
  }
}

function onDocumentKey(event: KeyboardEvent) {
  if (event.key === "Escape") open.value = false;
}

onMounted(() => {
  document.addEventListener("click", onDocumentClick);
  document.addEventListener("keydown", onDocumentKey);
});

// An open menu owns the keyboard: the container must not move the entry cursor
// behind it. The menu keeps its own Escape handling.
let releaseKeyboard: (() => void) | null = null;
watch(open, (isOpen) => {
  if (isOpen && !releaseKeyboard) {
    releaseKeyboard = blockKeyboard();
  } else if (!isOpen && releaseKeyboard) {
    releaseKeyboard();
    releaseKeyboard = null;
  }
});

onBeforeUnmount(() => {
  document.removeEventListener("click", onDocumentClick);
  document.removeEventListener("keydown", onDocumentKey);
  releaseKeyboard?.();
  releaseKeyboard = null;
});
</script>

<template>
  <span ref="root" class="row-menu">
    <button
      type="button"
      class="row-menu-trigger"
      :class="{ 'touch-target': props.touch }"
      :title="triggerTitle"
      :aria-label="triggerTitle"
      :disabled="disabled"
      aria-haspopup="menu"
      :aria-expanded="open"
      @click.stop="toggle"
    >
      &#8942;
    </button>
    <span v-if="open" class="row-menu-panel" role="menu">
      <button
        v-for="item in items"
        :key="item.key"
        type="button"
        class="row-menu-item"
        :class="{ danger: item.danger }"
        role="menuitem"
        @click.stop="choose(item.key)"
      >
        {{ item.label }}
      </button>
    </span>
  </span>
</template>

<style scoped>
.row-menu {
  position: relative;
  display: inline-flex;
  flex-shrink: 0;
}
.row-menu-trigger {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 1rem;
  line-height: 1;
  padding: 0.15rem 0.3rem;
  border-radius: 4px;
  color: #666;
}
.row-menu-trigger:hover:not(:disabled) {
  background: #ddd;
  color: #000;
}
@media (hover: none) {
  /* No pointer to hover with: a tap must not leave the trigger hovered. */
  .row-menu-trigger:hover:not(:disabled) {
    background: transparent;
    color: #666;
  }
  .row-menu-trigger:active {
    background: #ddd;
  }
}
.row-menu-trigger:disabled {
  opacity: 0.35;
  cursor: default;
}
.row-menu-panel {
  position: absolute;
  top: 100%;
  right: 0;
  z-index: 20;
  display: flex;
  flex-direction: column;
  min-width: 8.5rem;
  padding: 0.2rem;
  background: #fff;
  border: 1px solid #ccc;
  border-radius: 4px;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.15);
}
.row-menu-item {
  border: none;
  background: transparent;
  text-align: left;
  padding: 0.35rem 0.5rem;
  font-size: 0.8rem;
  border-radius: 3px;
  cursor: pointer;
  white-space: nowrap;
}
.row-menu-item:hover {
  background: #eee;
}
@media (hover: none) {
  .row-menu-item:hover {
    background: transparent;
  }
  .row-menu-item:active {
    background: #e6ebf2;
  }
}
.row-menu-item.danger {
  color: #c0392b;
}
</style>
