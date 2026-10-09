// The app's entry: starts the tab's session (boot.ts), then renders the shell and its
// screens under a path router: a screen's address is a real path, and the server answers
// every path it does not keep for itself (`/api/`, `/.well-known/`, `/healthz`) with this
// page, so a deep link or a reload opens its screen.
import "@design/tokens.css";
import "@design/base.css";
import "./ui/app.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter, Route, Routes } from "react-router";

import { boot } from "./boot.ts";
import { LayoutsContext } from "./canvas/hooks.ts";
import { Layouts, LayoutWorker } from "./canvas/layouts.ts";
import { SessionContext } from "./data/react.ts";
import type { Session } from "./data/session.ts";
import { Home } from "./journeys/Home.tsx";
import { JourneyIndex } from "./journeys/JourneyIndex.tsx";
import { MineScreen } from "./journeys/MineScreen.tsx";
import { NewJourney } from "./journeys/NewJourney.tsx";
import { Entities } from "./people/Entities.tsx";
import { ProposalScreen } from "./proposals/ProposalScreen.tsx";
import { Identity } from "./people/Identity.tsx";
import { RouteDetailPage } from "./routes/RouteDetail.tsx";
import { Library } from "./routes/Library.tsx";
import { BarePageRoute, JourneyDeepLink, JourneyLanding, JourneyPageRoute, LegacyRoute, SummaryRoute } from "./screens/JourneyRoutes.tsx";
import { RouteCanvasPage } from "./screens/RouteCanvasPage.tsx";
import { Shell } from "./shell/Shell.tsx";

function App({ session, layouts }: { session: Session; layouts: Layouts }) {
  return (
    <SessionContext value={session}>
      <LayoutsContext value={layouts}>
        <BrowserRouter>
          <Routes>
            <Route element={<Shell />}>
              <Route index element={<Home />} />
              <Route path="journeys" element={<JourneyIndex />} />
              <Route path="mine" element={<MineScreen />} />
              <Route path="new" element={<NewJourney />} />
              <Route path="journeys/:id" element={<JourneyLanding />} />
              <Route path="journeys/:id/nodes/:key" element={<JourneyDeepLink />} />
              <Route path="journeys/:id/next/:projection" element={<JourneyPageRoute page="next" />} />
              <Route path="journeys/:id/next/:projection/nodes/:key" element={<JourneyPageRoute page="next" />} />
              <Route path="journeys/:id/plan" element={<BarePageRoute page="plan" />} />
              <Route path="journeys/:id/plan/:projection" element={<JourneyPageRoute page="plan" />} />
              <Route path="journeys/:id/plan/:projection/nodes/:key" element={<JourneyPageRoute page="plan" />} />
              <Route path="journeys/:id/summary" element={<SummaryRoute />} />
              <Route path="journeys/:id/summary/nodes/:key" element={<SummaryRoute />} />
              <Route path="journeys/:id/:old" element={<LegacyRoute />} />
              <Route path="journeys/:id/:old/nodes/:key" element={<LegacyRoute />} />
              <Route path="library" element={<Library />} />
              <Route path="routes" element={<LegacyRoute />} />
              <Route path="routes/:id" element={<RouteCanvasPage />} />
              <Route path="routes/:id/nodes/:key" element={<RouteCanvasPage />} />
              <Route path="routes/:id/versions" element={<RouteDetailPage />} />
              <Route path="entities" element={<Entities />} />
              <Route path="me" element={<Identity />} />
              <Route path="proposals/:id" element={<ProposalScreen />} />
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
