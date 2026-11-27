// Browser client for the DEYPAY APIs. Every request goes to the same origin,
// where the HTTPS ingress routes credential and session-control paths to the
// isolated verifier and all other API paths to the general API. The session
// lives only in the HttpOnly `__Host-deypay_session` cookie; nothing here reads
// or stores it. Browsers attach the Origin header that the backend checks on
// state-changing requests.
import type { ApiErrorBody } from "./contracts";

export type ApiFailure = {
  ok: false;
  status: number;
  code: string;
  reason?: string;
  fields?: string[];
};
export type ApiSuccess<T> = { ok: true; status: number; data: T };
export type ApiResult<T> = ApiSuccess<T> | ApiFailure;

/**
 * - `required`: a protected customer request. A 401 refreshes the session once
 *   and retries; if that fails the customer is sent to sign in.
 * - `optional`: sends and receives the session cookie without treating 401 as
 *   expiry (sign-in, sign-out).
 * - `omit`: public requests that must not carry the session (link tokens,
 *   registration, availability).
 */
export type SessionMode = "required" | "optional" | "omit";

type RequestOptions = {
  method?: "GET" | "POST" | "PATCH" | "PUT" | "DELETE";
  json?: unknown;
  body?: Blob;
  contentType?: string;
  session?: SessionMode;
  signal?: AbortSignal;
  /** Extra request headers, such as Idempotency-Key. */
  headers?: Record<string, string>;
};

let refreshing: Promise<boolean> | null = null;

/** Rotates the session cookie. Concurrent callers share one attempt. */
export function refreshSession(): Promise<boolean> {
  if (!refreshing) {
    refreshing = fetch("/api/v1/auth/refresh", {
      method: "POST",
      credentials: "same-origin",
      cache: "no-store",
    })
      .then(response => response.ok)
      .catch(() => false)
      .finally(() => {
        refreshing = null;
      });
  }
  return refreshing;
}

let leaving = false;

/** Leaves the account area with a full navigation so no private state survives. */
export function endSession(reason: "expired" | "signed_out") {
  if (leaving || typeof window === "undefined") return;
  leaving = true;
  const target =
    reason === "expired"
      ? `/login?reason=expired&next=${encodeURIComponent(window.location.pathname)}`
      : "/login?reason=signed_out";
  window.location.replace(target);
}

export async function request<T = Record<string, never>>(
  path: string,
  options: RequestOptions = {},
): Promise<ApiResult<T>> {
  const session = options.session ?? "required";
  const headers: Record<string, string> = { ...options.headers, Accept: "application/json" };
  if (options.json !== undefined) headers["Content-Type"] = "application/json";
  else if (options.contentType) headers["Content-Type"] = options.contentType;
  const send = () =>
    fetch(path, {
      method: options.method ?? "GET",
      credentials: session === "omit" ? "omit" : "same-origin",
      cache: "no-store",
      redirect: "error",
      referrerPolicy: session === "omit" ? "no-referrer" : "same-origin",
      headers,
      body: options.json !== undefined ? JSON.stringify(options.json) : options.body,
      signal: options.signal,
    });

  let response: Response;
  try {
    response = await send();
    if (response.status === 401 && session === "required" && (await refreshSession())) {
      response = await send();
    }
  } catch (error) {
    if (error instanceof DOMException && error.name === "AbortError") throw error;
    return { ok: false, status: 0, code: "NETWORK_ERROR" };
  }

  if (response.status === 401 && session === "required") {
    endSession("expired");
  }

  const body = await readJson(response);
  if (response.ok) {
    const data = (body as { data?: T } | null)?.data;
    return { ok: true, status: response.status, data: data as T };
  }
  const error = (body as ApiErrorBody | null)?.error;
  return {
    ok: false,
    status: response.status,
    code: error?.code ?? "UNKNOWN",
    reason: error?.details?.reason,
    fields: error?.details?.fields,
  };
}

async function readJson(response: Response): Promise<unknown> {
  if (response.status === 204) return null;
  const type = response.headers.get("content-type") ?? "";
  if (!type.toLowerCase().startsWith("application/json")) return null;
  try {
    return await response.json();
  } catch {
    return null;
  }
}

/** Customer-facing wording for failures that every flow handles the same way. */
export function describeFailure(failure: ApiFailure, subject = "This service"): string {
  switch (failure.status) {
    case 0:
      return "We couldn't reach DEYPAY. Check your connection and try again.";
    case 403:
      return "We couldn't verify this request. Refresh the page and try again.";
    case 429:
      return "Too many attempts. Wait a minute, then try again.";
    case 503:
      return `${subject} is temporarily unavailable. Please try again shortly.`;
    default:
      return failure.status >= 500
        ? "Something went wrong on our side. Please try again."
        : "We couldn't complete that request. Please try again.";
  }
}
