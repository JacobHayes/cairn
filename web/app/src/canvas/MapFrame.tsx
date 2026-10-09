// Below 720px a canvas inside a scrolling page would capture every swipe and the page would
// stop scrolling whenever a finger landed on it. So there it is an inert preview (the fitted
// view, no gestures, `inert`) with one button, and the map opens full screen: a fixed layer
// over the page, where the gestures belong to the canvas. The address gains `map=1`, so Back
// closes it; the page behind keeps its scroll position (app.css stops it scrolling while the
// map is open). From 720px the canvas is drawn as it is, in a region that does not scroll.
import { useEffect, useSyncExternalStore, type ReactNode } from "react";
import { useSearchParams } from "react-router";

import { Button } from "../ui/kit.tsx";
import { MAP } from "./settings.ts";

const NARROW = "(max-width: 719px)";

/** Whether the window is a phone's width. */
export function useNarrow(): boolean {
  return useSyncExternalStore(
    (listener) => {
      const query = globalThis.matchMedia(NARROW);
      query.addEventListener("change", listener);
      return () => {
        query.removeEventListener("change", listener);
      };
    },
    () => globalThis.matchMedia(NARROW).matches,
    () => false,
  );
}

/** The full-screen map's bar: what the map is of, and the way back. */
function MapBar({ title, onClose }: { title: string; onClose: () => void }) {
  return (
    <div className="map-bar">
      <span className="label">{title}</span>
      <span className="spacer" />
      <Button aria-label="Close the map" onClick={onClose}>
        ×
      </Button>
    </div>
  );
}

/** `children` draws the canvas; `inert` asks it for the preview, with no gestures. `title` names the map in its bar. */
export function MapFrame({ title, children }: { title: string; children: (inert: boolean) => ReactNode }) {
  const narrow = useNarrow();
  const [params, setParams] = useSearchParams();
  const open = narrow && params.get(MAP) === "1";
  const setMap = (on: boolean) => {
    const next = new URLSearchParams(params);
    if (on) {
      next.set(MAP, "1");
    } else {
      next.delete(MAP);
    }
    // Opening adds a step to the history (Back closes); closing from the layer's own button replaces it.
    setParams(next, { replace: !on });
  };
  useEffect(() => {
    if (!open) {
      return undefined;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) {
        setMap(false);
      }
    };
    globalThis.addEventListener("keydown", onKey);
    return () => {
      globalThis.removeEventListener("keydown", onKey);
    };
  });
  if (!narrow) {
    return children(false);
  }
  if (open) {
    return (
      <>
        <div className="map-preview" aria-hidden="true" />
        <div className="map-full" role="dialog" aria-label="Map" data-testid="map-full">
          <MapBar title={title} onClose={() => { setMap(false); }} />
          <div className="map-canvas">{children(false)}</div>
        </div>
      </>
    );
  }
  return (
    <div className="map-preview" data-testid="map-preview">
      <div className="map-inert" inert>
        {children(true)}
      </div>
      <Button
        primary
        className="map-open"
        data-testid="map-open"
        onClick={() => {
          setMap(true);
        }}
      >
        Open map
      </Button>
    </div>
  );
}
