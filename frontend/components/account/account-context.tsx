"use client";

import * as React from "react";
import type { Profile } from "@/lib/contracts";

type AccountState = { profile: Profile; setProfile: (profile: Profile) => void };

const AccountContext = React.createContext<AccountState | null>(null);

/** Holds the signed-in customer's profile as returned by GET /api/v1/me. */
export function AccountProvider({ initial, children }: { initial: Profile; children: React.ReactNode }) {
  const [profile, setProfile] = React.useState(initial);
  // A fresh server render (for example after router.refresh) supersedes local state.
  React.useEffect(() => setProfile(initial), [initial]);
  const value = React.useMemo(() => ({ profile, setProfile }), [profile]);
  return <AccountContext.Provider value={value}>{children}</AccountContext.Provider>;
}

export function useAccount(): AccountState {
  const value = React.useContext(AccountContext);
  if (!value) throw new Error("useAccount must be used inside AccountProvider");
  return value;
}
