// A13: a route version, or its draft, exported as the YAML file kept on disk and handed to
// the browser to save.
import { useSession } from "../data/react.ts";
import { download, exportName } from "./model.ts";

export function useExport(route: string) {
  const { host, activity } = useSession();
  return async (version: number | undefined) => {
    try {
      const file = await host.exportRoute(route, version);
      download(exportName(route, version), host.files.text(file));
      activity.confirm("Route file exported");
    } catch (thrown) {
      activity.confirm(`Not exported: ${thrown instanceof Error ? thrown.message : String(thrown)}`, "problem");
    }
  };
}
