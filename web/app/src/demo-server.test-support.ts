// A `cairn demo` of a test's own: the real binary over the fixtures in memory, on a port of its
// own, so a test starts from the fixtures as seeded and nothing it writes reaches another test.
// The browser tests start one for each test (e2e/test.ts), the web unit tests that read the
// fixtures one for each file (authoring/engine.test-support.ts). The binary is built once
// beforehand, by the run's global setup (`scripts/demo-binary`), which names it and the web
// build to serve in the environment.
import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { createServer } from "node:net";
import { fileURLToPath } from "node:url";

const REPO = fileURLToPath(new URL("../../..", import.meta.url));

/** A demo that is answering. */
export interface Demo {
  /** Its origin, `http://127.0.0.1:<port>`. */
  origin: string;
  /** Stops it. */
  stop(): Promise<void>;
}

/** Builds the binary (nothing to do when it is current) and gives its path. */
export function demoBinary(): string {
  return execFileSync(`${REPO}/scripts/demo-binary`, { encoding: "utf8", stdio: ["ignore", "pipe", "inherit"] }).trim();
}

/** Starts a demo from the binary and web build the run's global setup named. */
export function startNamedDemo(): Promise<Demo> {
  const [binary, webDir] = [process.env["CAIRN_DEMO_BINARY"], process.env["CAIRN_DEMO_WEB"]];
  if (binary === undefined || webDir === undefined) {
    throw new Error("CAIRN_DEMO_BINARY and CAIRN_DEMO_WEB are not set: the run's global setup names them");
  }
  return startDemo(binary, webDir);
}

/** A port nothing listens on now. */
function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const probe = createServer();
    probe.once("error", reject);
    probe.listen(0, "127.0.0.1", () => {
      const address = probe.address();
      probe.close(() => {
        if (address === null || typeof address === "string") {
          reject(new Error("no port"));
        } else {
          resolve(address.port);
        }
      });
    });
  });
}

/** The shell script that runs the demo and stops it when its parent, or the shell, ends. */
const WATCH = 'parent=$PPID; "$0" demo --listen "$1" & demo=$!; trap \'kill "$demo"; exit\' TERM INT; while kill -0 "$parent" 2>/dev/null && kill -0 "$demo" 2>/dev/null; do sleep 0.5; done; kill "$demo" 2>/dev/null; wait "$demo"';

/** Whether the demo at `origin` answers, or the process has stopped (`undefined`). */
async function answers(origin: string, child: ChildProcess): Promise<boolean | undefined> {
  if (child.exitCode !== null || child.signalCode !== null) {
    return undefined;
  }
  try {
    return (await fetch(`${origin}/api/capabilities`)).ok;
  } catch {
    return false;
  }
}

/**
 * Starts `binary demo` serving the web build in `webDir` on a free port, once it answers. A port
 * taken between the probe and the bind stops the process at once, and the start is tried again.
 */
export async function startDemo(binary: string, webDir: string): Promise<Demo> {
  for (let attempt = 0; attempt < 5; attempt += 1) {
    const port = await freePort();
    const origin = `http://127.0.0.1:${String(port)}`;
    // The demo runs under a small shell that stops it when this process is gone (even killed
    // outright) or when the shell itself is stopped, so no run leaves one behind.
    const child = spawn("sh", ["-c", WATCH, binary, `127.0.0.1:${String(port)}`], { env: { ...process.env, CAIRN_WEB_DIR: webDir }, stdio: "ignore" });
    // A run that ends by exiting stops it at once.
    const kill = (): void => {
      child.kill();
    };
    process.once("exit", kill);
    for (;;) {
      const up = await answers(origin, child);
      if (up === true) {
        return {
          origin,
          stop: async () => {
            process.off("exit", kill);
            if (child.exitCode === null && child.signalCode === null) {
              child.kill();
              await once(child, "exit");
            }
          },
        };
      }
      if (up === undefined) {
        process.off("exit", kill);
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, 25));
    }
  }
  throw new Error("cairn demo did not start");
}
