// What structure authoring adds to a page's head, a route draft's or a journey's Edit
// structure: `+ Add` (the palette that adds a node, and on a draft, Insert segment) and `Roles
// and kinds`, each a popover so the forms take no room until asked for.
import { Menu } from "../screens/Menu.tsx";
import { Button } from "../ui/kit.tsx";
import { AddNode } from "./AddNode.tsx";
import type { GraphNode } from "./graph.ts";
import { RolesAndKindsPanel } from "./RolesAndKindsPanel.tsx";
import type { Authored } from "./target.ts";

export function StructureTools({ authored, container, onAdded, onInsert }: { authored: Authored; container: string | undefined; onAdded: (node: GraphNode) => void; onInsert?: () => void }) {
  return (
    <>
      <Menu label="Add a node" testId="add-menu" role="dialog" trigger="+ Add">
        {(close) => (
          <>
            <AddNode
              authored={authored}
              container={container}
              onAdded={(node) => {
                close();
                onAdded(node);
              }}
            />
            {onInsert === undefined ? null : (
              <Button ghost data-testid="add-insert-segment" onClick={() => { close(); onInsert(); }}>
                Insert segment...
              </Button>
            )}
          </>
        )}
      </Menu>
      <Menu label="Roles and kinds" testId="roles-menu" role="dialog" trigger="Roles and kinds">
        {() => <RolesAndKindsPanel authored={authored} />}
      </Menu>
    </>
  );
}
