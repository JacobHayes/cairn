# The journey header holds the View picker, and the status moves into the menu

Question (the journey page rework that gives every view one header and one toolbar): where do the views and the journey's status go once the toolbar holds only the controls the views share?

Call:

- **The views are a View picker in the header**, "View: Graph ▾", a menu of five links: Graph, List, Timeline, Summary and Walkthrough. The Walkthrough is the fifth view and the Walk through button is gone. Links keep real addresses (open in a new tab, copy), and the open view is marked `aria-current="page"` and drawn banded with a 2px ink edge, the active-nav marker turned vertical. The keys stay: `v` cycles Graph, List and Timeline, `w` opens the Walkthrough. One path rule serves the picker and the keys: the open node stays open when switching, except into the Walkthrough from another view (there a node's address means "opened from a card"), and the view already open keeps its whole query.
- **Status is a row in `⋯`**, "Status  Active ▸", that opens in place to the moves the status allows, and Delete once archived. It sits last, after Copy link: it is the rarest group, and Archive and Delete the heaviest moves.
- **A read-only chip beside the title, only when the journey is not active.** An archived or completed journey still reads as such at a glance and in print; active is the default and says nothing. The chip is not a button (one way to change status), and its tooltip says what the status means and where to change it. The helper line under the header is gone.
- **Menus of actions get one keyboard model.** Opened from the keyboard a menu focuses its current or first item; the arrows wrap over the items that can take focus (a disabled button cannot), Home and End jump, and Esc, choosing an item and Tab close it and return focus to its button. Menus holding controls (Filter, Depth, Signal) are unchanged. The arrow arithmetic is one function shared with the context menu.
- **The phone graph's "View" button is renamed "Controls"**: the picker owns the word View.
- **On a phone the header is always two lines**: the title and chip, then the picker and `⋯` at the right, whatever the title, status or view, so switching views never moves the page.

Alternatives: radios in a dialog like Depth (arrow keys would navigate on every step, and they are not links); a segmented control of five (does not fit beside the title on a tablet); a flyout for Status (needs its own positioning, falls off a 390px window, hover does not exist on touch) or a drill-in that replaces the menu body (a back row to learn); the status as a clickable chip (a second way to do one thing); no chip at all (an archived journey would look live); keeping "View" for both the picker and the phone controls (one word, two meanings).

What would change it: people missing the one-click status change, in which case the chip becomes the menu again; or a sixth view, which makes the picker's list long enough to group.
