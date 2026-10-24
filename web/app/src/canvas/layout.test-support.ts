// The one geometry check of a layout (C15), shared by the fixture tests and the generated
// property: no two siblings overlap (cards or groups), and every child lies wholly inside its
// container's box and below the container's header. Exact, with no tolerance.
import ELK from "elkjs/lib/elk.bundled.js";
import { expect } from "vitest";

import { layOut, type LayoutRequest } from "./layout.ts";

/** What is wrong with the layout of `request`, one line each; empty when it holds. */
async function geometryProblems(request: LayoutRequest): Promise<string[]> {
  const { nodes: placed } = await layOut(new ELK(), request);
  const found: string[] = [];
  const groups = new Set(request.nodes.flatMap((node) => (node.parent === undefined ? [] : [node.parent])));
  const headers = new Map(request.nodes.map((node) => [node.key, node.header ?? 0]));
  const byParent = new Map<string | undefined, string[]>();
  for (const node of request.nodes) {
    byParent.set(node.parent, [...(byParent.get(node.parent) ?? []), node.key]);
    if (placed[node.key] === undefined) {
      found.push(`${node.key} was not placed`);
    }
  }
  for (const [parent, siblings] of byParent) {
    const box = parent === undefined ? undefined : placed[parent];
    for (const key of siblings) {
      const own = placed[key];
      if (own === undefined) {
        continue;
      }
      if (box !== undefined) {
        const top = headers.get(parent ?? "") ?? 0;
        if (own.y < top) {
          found.push(`${key} covers the header of ${parent ?? ""} (y ${String(own.y)} < ${String(top)})`);
        }
        if (own.x < 0 || own.y < 0 || own.x + own.width > box.width || own.y + own.height > box.height) {
          found.push(`${key} leaves ${parent ?? ""}`);
        }
      }
    }
    for (const [at, key] of siblings.entries()) {
      for (const other of siblings.slice(at + 1)) {
        const [a, b] = [placed[key], placed[other]];
        if (a !== undefined && b !== undefined && a.x < b.x + b.width && b.x < a.x + a.width && a.y < b.y + b.height && b.y < a.y + a.height) {
          const kind = groups.has(key) && groups.has(other) ? "groups" : "siblings";
          found.push(`${kind} ${key} and ${other} overlap`);
        }
      }
    }
  }
  return found;
}

export async function expectGeometry(name: string, request: LayoutRequest): Promise<void> {
  const found = await geometryProblems(request);
  expect(found, `${name}: ${found.slice(0, 5).join("; ")}\nthe graph: ${JSON.stringify(request)}`).toEqual([]);
}

