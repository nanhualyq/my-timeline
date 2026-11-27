<script setup lang="ts">
import { onBeforeUnmount } from "vue";
import { SECTIONS, bindingsIn } from "../lib/keymap";
import { blockKeyboard } from "../lib/keyboard-blocks";

/** Closing is the container's decision; this only reports the intent. */
const emit = defineEmits<{ close: [] }>();

// No key acts on the list while the reference is open. The container still
// handles `?` and `Escape` to close it.
const releaseKeyboard = blockKeyboard();

onBeforeUnmount(() => releaseKeyboard());
</script>

<template>
  <Teleport to="body">
    <div class="help-overlay" @click.self="emit('close')">
      <div class="help" role="dialog" aria-modal="true" aria-label="Keyboard shortcuts">
        <header class="help-header">
          <h2 class="help-title">Keyboard</h2>
          <button class="help-close" title="Close" @click="emit('close')">
            &#10005;
          </button>
        </header>
        <!-- Rendered from the binding table, so this can never describe a key
             the application does not actually honour. -->
        <div class="help-sections">
          <section v-for="section in SECTIONS" :key="section" class="help-section">
            <h3 class="help-section-title">{{ section }}</h3>
            <dl class="help-list">
              <template v-for="binding in bindingsIn(section)" :key="binding.label">
                <dt class="help-keys">
                  <kbd>{{ binding.label }}</kbd>
                </dt>
                <dd class="help-description">{{ binding.description }}</dd>
              </template>
            </dl>
          </section>
        </div>
        <footer class="help-footer">
          Press <kbd>?</kbd> or <kbd>Esc</kbd> to close. A number before a
          cursor key repeats it, for example <kbd>5j</kbd>.
        </footer>
      </div>
    </div>
  </Teleport>
</template>

<style scoped>
.help-overlay {
  position: fixed;
  inset: 0;
  z-index: 50;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.35);
  padding: 1rem;
  box-sizing: border-box;
}
.help {
  width: min(46rem, 92vw);
  max-height: 86vh;
  /* dvh where supported: the visual viewport is what is actually on screen
     when a URL bar or the on-screen keyboard is showing. */
  max-height: 86dvh;
  display: flex;
  flex-direction: column;
  background: #fff;
  border-radius: 6px;
  box-shadow: 0 8px 30px rgba(0, 0, 0, 0.3);
  overflow: hidden;
}
.help-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid #ddd;
}
.help-title {
  margin: 0;
  font-size: 1rem;
}
.help-close {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 0.9rem;
  color: #666;
  padding: 0.2rem 0.4rem;
  border-radius: 4px;
  /* A dialog close control is a tap target wherever it appears. */
  min-width: 44px;
  min-height: 44px;
}
.help-close:hover {
  background: #eee;
  color: #000;
}.help-sections {
  display: grid;
  grid-template-columns: repeat(auto-fit, minmax(14rem, 1fr));
  gap: 0.5rem 1.5rem;
  padding: 1rem;
  overflow-y: auto;
}
.help-section-title {
  margin: 0 0 0.35rem;
  font-size: 0.72rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: #888;
}
.help-list {
  display: grid;
  grid-template-columns: auto 1fr;
  gap: 0.3rem 0.6rem;
  margin: 0;
  align-items: baseline;
}
.help-keys {
  margin: 0;
}
kbd {
  display: inline-block;
  min-width: 1.1rem;
  padding: 0.1rem 0.3rem;
  border: 1px solid #ccc;
  border-bottom-width: 2px;
  border-radius: 4px;
  background: #f7f7f7;
  font: inherit;
  font-size: 0.78rem;
  text-align: center;
  white-space: nowrap;
}
.help-description {
  margin: 0;
  font-size: 0.8rem;
  color: #444;
  line-height: 1.35;
}
.help-footer {
  padding: 0.6rem 1rem;
  border-top: 1px solid #ddd;
  font-size: 0.76rem;
  color: #666;
}
</style>
