// The agreement cases the server walk wrote (crates/wasm `cases`; the cairn-wasm-cases
// binary), as the page and the tests read them.
import type { Schema } from "@cairn/client";

import type {
  ApplyRequest,
  ExportRequest,
  ImportRequest,
  PreviewRequest,
  ProjectionRequest,
} from "../src/types.ts";

/** `crates/wasm` `CaseCall`: one call the browser host makes. */
export type CaseCall =
  | { kind: "derive" }
  | { kind: "project"; request: ProjectionRequest }
  | { kind: "preview"; request: PreviewRequest }
  | { kind: "apply"; request: ApplyRequest }
  | { kind: "export"; request: ExportRequest }
  | { kind: "import"; request: ImportRequest }
  | { kind: "touched"; patch: Schema<"Patch"> };

/** One call and the server's answer, byte for byte. */
export interface Case {
  name: string;
  call: CaseCall;
  expected: string;
}

/** One domain document (the server's text) and the calls over it. */
export interface Group {
  label: string;
  document?: string;
  cases: Case[];
}

/** An entry of the cases' index. */
export interface IndexEntry {
  label: string;
  file: string;
  cases: number;
}

/** What the page found for one case. */
export interface CaseResult {
  name: string;
  kind: CaseCall["kind"];
  bytes: number;
  equal: boolean;
  firstDifference?: number;
  error?: string;
}

/** What the page found for one group. */
export interface GroupResult {
  label: string;
  documentBytes: number;
  results: CaseResult[];
}

/** The clock the server walk read and wrote at (crates/wasm `cases::NOW`). */
export const NOW = "2026-10-12T12:00:00Z";
