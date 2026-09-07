import * as React from "react";
import { getRelayHttpUrl } from "@/shared/api/tauri";

export function supportsWorkflowHistory(document: unknown): boolean {
  if (!document || typeof document !== "object") return false;
  const extensions = (document as { supported_extensions?: unknown })
    .supported_extensions;
  return (
    Array.isArray(extensions) && extensions.includes("buzz.workflow-history.v1")
  );
}

/** Mount-scoped capability probe; late replies cannot leak across dialog/community changes. */
export function useWorkflowHistoryCapability(open: boolean): boolean {
  const [supported, setSupported] = React.useState(false);
  React.useEffect(() => {
    setSupported(false);
    if (!open) return;
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), 5000);
    void (async () => {
      try {
        const url = await getRelayHttpUrl();
        if (controller.signal.aborted) return;
        const response = await fetch(url, {
          headers: { Accept: "application/nostr+json" },
          signal: controller.signal,
        });
        const document: unknown = response.ok ? await response.json() : null;
        if (!controller.signal.aborted)
          setSupported(supportsWorkflowHistory(document));
      } catch {
        // Unsupported, malformed and unavailable relays do not advertise support.
      } finally {
        clearTimeout(timer);
      }
    })();
    return () => {
      clearTimeout(timer);
      controller.abort();
    };
  }, [open]);
  return supported;
}
