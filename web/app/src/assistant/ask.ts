// Opening the assistant from a screen's own button (the Summary's empty journey): the shell's
// own opener, the one the inspector's assistant tab uses, so there is one owner of the panel.
// None when the host offers no assistant.
import { useFrameState } from "../shell/frame.tsx";

/** A function that opens the assistant panel, or undefined where none is offered. */
export function useAssistantAsk(): (() => void) | undefined {
  return useFrameState()?.openAssistant;
}
