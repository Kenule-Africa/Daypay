// Server-side session checks for protected and authentication routes. The
// customer's own HttpOnly cookie is forwarded to the general API, which validates
// it against PostgreSQL through the session guard. Nothing is cached: each
// request re-validates, so a revoked or expired session never renders private data.
import { cookies } from "next/headers";
import { redirect } from "next/navigation";
import { cache } from "react";
import type { Preferences, Profile } from "../contracts";

export const SESSION_COOKIE = "__Host-deypay_session";

export function apiBase(): string {
  return process.env.API_BASE_URL ?? "http://127.0.0.1:8000";
}

export type CustomerState =
  | { state: "signed_out" }
  | { state: "expired" }
  | { state: "unavailable" }
  | { state: "ok"; profile: Profile };

/** The single session cookie, or null when absent, duplicated or malformed. */
async function sessionCookie(): Promise<string | null | "invalid"> {
  const jar = await cookies();
  const values = jar.getAll(SESSION_COOKIE);
  if (values.length === 0) return null;
  if (values.length > 1) return "invalid";
  const value = values[0].value;
  return /^[A-Za-z0-9_-]{16,512}$/.test(value) ? value : "invalid";
}

async function customerFetch(path: string, token: string): Promise<Response> {
  return fetch(new URL(path, apiBase()), {
    headers: { accept: "application/json", cookie: `${SESSION_COOKIE}=${token}` },
    cache: "no-store",
    redirect: "error",
    signal: AbortSignal.timeout(5000),
  });
}

/** One profile lookup per server request, shared by layouts and pages. */
export const loadCustomer = cache(async (): Promise<CustomerState> => {
  const token = await sessionCookie();
  if (token === null) return { state: "signed_out" };
  if (token === "invalid") return { state: "expired" };
  try {
    const response = await customerFetch("/api/v1/me", token);
    if (response.status === 401) return { state: "expired" };
    if (!response.ok) return { state: "unavailable" };
    const body = (await response.json()) as { data?: Profile };
    return body.data?.id ? { state: "ok", profile: body.data } : { state: "unavailable" };
  } catch {
    return { state: "unavailable" };
  }
});

/** Protected pages: returns the signed-in profile or leaves for sign-in. */
export async function requireCustomer(next = "/app"): Promise<Profile> {
  const customer = await loadCustomer();
  switch (customer.state) {
    case "ok":
      return customer.profile;
    case "signed_out":
      redirect(`/login?next=${encodeURIComponent(next)}`);
    case "expired":
      redirect(`/login?reason=expired&next=${encodeURIComponent(next)}`);
    default:
      throw new Error("Account service unavailable");
  }
}

/** Preferences for the signed-in customer, or null when they cannot be read now. */
export async function loadPreferences(): Promise<Preferences | null> {
  const token = await sessionCookie();
  if (!token || token === "invalid") return null;
  try {
    const response = await customerFetch("/api/v1/me/preferences", token);
    if (!response.ok) return null;
    const body = (await response.json()) as { data?: Preferences };
    return body.data ?? null;
  } catch {
    return null;
  }
}
