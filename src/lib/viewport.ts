import { onBeforeUnmount, onMounted, ref, type Ref } from "vue";

/**
 * The width below which the app shows one pane at a time. Chosen to sit under
 * the desktop window's `minWidth` (900), so no desktop window can ever fall
 * into narrow mode, while a tablet at 800 px or wider keeps three columns.
 *
 * This constant is the only place the breakpoint is written down: layout CSS
 * keys off `[data-layout]` on the root element instead of repeating the number
 * in a media query, so the JS state and the CSS can never disagree.
 */
export const NARROW_MAX_WIDTH = 800;

export const NARROW_MEDIA_QUERY = `(max-width: ${NARROW_MAX_WIDTH}px)`;

/** True where a pointer can rest on a row: a mouse or a trackpad. False on a
 *  touch screen, where hover styling and hover-revealed controls make no
 *  sense and a keystroke is not something the reader can produce. */
export const HOVER_MEDIA_QUERY = "(hover: hover) and (pointer: fine)";

export type Layout = "narrow" | "wide";

/** The subset of `MediaQueryList` this module needs, so tests can supply one. */
export interface MediaQueryLike {
  matches: boolean;
  addEventListener?(type: "change", listener: () => void): void;
  removeEventListener?(type: "change", listener: () => void): void;
  // Safari < 14 spelling; still present in some Android WebView builds.
  addListener?(listener: () => void): void;
  removeListener?(listener: () => void): void;
}

export type MatchMedia = (query: string) => MediaQueryLike | null;

export function layoutOf(narrow: boolean): Layout {
  return narrow ? "narrow" : "wide";
}

/** The query in the environment, or null where `matchMedia` is unavailable. */
const browserMatchMedia: MatchMedia = (query) => {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") {
    return null;
  }
  return window.matchMedia(query);
};

function subscribeMatches(
  media: MatchMedia,
  query: string,
  onChange: (matches: boolean) => void,
): () => void {
  const list = media(query);
  if (list === null) {
    // No media query support (jsdom, or a very old webview): report "no" and
    // nothing to unsubscribe from.
    onChange(false);
    return () => {};
  }
  const notify = () => onChange(list.matches);
  notify();
  if (list.addEventListener) {
    list.addEventListener("change", notify);
    return () => list.removeEventListener?.("change", notify);
  }
  if (list.addListener) {
    list.addListener(notify);
    return () => list.removeListener?.(notify);
  }
  return () => {};
}

/**
 * Publish the layout to the document so CSS can select on it, and keep it in
 * sync with the viewport. Returns an unsubscribe function.
 */
export function watchLayout(options: {
  onChange?: (layout: Layout) => void;
  matchMedia?: MatchMedia;
  query?: string;
  root?: HTMLElement | null;
} = {}): () => void {
  const { onChange, matchMedia = browserMatchMedia, query = NARROW_MEDIA_QUERY } = options;
  const root =
    options.root ?? (typeof document === "undefined" ? null : document.documentElement);

  return subscribeMatches(matchMedia, query, (matches) => {
    const layout = layoutOf(matches);
    if (root !== null) root.dataset.layout = layout;
    onChange?.(layout);
  });
}

/** Reactive view of the current layout, wired to the document on mount. */
export function useLayout(matchMedia: MatchMedia = browserMatchMedia): Ref<Layout> {
  const layout = ref<Layout>("wide");
  let unwatch: (() => void) | null = null;

  onMounted(() => {
    // `watchLayout` reports synchronously, so the ref is populated by the time
    // the component's first paint happens.
    unwatch = watchLayout({ matchMedia, onChange: (next) => (layout.value = next) });
  });

  onBeforeUnmount(() => {
    unwatch?.();
    unwatch = null;
  });

  return layout;
}

/** Reactive view of whether a media query currently matches. */
export function useMediaQuery(
  query: string,
  matchMedia: MatchMedia = browserMatchMedia,
): Ref<boolean> {
  const matches = ref(false);
  let unwatch: (() => void) | null = null;

  onMounted(() => {
    unwatch = subscribeMatches(matchMedia, query, (next) => (matches.value = next));
  });

  onBeforeUnmount(() => {
    unwatch?.();
    unwatch = null;
  });

  return matches;
}
