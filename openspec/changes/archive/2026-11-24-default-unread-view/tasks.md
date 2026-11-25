## 1. Filter state

- [x] 1.1 Change the initial `view` in `App.vue` from `"all"` to `"unread"` and verify an app spec that a fresh mount reports `view: "unread"` to the list with a total equal to the unread count.
- [x] 1.2 Stop `activeView()` from forcing `"all"` for non-global selections, keeping the Starred short-circuit; verify with an app spec that selecting a feed keeps the filter and lists only that feed's unread entries.
- [x] 1.3 Stop `setView()` from reassigning the selection to "All entries", keeping the Starred no-op; verify with an app spec that the All tab inside a feed keeps the feed selected and lists its read entries too, and that the chosen filter carries over to the next feed.

## 2. Regression coverage

- [x] 2.1 Move the paging, scope-reset, and progress specs that assume the old all-entries default onto an explicit All tab via a small `mountApp({ view })` option, and verify they still assert the same arithmetic.
- [x] 2.2 Verify the Starred view still reports `view: "all"` and hides the tabs, with an app spec.
- [x] 2.3 Verify the batch-read specs still pass with the unread filter as the default, adjusting any assertion that depended on read entries being loaded.

## 3. Checks

- [x] 3.1 Verify the full suite passes: `npm test`, `cargo test` in `src-tauri`, `npm run check`, `npm run build`.
