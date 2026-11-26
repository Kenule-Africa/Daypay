"use client";

import * as React from "react";
import { initials, profileImageUrl } from "@/lib/account";
import type { Profile } from "@/lib/contracts";
import { cn } from "@/lib/utils";

/**
 * The customer's own profile image, served privately by the API with an ETag
 * per version. Next.js image optimization is deliberately not used: it would
 * fetch and cache a private, cookie-protected image on the server.
 */
export function Avatar({ profile, className }: { profile: Profile; className?: string }) {
  const url = profileImageUrl(profile);
  const [failed, setFailed] = React.useState(false);
  React.useEffect(() => setFailed(false), [url]);
  return (
    <span
      className={cn(
        "relative inline-flex shrink-0 items-center justify-center overflow-hidden rounded-full bg-brand-100 font-display font-semibold text-brand-700",
        className,
      )}
    >
      {url && !failed ? (
        // eslint-disable-next-line @next/next/no-img-element -- private, session-bound image; see component comment
        <img src={url} alt="" className="size-full object-cover" onError={() => setFailed(true)} />
      ) : (
        <span aria-hidden>{initials(profile)}</span>
      )}
    </span>
  );
}
