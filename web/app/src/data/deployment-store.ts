// The deployment context the tab holds (E6: entities, emails, aliases; owners and entities
// feed rank), watched for the tab's whole life and refetched on a deployment tick newer than
// the revision held (H6). The journeys re-derive on the same tick (journeys.ts).
import { Tracker, type Subscription } from "@cairn/client";

import { Emitter } from "./emitter.ts";
import type { Deployment, Host } from "./host.ts";

const DEPLOYMENT = { domain: "deployment" } as const;

export class DeploymentStore extends Emitter {
  readonly #host: Host;
  readonly #tracker = new Tracker();
  #current: Deployment | undefined;
  #fetches = 0;

  constructor(host: Host, subscription: Subscription) {
    super();
    this.#host = host;
    subscription.watch(["deployment"]);
    subscription.listen({
      opened: () => {
        this.#tracker.reopened();
      },
      tick: (tick) => {
        if ("domain" in tick.of && tick.of.domain === "deployment" && this.#tracker.ticked(tick)) {
          void this.refetch();
        }
      },
    });
  }

  /** The deployment context held, once fetched. */
  get current(): Deployment | undefined {
    return this.#current;
  }

  /** How many times it was fetched: each one a tick newer than held asked for. */
  get fetches(): number {
    return this.#fetches;
  }

  async refetch(): Promise<void> {
    try {
      const deployment = await this.#host.deployment();
      this.#fetches += 1;
      this.#tracker.fetched(DEPLOYMENT, deployment.revision);
      if (this.#current === undefined || deployment.revision >= this.#current.revision) {
        this.#current = deployment;
        this.emit();
      }
    } catch {
      this.#tracker.refetchFailed(DEPLOYMENT);
    }
  }
}
