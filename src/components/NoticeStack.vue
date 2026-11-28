<script setup lang="ts">
/**
 * The one region the app's short-lived reports live in.
 *
 * A batch mark offers an undo while a refresh may be reporting progress; both
 * are notices, so they stack instead of covering each other. The region sits
 * above the panes and the drawer and below every dialog, which is what keeps a
 * notice from ever competing with something the reader is deciding.
 */
</script>

<template>
  <div class="notice-stack">
    <slot />
  </div>
</template>

<style scoped>
.notice-stack {
  position: fixed;
  left: 50%;
  transform: translateX(-50%);
  bottom: calc(env(safe-area-inset-bottom, 0px) + 0.75rem);
  z-index: 25;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
  max-width: calc(100vw - 2rem);
  /* The region is a layout box only: it must never eat a tap that belongs to
     the list behind it. */
  pointer-events: none;
}
.notice-stack > :deep(*) {
  pointer-events: auto;
}
</style>
