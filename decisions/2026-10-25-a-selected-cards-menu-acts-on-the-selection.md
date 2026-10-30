# A selected card's menu acts on the whole selection

Question: with several cards selected, what does a right-click on one of them offer?

Call: the selection's menu. Under an "N selected" label it lists, in the inspector's order, the actions every member can take, then removing that card from the selection and clearing it. A plain move is sent at once as the one patch the inspector would send; an action that needs words opens its form in the inspector. An action some member cannot take is left out, since a context menu has nowhere to give the reason. A right-click on a card outside the selection opens that card's own menu and leaves the selection alone.

Alternatives: the single card's menu (acts on less than the person selected); replacing the selection with the right-clicked card, as file managers do (throws away a selection built by hand); listing actions disabled with reasons (a menu has no tip to give them).

What would change it: people wanting a card's own actions while it sits in a selection, which would argue for a submenu.
