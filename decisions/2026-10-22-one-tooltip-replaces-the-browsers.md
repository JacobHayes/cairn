# One tooltip replaces the browser's

Question: native `title` tips show late, unstyled, never on keyboard focus and never on touch. What does the app use instead?

Call: one tooltip component for the whole app, and no native `title` anywhere (a browser test fails a page that carries one). Guidance appears when someone needs it, can be dismissed, and does not come back once seen.

- A tip describes, names an icon-only control, or shows a cut-short text whole. A tip that repeats the words beside it is not drawn; a long list belongs in the inspector.
- A control that cannot act now but has a reason is `aria-disabled`, stays focusable, and says why in its tip.
- A tip shows after the pointer rests, at once on keyboard focus, and Esc only dismisses it. A fact whose only place is a tip is a toggletip, so touch and keyboard reach it.
- One placement function, `place()`, positions every floating box (tips, menus, popovers, the tour's callout). A tip is a `popover="manual"` element, so it sits in the top layer with no portal or z-index and never re-renders its trigger.

Alternatives: keeping `title` (its delay and look cannot be changed, and it has no keyboard or touch path); a positioning library (flip and slide inside the window is a few dozen lines); per-trigger React state (every card would re-render on hover); a portal per tip (the Popover API already gives the top layer).

What would change it: a browser floor below the Popover API, or a tip that has to hold a control (it becomes a popover).
