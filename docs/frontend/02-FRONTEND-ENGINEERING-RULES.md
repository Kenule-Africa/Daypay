# DEYPAY — Frontend Engineering Rules for Claude

**Status:** Owner-directed implementation foundation  
**Scope:** Existing `frontend/` Next.js application; real product implementation

## 1. Source of truth and timing

- Reuse the existing **Next.js / React / TypeScript** application in `frontend/`. Do not scaffold a second app or replace the framework without owner approval.
- Read `AGENTS.md`, `docs/01-FRS.md`, `docs/04-API-SPECIFICATION.md`, `docs/08-IMPLEMENTATION-PLAN.md`, and the current engineering handoff before making implementation changes.
- **Frontend implementation begins only after the required authentication/account backend is merged, tested, and engineering-accepted.** Planning and documentation may happen earlier.
- Preserve the owner-approved sequence: account frontend first, then real financial infrastructure and features in dependency order. KYC verification remains on hold; MFA and phone OTP remain deferred.

## 2. Component and styling foundation

- Use **`lucide-react`** for application icons (no emoji or fake icon substitutes).
- Prefer **shadcn/ui** primitives for accessible UI building blocks, customized to the DEYPAY design system and no-border rule.
- Establish theme tokens and reusable components before duplicating styles across screens.
- Keep a single consistent component system; do not add several UI frameworks for the same purpose.
- Choose the stable, compatible version of libraries for the repository's installed Next.js/React versions; do not upgrade the framework simply to obtain a component library.
- Avoid hardcoded one-off hex colors, spacing, icon sizes, and shadows throughout page components.

## 3. Product structure

Plan and implement distinct layouts for:

1. **Public:** `/` — professional DEYPAY landing page.
2. **Authentication:** registration, login, email confirmation, forgot password, password reset.
3. **Protected app:** `/app` — authenticated account overview, profile, preferences, and session/security management.

Use Next.js route groups/layouts where beneficial. Prefer small feature modules and reusable UI components; avoid a sprawling single-page component. Add additional feature routes only when backed by approved product scope.

## 4. Real backend integration only

- Use the actual DEYPAY API contracts; inspect implemented backend routes before wiring a screen.
- Registration, email verification, login, logout, password recovery, sessions, profile and image management must connect to **real** backend APIs.
- Do not ship fake product endpoints, simulated successful authentication, hardcoded demo profiles, synthetic balances, or placeholder completed transactions.
- Clearly distinguish unavailable or not-yet-enabled capabilities from functioning ones.
- The frontend must not calculate authoritative balances or execute financial activity outside DEYPAY's controlled backend APIs.
- Never put privileged provider keys or server secrets in client bundles.

## 5. Authentication and protected routes

- Honor the existing Rust authentication architecture and its secure, HttpOnly session cookie; do not invent JWT/localStorage authentication or a competing session system.
- Plan the same-origin HTTPS routing/proxy contract so browser login, refresh, profile requests, and logout preserve secure-cookie, Origin, CSRF, and deployment requirements.
- Enforce protection **server-side** for `/app` and every protected route, not just through a client-side redirect.
- Authenticated customers who visit login/register should be redirected to their app when appropriate.
- After successful login use history replacement where appropriate; after logout revoke the session server-side and redirect to login.
- Browser Back **cannot be disabled reliably**: instead, invalid or revoked sessions must never reveal protected data through navigation, caching, or restored history.
- On session expiry or revocation, clear client-side private state, stop protected requests, and lead the user back to authentication.
- Account creation does not bypass email confirmation. **KYC is not required for normal non-financial account access.**
- Use no-store or appropriately private cache behavior for customer-sensitive content. Avoid caching private data in shared caches.

## 6. UX and accessibility engineering

- Design mobile and desktop layouts deliberately; test them separately.
- Build working form validation from API contracts, including server error and rate-limit responses.
- Preserve accessibility: keyboard navigation, semantic inputs, labels, visible focus, error announcements, logical tab order, and reduced-motion support.
- Provide appropriate loading, retry, empty, disabled, and failure states. Do not make blocked features look actionable.
- Prefer standard responsive CSS and stable components over difficult-to-maintain visual effects.

## 7. Implementation quality and collaboration

- Claude owns the frontend implementation within its approved task/branch. Do not modify backend functionality or engineering tracking files unless separately authorized.
- Keep implementation reviewable: modular code, meaningful naming, minimal dependencies, clear component boundaries, and tests for actual behavior.
- Run applicable `npm` install/lockfile validation, lint, typecheck, build, and tests. Add tests for authentication redirects, protected routes, account flows, responsive behavior where practical, and accessibility-critical interactions.
- Test against the actual integrated backend in an authorized local/test environment. Test fixtures may exercise real code; mock business behavior must never be presented as production functionality.
- Use approved assets and licensed imagery. Do not copy competitors' visual designs, proprietary assets, or unlicensed fonts.
- Commit to the assigned feature branch; open a PR; do not self-merge or deploy.

## 8. Definition of done for the first account frontend

- Public landing page exists in polished, responsive desktop and mobile layouts.
- Registration, verification, login, recovery, and session/logout flows use real DEYPAY APIs.
- Logged-in user sees **their actual profile** and can update supported fields, preferences, and profile image.
- Protected `/app` routes are server-guarded; expired/revoked/logout sessions cannot regain access through Back or direct URLs.
- No decorative borders on UI surfaces; real Lucide icons and consistently styled shadcn/ui primitives are used.
- Automated quality checks pass, and manual mobile/desktop acceptance covers relevant widths and states.
- No fake money, fabricated claims, false activation of deferred features, or unnecessary rewrites.
