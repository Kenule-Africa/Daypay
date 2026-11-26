"use client";

import * as React from "react";
import { describeFailure, endSession, request } from "@/lib/api";

/** POST /api/v1/auth/logout revokes this session server-side and clears the cookie. */
export function useSignOut() {
  const [pending, setPending] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const signOut = React.useCallback(async () => {
    setPending(true);
    setError(null);
    const result = await request("/api/v1/auth/logout", { method: "POST", session: "optional" });
    // 401 means the session had already ended; either way this browser is signed out.
    if (result.ok || result.status === 401) {
      endSession("signed_out");
      return;
    }
    setPending(false);
    setError(`We couldn't sign you out. ${describeFailure(result, "Sign-out")}`);
  }, []);
  return { signOut, pending, error };
}
