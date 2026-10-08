// Vite's client types: asset imports (`?url`) and import.meta.env.
/// <reference types="vite/client" />

/** The host this build runs on, fixed by vite.config.ts. */
declare const __CAIRN_HOST__: import("./data/host.ts").HostKind;
