import { NextResponse, type NextRequest } from "next/server";

const SESSION_COOKIE = "__Host-deypay_session";

function contentSecurityPolicy(nonce: string): string {
  const development = process.env.NODE_ENV !== "production";
  return [
    "default-src 'self'",
    `script-src 'self' 'nonce-${nonce}' 'strict-dynamic'${development ? " 'unsafe-eval'" : ""}`,
    "style-src 'self' 'unsafe-inline'",
    "img-src 'self' blob: data:",
    "font-src 'self'",
    "connect-src 'self'",
    "object-src 'none'",
    "base-uri 'none'",
    "form-action 'self'",
    "frame-ancestors 'none'",
  ].join("; ");
}

function isProtected(pathname: string): boolean {
  return pathname === "/app" || pathname.startsWith("/app/");
}

const HOST = /^(?:[A-Za-z0-9-]+(?:\.[A-Za-z0-9-]+)*|\[[0-9A-Fa-f:.]+\])(?::\d{1,5})?$/;

/**
 * The origin the browser addressed. Behind the HTTPS ingress, the development
 * server reports its own listener (`localhost:3000`) in `request.nextUrl`, so a
 * redirect built from it would leave the application origin. Next.js leaves
 * absolute redirects to any other host unchanged. The redirect is therefore built
 * from the Host header and forwarded scheme that the proxy preserves. A malformed
 * Host falls back to `nextUrl`. Redirects are `no-store`, so a forged Host only
 * affects the sender's own response.
 */
function browserOrigin(request: NextRequest): string {
  const host = request.headers.get("host") ?? "";
  if (!HOST.test(host)) return request.nextUrl.origin;
  const forwarded = request.headers.get("x-forwarded-proto")?.split(",")[0]?.trim().toLowerCase();
  const scheme = forwarded === "https" || forwarded === "http" ? forwarded : request.nextUrl.protocol.slice(0, -1);
  return `${scheme}://${host}`;
}

/**
 * Applies a per-request nonce CSP and turns away requests for the account area
 * that carry no session cookie. Cookie validity is decided by the API, which the
 * account layout calls on every request; this gate only avoids a round trip.
 */
export function middleware(request: NextRequest) {
  const nonce = btoa(crypto.randomUUID());
  const policy = contentSecurityPolicy(nonce);
  const { pathname } = request.nextUrl;

  if (isProtected(pathname) && !request.cookies.has(SESSION_COOKIE)) {
    const url = new URL("/login", browserOrigin(request));
    url.searchParams.set("next", pathname);
    const response = NextResponse.redirect(url);
    response.headers.set("Cache-Control", "no-store");
    return response;
  }

  const headers = new Headers(request.headers);
  headers.set("x-nonce", nonce);
  headers.set("Content-Security-Policy", policy);
  const response = NextResponse.next({ request: { headers } });
  response.headers.set("Content-Security-Policy", policy);
  if (isProtected(pathname)) {
    response.headers.set("Cache-Control", "private, no-store");
  }
  return response;
}

export const config = {
  matcher: ["/((?!_next/static|_next/image|favicon.ico|icon.svg|api/).*)"],
};
