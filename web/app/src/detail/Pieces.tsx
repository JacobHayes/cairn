// A sentence's pieces drawn (sentence.ts): text, links to nodes, and the Unsnooze action.
import { Fragment } from "react";

import { Button } from "../ui/kit.tsx";
import { titleOf, type Ready } from "./model.ts";
import { NodeLink } from "./parts.tsx";
import type { Piece } from "./sentence.ts";

export function Pieces({ view, pieces, onUnsnooze, disabled = false }: { view: Ready; pieces: Piece[]; onUnsnooze?: (container: string) => void; disabled?: boolean }) {
  return (
    <>
      {pieces.map((piece, at) => (
        <Fragment key={at}>
          {typeof piece === "string" ? (
            piece
          ) : "node" in piece ? (
            <NodeLink view={view} node={piece.node} />
          ) : onUnsnooze === undefined ? null : (
            <Button
              disabled={disabled}
              data-testid="unsnooze-container"
              onClick={() => {
                onUnsnooze(piece.unsnooze);
              }}
            >
              Unsnooze {titleOf(view, piece.unsnooze)}
            </Button>
          )}
        </Fragment>
      ))}
    </>
  );
}
