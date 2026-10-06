// A store's change signal for React's useSyncExternalStore: listeners hear that something
// changed, then read the store's current (immutable) snapshot.

export class Emitter {
  readonly #listeners = new Set<() => void>();

  /** Hears every change until the returned function is called. */
  readonly subscribe = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => {
      this.#listeners.delete(listener);
    };
  };

  emit(): void {
    for (const listener of [...this.#listeners]) {
      listener();
    }
  }
}
