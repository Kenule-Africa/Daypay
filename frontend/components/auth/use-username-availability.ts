"use client";

import * as React from "react";
import { request } from "@/lib/api";
import { usernameHandle } from "@/lib/validation";

export type Availability = "idle" | "invalid" | "checking" | "available" | "taken" | "limited" | "error";

/**
 * Advisory check against GET /api/v1/usernames/availability. Registration stays
 * authoritative: a handle can still be claimed between this check and sign-up.
 */
export function useUsernameAvailability(input: string): Availability {
  const [state, setState] = React.useState<Availability>("idle");
  React.useEffect(() => {
    if (!input) {
      setState("idle");
      return;
    }
    const handle = usernameHandle(input);
    if (!handle) {
      setState("invalid");
      return;
    }
    setState("checking");
    const controller = new AbortController();
    const timer = window.setTimeout(async () => {
      try {
        const result = await request<{ available: boolean }>(
          `/api/v1/usernames/availability?username=${encodeURIComponent(handle)}`,
          { session: "omit", signal: controller.signal },
        );
        if (result.ok) setState(result.data?.available ? "available" : "taken");
        else if (result.status === 422) setState("invalid");
        else if (result.status === 429) setState("limited");
        else setState("error");
      } catch {
        // Superseded by a newer value.
      }
    }, 450);
    return () => {
      controller.abort();
      window.clearTimeout(timer);
    };
  }, [input]);
  return state;
}
