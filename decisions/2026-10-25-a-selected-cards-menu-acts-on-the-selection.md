# A selected card's menu acts on the whole selection

The question, from fixing the graph's multi-select: with several cards selected, a right-click on one of them opened that one card's menu, so its actions reached only that card while the inspector acted on all of them. What should the menu of a card in a selection of several offer?

The call: the selection's menu. Under an "N selected" label it lists the selection's actions that every member can take, in the inspector's order (Status: Start, Mark done, Reopen, Skip; then Assign owner and Snooze, Unsnooze), then Remove <the card> from selection and Clear selection. A plain move is sent at once as the one patch the inspector would send; an action that needs words (Skip, Assign owner, Snooze) opens its form in the inspector's selection panel. An action some member cannot take is left out, as a node's own menu lists only what applies; the inspector's menus still show it disabled with the reason. While the structure is edited, or where the page offers no write, it lists only the two selection entries. A right-click on a card outside the selection keeps that card's own menu and leaves the selection alone.

Alternatives: keep the single card's menu (what a right-click did, which acts on less than the user selected); let a right-click outside the selection replace it with that card, as file managers do (it throws away a selection the user may have built up by hand, for a gesture that only asked for a menu); show the selection's actions disabled with reasons, as the inspector does (a context menu has no tooltip to give the reason on).

What would change it: people wanting a single card's own actions (Rename, Pin a date) while it sits in a selection, which would argue for a submenu holding the card's own menu.
