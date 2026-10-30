# A card's title width is measured in the browser, not estimated

Question: card layout reserves a title's lines before it runs, while the browser wraps the real text in the real face, so a per-character width estimate can reserve a blank line or clip one. Should layout estimate or measure?

Call: measure. Before any card is laid out, the title face is loaded and words are measured with a canvas, cached per word, and layout reads widths only through that measure, so the lines it reserves are the lines the browser draws. Unit tests, which run without fonts, install a fixed model; browser tests measure the real face and fail when drawn and reserved lines differ. Two places still estimate, because the containment check catches what they get wrong: an open container's header, sized before layout gives it a width, and whether an optional chip fits.

Alternatives: a wider estimate (always errs one way); a hidden DOM element per title (a layout per card, and not available before the DOM); letting rows grow to fit (the layout needs every height before it runs).

What would change it: titles in a script whose glyphs load after boot drifting in practice, which would need a re-layout when fonts finish loading.
