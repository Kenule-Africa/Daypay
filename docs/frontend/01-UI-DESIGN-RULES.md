# DEYPAY — Frontend UI Design Rules

**Status:** Owner-directed design foundation  
**Applies to:** Landing page, authentication, account app, and all future DEYPAY frontend screens  
**Owner of frontend implementation:** Claude

## 1. Product design direction

- Build the **real DEYPAY product**, not a mockup, demo, or throwaway theme.
- The brand should feel premium, precise, calm, modern, and recognizably DEYPAY.
- Use the **approved purple / near-black / off-white direction** from the visual review. **Do not use green as the primary brand colour.** Final token values may be refined consistently during design approval.
- Make DEYPAY recognizable through typography, composition, color, consistent navigation, and a restrained logo/wordmark treatment — not decorative clutter.
- Do not invent trust numbers, customer counts, regulatory claims, partnerships, testimonials, balances, or transaction activity.

## 2. Strict no-border visual rule

**Default: no visible decorative borders.**

- No outlines around cards, content panels, page sections, navigation shells, or dashboards solely for decoration.
- Avoid excessive dividers, outlined boxes, nested framed cards, and generic dashboard grids.
- Establish visual hierarchy through whitespace, typography, layout, tonal surface contrast, and restrained shadows only where needed.
- Shadcn/ui components must be restyled to remove default decorative borders and ring-like container treatments.
- **Accessibility and clarity exceptions:** visible keyboard focus indicators, input errors, high-contrast controls, and legally/financially important separation are required when needed. The no-border rule must not compromise accessibility or usability.

## 3. Responsive design: two intentionally designed experiences

- **Mobile-first:** begin with small-screen layout, touch targets, form ergonomics, safe areas, and realistic phone widths.
- **Desktop remains desktop:** use desktop-appropriate navigation, columns, whitespace, density, and interaction patterns. Never stretch or simply enlarge the mobile view.
- Shared components and brand tokens must be consistent across both layouts, while their **composition may differ substantially**.
- Account navigation: compact, thumb-friendly navigation on mobile; stable sidebar or similarly deliberate desktop navigation once approved.
- No horizontal overflow at supported viewport widths. Test narrow mobile, normal mobile, tablet, laptop, and large desktop.

## 4. Design tokens and visual consistency

- Define colors, typography, spacing, radii, shadows, layout widths, and motion through shared CSS/theme tokens. No scattered arbitrary values.
- Palette intent: **violet / electric purple accent**, near-black or deep ink, off-white, and neutral grays; avoid constant gradients. A gradient is an accent, not the entire interface.
- Prefer excellent typography, purposeful spacing, deliberate emphasis, and clean alignment over illustrations and effects.
- Use radius consistently; borderless does **not** mean every surface must be a pill.
- Motion must be subtle, functional, and respect reduced-motion settings.
- Use responsive content constraints rather than making long forms span the full desktop width.
- Avoid fake device mockups or conceptual UI graphics inside functional product pages.

## 5. Real icons and real components

- Use **`lucide-react`** as the default icon library. Import actual icons as components; use consistent size, stroke weight, and alignment.
- **Never use emoji, Unicode symbols, text glyphs, or CSS-drawn imitation icons as application icons.**
- Use **shadcn/ui** as the preferred accessible component foundation for buttons, forms, dialogs, menus, tabs, sheets, popovers, and cards/panels where appropriate. Use current compatible stable packages; check project compatibility before installing.
- shadcn/ui is a source-code component foundation, not a fixed visual theme: tailor primitives to DEYPAY's no-border aesthetic.
- Use a component only where it helps usability. Do not fill screens with cards just because a card component exists.
- Avoid mixing multiple unrelated icon sets or overlapping UI libraries without a justified need.

## 6. Screen design rules

### Landing page
- Professional fintech presentation that explains DEYPAY **at a glance**: what it does, who it is for, and how to start.
- Clear hero, concise copy, meaningful calls to action, purposeful feature explanation, and credible security language.
- Discuss USDC/USDT wallets, transfers, conversions, and Nigerian payments accurately as product capabilities/roadmap; never imply unfinished functionality is already live.
- Avoid oversized marketing copy, unnecessary decorative widgets, imaginary transaction previews, and unsupported performance/security claims.

### Authentication
- Design registration, email verification, login, forgot/reset password as one coherent experience.
- Keep forms focused, readable, accessible, and practical for mobile keyboards.
- Desktop may include a sophisticated supporting brand panel; mobile prioritizes the actual form.
- Show field-level validation, password reveal controls, loading, failure, retry, and success/next-step states.
- Email verification is required before eligible login. Do not make registration alone appear to create an active session.

### Authenticated account application
- Show the **real signed-in customer's details** from backend APIs: name, username, image, email/verification state, account status, preferences, and active sessions.
- Keep profile and security functions easy to discover on mobile and desktop.
- No fictional wallet totals, balances, deposit addresses, transactions, or enabled financial operations.
- Future wallet/payment UI must be gated on real backend capabilities, actual eligibility checks, and approved release conditions.

## 7. Required UI states and quality

Every real flow needs loading, empty, success, validation-error, unavailable, and unauthorized/session-expired states as relevant. Maintain consistent color contrast, keyboard accessibility, screen-reader labels, visible focus, sensible tap targets, and predictable navigation.

**Design acceptance:** DEYPAY should be beautiful because its structure is considered and its information is clear — not because it contains extra visual effects.
