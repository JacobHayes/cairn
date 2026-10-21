// A13: a route version, or its draft, exported as the YAML file kept on disk and handed to
// the browser to save.
import { useSession } from "../data/react.ts";
import { download, exportName } from "./model.ts";

export function useExport(route: string) {
  const { host, deriver, toasts } = useSession();
  return async (version: number | undefined) => {
    try {
      const file = await host.exportRoute(route, version);
      download(exportName(route, version), await deriver.routeFileText(file));
      toasts.send({ type: "show", text: "Route file exported", tone: "confirm" });
    } catch (thrown) {
      toasts.send({ type: "show", text: `Not exported: ${thrown instanceof Error ? thrown.message : String(thrown)}`, tone: "problem" });
    }
  };
}
