"use client";

import { KeyRound, LogOut, MonitorSmartphone, RotateCw, ShieldCheck } from "lucide-react";
import * as React from "react";
import { useAccount } from "@/components/account/account-context";
import { PageHeader, Section } from "@/components/account/page-header";
import { useSignOut } from "@/components/account/use-sign-out";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { ConfirmAction, ConfirmCancel, ConfirmContent, ConfirmDialog, ConfirmTrigger } from "@/components/ui/dialog";
import { Notice } from "@/components/ui/notice";
import { formatMoment } from "@/lib/account";
import { describeFailure, endSession, request } from "@/lib/api";
import type { Session } from "@/lib/contracts";
import { cn } from "@/lib/utils";

type Load = { state: "loading" } | { state: "error"; text: string } | { state: "ready"; sessions: Session[] };

export function SecurityCenter() {
  return (
    <>
      <PageHeader
        eyebrow="Security"
        title="Sign-in and sessions"
        description="See where your account is signed in and keep your password up to date."
      />
      <div className="grid gap-5 lg:gap-6">
        <Sessions />
        <PasswordReset />
        <SignOutSection />
      </div>
    </>
  );
}

/** GET /api/v1/auth/sessions and DELETE /api/v1/auth/sessions/{id}, served by the isolated verifier. */
function Sessions() {
  const [load, setLoad] = React.useState<Load>({ state: "loading" });
  const [revoking, setRevoking] = React.useState<string | null>(null);
  const [message, setMessage] = React.useState<{ tone: "success" | "error"; text: string } | null>(null);

  const refresh = React.useCallback(async () => {
    const result = await request<Session[]>("/api/v1/auth/sessions");
    if (result.ok) {
      const sessions = [...(result.data ?? [])].sort(
        (a, b) => Number(b.is_current) - Number(a.is_current) || b.created_at.localeCompare(a.created_at),
      );
      setLoad({ state: "ready", sessions });
    } else if (result.status !== 401) {
      setLoad({ state: "error", text: describeFailure(result, "Session management") });
    }
  }, []);

  React.useEffect(() => {
    void refresh();
  }, [refresh]);

  async function revoke(session: Session): Promise<boolean> {
    const result = await request(`/api/v1/auth/sessions/${encodeURIComponent(session.session_id)}`, { method: "DELETE" });
    if (result.ok && session.is_current) {
      endSession("signed_out");
      return true;
    }
    // 404: the session had already ended, which is the outcome the customer wanted.
    if (result.ok || result.status === 404) return true;
    setMessage({ tone: "error", text: `That session wasn't signed out. ${describeFailure(result, "Session management")}` });
    return false;
  }

  async function revokeOne(session: Session) {
    setRevoking(session.session_id);
    setMessage(null);
    const done = await revoke(session);
    setRevoking(null);
    if (done && !session.is_current) {
      setMessage({ tone: "success", text: "That session has been signed out." });
      await refresh();
    }
  }

  async function revokeOthers(sessions: Session[]) {
    setRevoking("others");
    setMessage(null);
    let all = true;
    for (const session of sessions.filter(s => !s.is_current)) {
      if (!(await revoke(session))) {
        all = false;
        break;
      }
    }
    setRevoking(null);
    if (all) setMessage({ tone: "success", text: "All other sessions have been signed out." });
    await refresh();
  }

  const others = load.state === "ready" ? load.sessions.filter(s => !s.is_current) : [];

  return (
    <Section
      title="Active sessions"
      description="Each sign-in creates a session. Sign out any you don't recognise."
      action={
        others.length > 0 ? (
          <ConfirmDialog>
            <ConfirmTrigger asChild>
              <Button variant="soft" size="sm" loading={revoking === "others"} disabled={revoking !== null}>
                Sign out all others
              </Button>
            </ConfirmTrigger>
            <ConfirmContent
              title="Sign out all other sessions?"
              description={`This signs out ${others.length} other ${others.length === 1 ? "session" : "sessions"}. You'll stay signed in here.`}
            >
              <ConfirmCancel asChild>
                <Button variant="quiet">Cancel</Button>
              </ConfirmCancel>
              <ConfirmAction asChild>
                <Button variant="ink" onClick={() => load.state === "ready" && revokeOthers(load.sessions)}>
                  Sign out others
                </Button>
              </ConfirmAction>
            </ConfirmContent>
          </ConfirmDialog>
        ) : null
      }
    >
      {load.state === "loading" && (
        <ul aria-busy className="grid gap-2" aria-label="Loading sessions">
          {[0, 1].map(i => (
            <li key={i} className="flex animate-pulse items-center gap-4 py-2">
              <span className="size-10 rounded-full bg-paper-2" />
              <span className="grid flex-1 gap-2">
                <span className="h-3.5 w-40 rounded bg-paper-2" />
                <span className="h-3 w-56 rounded bg-paper-2" />
              </span>
            </li>
          ))}
        </ul>
      )}
      {load.state === "error" && (
        <div>
          <Notice tone="error">{load.text}</Notice>
          <Button variant="soft" className="mt-4" onClick={() => { setLoad({ state: "loading" }); void refresh(); }}>
            <RotateCw aria-hidden /> Try again
          </Button>
        </div>
      )}
      {load.state === "ready" && (
        <ul className="grid gap-1">
          {load.sessions.map(session => (
            <li key={session.session_id} className="flex flex-wrap items-center gap-x-4 gap-y-3 rounded-panel py-3 sm:flex-nowrap">
              <span
                className={cn(
                  "inline-flex size-10 shrink-0 items-center justify-center rounded-full",
                  session.is_current ? "bg-brand-50 text-brand-600" : "bg-paper text-ink-500",
                )}
              >
                <MonitorSmartphone aria-hidden className="size-[1.1rem]" />
              </span>
              <div className="min-w-0 flex-1">
                <p className="flex flex-wrap items-center gap-2 font-medium text-ink-900">
                  {session.is_current ? "This device" : "Another session"}
                  {session.is_current && <Badge tone="live">Current</Badge>}
                </p>
                <p className="text-sm text-ink-500">
                  Signed in {formatMoment(session.created_at)} · Expires {formatMoment(session.expires_at)}
                </p>
              </div>
              {session.is_current ? (
                <ConfirmDialog>
                  <ConfirmTrigger asChild>
                    <Button variant="ghost" size="sm" disabled={revoking !== null} className="ml-14 sm:ml-0">
                      Sign out
                    </Button>
                  </ConfirmTrigger>
                  <ConfirmContent title="Sign out of this device?" description="You'll need to sign in again to use DEYPAY here.">
                    <ConfirmCancel asChild>
                      <Button variant="quiet">Cancel</Button>
                    </ConfirmCancel>
                    <ConfirmAction asChild>
                      <Button variant="ink" onClick={() => revokeOne(session)}>
                        Sign out
                      </Button>
                    </ConfirmAction>
                  </ConfirmContent>
                </ConfirmDialog>
              ) : (
                <Button
                  variant="danger"
                  size="sm"
                  loading={revoking === session.session_id}
                  disabled={revoking !== null}
                  onClick={() => revokeOne(session)}
                  className="ml-14 sm:ml-0"
                >
                  Sign out
                </Button>
              )}
            </li>
          ))}
        </ul>
      )}
      {message && (
        <Notice tone={message.tone} className="mt-4">
          {message.text}
        </Notice>
      )}
    </Section>
  );
}

/** Password changes go through the reset email (POST /api/v1/auth/forgot-password). */
function PasswordReset() {
  const { profile } = useAccount();
  const [pending, setPending] = React.useState(false);
  const [message, setMessage] = React.useState<{ tone: "success" | "error"; text: string } | null>(null);

  async function send() {
    setPending(true);
    setMessage(null);
    const result = await request("/api/v1/auth/forgot-password", {
      method: "POST",
      session: "omit",
      json: { email: profile.email },
    });
    setPending(false);
    setMessage(
      result.ok
        ? { tone: "success", text: `We've sent a password reset link to ${profile.email}. It works once and expires after a short time.` }
        : { tone: "error", text: describeFailure(result, "Password recovery") },
    );
  }

  return (
    <Section
      title="Password"
      description="To change your password, we'll email you a secure reset link. Resetting signs you out on every device."
    >
      <div className="flex items-center gap-4">
        <span className="inline-flex size-10 shrink-0 items-center justify-center rounded-full bg-paper text-ink-500">
          <KeyRound aria-hidden className="size-[1.1rem]" />
        </span>
        <p className="min-w-0 flex-1 text-[0.9375rem] text-ink-600">
          Reset link goes to <span className="font-medium text-ink-900">{profile.email}</span>
        </p>
      </div>
      <Button variant="soft" className="mt-5" onClick={send} loading={pending}>
        Email me a reset link
      </Button>
      {message && (
        <Notice tone={message.tone} className="mt-4">
          {message.text}
        </Notice>
      )}
    </Section>
  );
}

function SignOutSection() {
  const { signOut, pending, error } = useSignOut();
  return (
    <Section title="Sign out" description="End your session on this device.">
      <div className="flex flex-wrap items-center gap-3">
        <Button variant="ink" onClick={signOut} loading={pending}>
          <LogOut aria-hidden /> Sign out of DEYPAY
        </Button>
        <span className="flex items-center gap-2 text-sm text-ink-500">
          <ShieldCheck aria-hidden className="size-4 text-positive" /> Your session ends on our servers, not just in this browser.
        </span>
      </div>
      {error && (
        <Notice tone="error" className="mt-4">
          {error}
        </Notice>
      )}
    </Section>
  );
}
