// A sentence's pieces drawn (sentence.ts): text and links to nodes.
import { Fragment } from "react";

import type { Ready } from "./model.ts";
import { NodeLink } from "./parts.tsx";
import type { Piece } from "./sentence.ts";

export function Pieces({ view, pieces }: { view: Ready; pieces: Piece[] }) {
  return (
    <>
      {pieces.map((piece, at) => (
        <Fragment key={at}>{typeof piece === "string" ? piece : <NodeLink view={view} node={piece.node} />}</Fragment>
      ))}
    </>
  );
}
