// The app's entry: starts the tab's session (boot.ts), then renders the shell and its
// screens under a path router: a screen's address is a real path, and the server answers
// every path it does not keep for itself (`/api/`, `/.well-known/`, `/healthz`) with this
// page, so a deep link or a reload opens its screen.
import "@design/tokens.css";
import "@design/base.css";
import "./ui/app.css";

import { lazy, StrictMode, type ReactElement } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router";

import { ADDRESSES, type AddressName } from "./address.ts";

import { boot } from "./boot.ts";
import { LayoutsContext } from "./canvas/hooks.ts";
import { Layouts, LayoutWorker } from "./canvas/layouts.ts";
import { SessionContext } from "./data/react.ts";
import type { Session } from "./data/session.ts";
import { Home } from "./journeys/Home.tsx";
import { JourneyIndex } from "./journeys/JourneyIndex.tsx";
import { JourneyPage, NoSuchPage } from "./screens/JourneyRoutes.tsx";
import { Shell } from "./shell/Shell.tsx";

const NewJourney = lazy(() => import("./journeys/NewJourney.tsx").then((module) => ({ default: module.NewJourney })));
const Entities = lazy(() => import("./people/Entities.tsx").then((module) => ({ default: module.Entities })));
const Identity = lazy(() => import("./people/Identity.tsx").then((module) => ({ default: module.Identity })));
const ProposalScreen = lazy(() => import("./proposals/ProposalScreen.tsx").then((module) => ({ default: module.ProposalScreen })));
const NewRoutePage = lazy(() => import("./routes/NewRoute.tsx").then((module) => ({ default: module.NewRoutePage })));
const RouteIndex = lazy(() => import("./routes/RouteIndex.tsx").then((module) => ({ default: module.RouteIndex })));
const RouteDetailPage = lazy(() => import("./routes/RouteDetail.tsx").then((module) => ({ default: module.RouteDetailPage })));
const RouteCanvasPage = lazy(() => import("./screens/RouteCanvasPage.tsx").then((module) => ({ default: module.RouteCanvasPage })));

/** The screen each address pattern shows (address.ts); a journey's or route's `/proposals` opens its page with the inspector on the Proposals tab. */
const SCREENS: Record<AddressName, ReactElement> = {
  home: <Home />,
  journeys: <JourneyIndex />,
  journeyNew: <NewJourney />,
  journey: <JourneyPage />,
  journeyNode: <JourneyPage />,
  journeyEdge: <JourneyPage />,
  journeyProposals: <JourneyPage />,
  journeyProposal: <ProposalScreen />,
  journeyProposalNode: <ProposalScreen />,
  routes: <RouteIndex />,
  routeNew: <NewRoutePage />,
  route: <RouteCanvasPage />,
  routeDraft: <RouteCanvasPage />,
  routeDraftNode: <RouteCanvasPage />,
  routeVersions: <RouteDetailPage />,
  routeProposals: <RouteCanvasPage />,
  routeProposal: <ProposalScreen />,
  routeProposalNode: <ProposalScreen />,
  entities: <Entities />,
  deploymentProposals: <Entities />,
  deploymentProposal: <ProposalScreen />,
  deploymentProposalNode: <ProposalScreen />,
  me: <Identity />,
};

function App({ session, layouts }: { session: Session; layouts: Layouts }) {
  return (
    <SessionContext value={session}>
      <LayoutsContext value={layouts}>
        <BrowserRouter>
          <Routes>
            <Route element={<Shell />}>
              {ADDRESSES.map(({ name, path }) => (name === "home" ? <Route key={name} index element={SCREENS.home} /> : <Route key={name} path={path.slice(1)} element={SCREENS[name]} />))}
              <Route path="*" element={<NoSuchPage />} />
            </Route>
          </Routes>
        </BrowserRouter>
      </LayoutsContext>
    </SessionContext>
  );
}

const container = document.getElementById("root");
if (container === null) {
  throw new Error("index.html has no #root");
}
const root = createRoot(container);
boot().then(
  (session) => {
    root.render(
      <StrictMode>
        <App session={session} layouts={new Layouts(new LayoutWorker())} />
      </StrictMode>,
    );
  },
  (thrown: unknown) => {
    root.render(
      <p className="callout callout-bad" role="alert">
        Cairn could not start: {thrown instanceof Error ? thrown.message : String(thrown)}
      </p>,
    );
  },
);
