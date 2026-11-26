"use client";

import * as React from "react";

/**
 * Reads a one-time link token from the URL fragment and immediately removes it
 * from the address bar and history. Fragments are never sent to a server, so the
 * token reaches only the API request that redeems it. This follows the backend's
 * own verify-email.js and reset_password.js handling.
 *
 * Returns undefined until read, then the token or null when the link had none.
 */
export function useFragmentToken(): string | null | undefined {
  const [token, setToken] = React.useState<string | null | undefined>(undefined);
  React.useEffect(() => {
    const value = window.location.hash.slice(1);
    window.history.replaceState(null, "", window.location.pathname);
    setToken(value || null);
  }, []);
  return token;
}
