"use client";

import { RotateCw } from "lucide-react";
import * as React from "react";
import { Button } from "@/components/ui/button";
import { Notice } from "@/components/ui/notice";
import { describeFailure, request } from "@/lib/api";

const COOLDOWN_SECONDS = 60;

/** POST /api/v1/auth/resend-verification. The response never reveals whether the address is registered. */
export function ResendVerification({ email }: { email: string }) {
  const [pending, setPending] = React.useState(false);
  const [message, setMessage] = React.useState<{ tone: "success" | "error"; text: string } | null>(null);
  const [wait, setWait] = React.useState(0);

  React.useEffect(() => {
    if (wait <= 0) return;
    const timer = window.setTimeout(() => setWait(wait - 1), 1000);
    return () => window.clearTimeout(timer);
  }, [wait]);

  async function resend() {
    setPending(true);
    setMessage(null);
    const result = await request("/api/v1/auth/resend-verification", {
      method: "POST",
      session: "omit",
      json: { email },
    });
    setPending(false);
    if (result.ok) {
      setMessage({
        tone: "success",
        text: "If this address is waiting for verification, a new link is on its way. Earlier links stop working.",
      });
      setWait(COOLDOWN_SECONDS);
    } else if (result.status === 422) {
      setMessage({ tone: "error", text: "That email address isn't valid." });
    } else {
      setMessage({ tone: "error", text: describeFailure(result, "Verification email") });
      if (result.status === 429) setWait(COOLDOWN_SECONDS);
    }
  }

  return (
    <div className="grid gap-3">
      <Button variant="quiet" size="lg" onClick={resend} loading={pending} disabled={wait > 0}>
        <RotateCw aria-hidden />
        {wait > 0 ? `Resend available in ${wait}s` : "Resend verification email"}
      </Button>
      {message && <Notice tone={message.tone}>{message.text}</Notice>}
    </div>
  );
}
