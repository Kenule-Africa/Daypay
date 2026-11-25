# DEYPAY Frontend Rules — Start Here

These files establish the **long-term frontend design and engineering standards** for DEYPAY. Place them under `docs/frontend/` in the DEYPAY repository:

1. `01-UI-DESIGN-RULES.md` — Visual direction, strict no-border design, mobile-first/desktop-specific UX, real icons, shadcn/ui, and product screen principles.
2. `02-FRONTEND-ENGINEERING-RULES.md` — Frontend architecture, true API integration, session-protected routes, testing, and acceptance requirements.

**Claude instruction when frontend work is authorized:** Read both files along with `AGENTS.md` and the current FRS/API/implementation documentation before implementation. Treat these rules as persistent frontend requirements. If a rule conflicts with an approved backend contract or security requirement, preserve the backend/security contract and report the conflict.

**Current scheduling rule:** Frontend implementation must wait until the customer-account backend has completed engineering acceptance. Documentation/design preparation may continue meanwhile.
