// A screen whose code could not be loaded (it is fetched when first opened, main.tsx): in a tab
// kept open across a deployment its file is gone, as the old engine is (ARCHITECTURE, Web UI:
// version skew). The frame stays, so the sync chip and the rail do, and the screen asks for a
// reload, which loses nothing: drafts are kept per tab. Going to another screen tries again.
import { Component, type ReactNode } from "react";

import { Button } from "../ui/kit.tsx";

export class LoadFailure extends Component<{ children: ReactNode; screen: string }, { failed: boolean }> {
  override state = { failed: false };

  static getDerivedStateFromError(): { failed: boolean } {
    return { failed: true };
  }

  override componentDidUpdate(before: { screen: string }): void {
    if (this.state.failed && before.screen !== this.props.screen) {
      this.setState({ failed: false });
    }
  }

  override render(): ReactNode {
    if (!this.state.failed) {
      return this.props.children;
    }
    return (
      <p className="callout callout-bad stack" role="alert" data-testid="load-failure">
        <span>This screen could not be loaded; Cairn may have been updated. Reload to open it.</span>
        <span className="row">
          <Button
            primary
            onClick={() => {
              location.reload();
            }}
          >
            Reload
          </Button>
        </span>
      </p>
    );
  }
}
