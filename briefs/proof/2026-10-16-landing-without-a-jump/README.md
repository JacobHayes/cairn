# Proof: opening Cairn without a jump

Opening Cairn at its address now shows the journey index straight away. The page used to say
"Reading your journeys...", read every journey's own list, and then move to Mine or the
index, so the screen and the address changed after the page had loaded. Now `/` reads
nothing: the address becomes `/journeys` before anything is painted, and the screen stays
where it is. Mine is one click away in the rail.

![The journey index, the first and only screen painted when Cairn is opened](1-opening-cairn.png)

*Opening `/` on the in-browser host: the index, with Journeys current in the rail.*

| Opening `/` | Before | After |
|---|---|---|
| First screen painted | a one-line "reading" notice | the journey index |
| Address after load | `/mine` or `/journeys`, once every journey was read | `/journeys`, from the first paint |
| Data read to decide | the viewer, the index, every journey's "mine" | none |

The decision is recorded in `decisions/2026-10-16-the-landing-goes-straight-to-the-journey-index.md`.
`prove.sh` regenerates the picture.

## Known limits

- A person who mostly wants Mine opens one click away from it; the decision record says what
  would move `/` to Mine.
