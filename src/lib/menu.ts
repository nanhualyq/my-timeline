/**
 * The shape of one menu entry, shared by the dropdown menus (a trigger button
 * opens them) and the touch layout's long-press sheet (a finger opens it).
 *
 * The two presentations differ because their triggers do, but an item is the
 * same thing in both, so the item model lives here rather than in either
 * component.
 */
export interface MenuItem {
  key: string;
  label: string;
  /** Render in the destructive style (e.g. Delete, Remove). */
  danger?: boolean;
}
