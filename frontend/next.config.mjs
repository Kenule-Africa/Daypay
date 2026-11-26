import { PHASE_DEVELOPMENT_SERVER } from "next/constants.js";

const securityHeaders = [
  { key: "X-Content-Type-Options", value: "nosniff" },
  { key: "X-Frame-Options", value: "DENY" },
  { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
  { key: "Permissions-Policy", value: "camera=(), microphone=(), geolocation=(), payment=()" },
  { key: "Cross-Origin-Opener-Policy", value: "same-origin" },
];

// Pages that receive a one-time token in the URL fragment never send a referrer
// and are never stored.
const tokenPageHeaders = [
  { key: "Referrer-Policy", value: "no-referrer" },
  { key: "Cache-Control", value: "no-store" },
];

/**
 * In production the HTTPS ingress serves this app and the APIs from one origin:
 * credential and session-control paths go to `deypay-auth`, other `/api/` paths to
 * `deypay-api` (docs/engineering/AUTHENTICATION-RELEASE.md). The development
 * server can stand in for that ingress when DEYPAY_DEV_AUTH_URL and
 * API_BASE_URL point at locally running services.
 */
function developmentRewrites() {
  const api = process.env.API_BASE_URL;
  const auth = process.env.DEYPAY_DEV_AUTH_URL;
  if (!api || !auth) return [];
  const verifier = [
    "login",
    "refresh",
    "logout",
    "forgot-password",
    "reset-password",
    "sessions",
    "sessions/:id",
  ].map(path => ({ source: `/api/v1/auth/${path}`, destination: `${auth}/api/v1/auth/${path}` }));
  return [...verifier, { source: "/api/:path*", destination: `${api}/api/:path*` }];
}

export default function config(phase) {
  return {
    poweredByHeader: false,
    reactStrictMode: true,
    async headers() {
      return [
        { source: "/:path*", headers: securityHeaders },
        { source: "/verify-email", headers: tokenPageHeaders },
        { source: "/reset-password", headers: tokenPageHeaders },
      ];
    },
    async rewrites() {
      return phase === PHASE_DEVELOPMENT_SERVER ? developmentRewrites() : [];
    },
  };
}
