<script setup lang="ts">
/**
 * The undo offer that follows a batch mark.
 *
 * A batch action is immediate and stays immediate — this bar is not a
 * confirmation step, it is what makes the result recoverable. It withdraws on
 * its own when the container's window passes; it does not ask to be dismissed.
 */
defineProps<{
  /** How many entries the batch marked, which is what undo would restore. */
  count: number;
}>();

const emit = defineEmits<{ undo: [] }>();
</script>

<template>
  <div class="undo-bar" role="status" aria-live="polite">
    <span class="undo-text">
      {{ count }} {{ count === 1 ? "entry" : "entries" }} marked read
    </span>
    <button type="button" class="undo-action" @click="emit('undo')">
      Undo
    </button>
  </div>
</template>

<style scoped>
.undo-bar {
  /* Positioned by the notice stack, which places every short-lived report in
     one column so an undo offer and a refresh report cannot overlap. */
  display: flex;
  align-items: center;
  gap: 0.75rem;
  max-width: min(32rem, calc(100vw - 2rem));
  padding: 0.4rem 0.4rem 0.4rem 0.9rem;
  background: #2f2f33;
  color: #f5f5f5;
  border-radius: 6px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.28);
  font-size: 0.85rem;
}
.undo-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.undo-action {
  /* Fingertip-sized whatever the layout, since this is a short-lived target. */
  min-width: 44px;
  min-height: 44px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border: none;
  border-radius: 4px;
  background: #2d7ff9;
  color: #fff;
  font: inherit;
  font-weight: 600;
  cursor: pointer;
}
@media (hover: hover) and (pointer: fine) {
  .undo-action:hover {
    background: #4a90fa;
  }
}
</style>
