// The title row both Library details share (C17, C19): the kind word, the name, a retired chip,
// the screen's own actions, and the way back to the list.
import type { ReactNode } from "react";
import { Link } from "react-router";

import type { Schema } from "@cairn/client";

import { Badge } from "../ui/kit.tsx";
import { ROUTES_PATH, SEGMENTS_PATH } from "./address.ts";

export function DetailHeader({ header, children }: { header: Schema<"RouteHeader">; children?: ReactNode }) {
  const segment = header.kind === "segment";
  return (
    <span className="row">
      <span className="label">{segment ? "Segment" : "Route"}</span>
      <h1 data-testid="route-detail-name">{header.name}</h1>
      {header.retired === true ? <Badge data-testid="retired">{segment ? "Retired: hidden from new insertions" : "Retired: hidden from new journeys"}</Badge> : null}
      <span className="spacer" />
      {children}
      <Link to={segment ? SEGMENTS_PATH : ROUTES_PATH}>{segment ? "All segments" : "All routes"}</Link>
    </span>
  );
}
