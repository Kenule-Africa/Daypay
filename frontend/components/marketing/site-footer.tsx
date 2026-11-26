import Link from "next/link";
import { Logo } from "@/components/brand/logo";

const columns = [
  {
    title: "Product",
    links: [
      { href: "/#product", label: "Overview" },
      { href: "/#pay-anyone", label: "Pay Anyone" },
      { href: "/#auto-settlement", label: "Auto-Settlement" },
      { href: "/#security", label: "Security" },
      { href: "/#availability", label: "Availability" },
    ],
  },
  {
    title: "Account",
    links: [
      { href: "/register", label: "Create account" },
      { href: "/login", label: "Sign in" },
      { href: "/forgot-password", label: "Reset password" },
      { href: "/verify-email", label: "Verify email" },
    ],
  },
  {
    title: "Legal",
    links: [
      { href: "/legal", label: "Terms of Service" },
      { href: "/legal", label: "Privacy Notice" },
      { href: "/legal", label: "Consent and acknowledgements" },
      { href: "/status", label: "Service status" },
    ],
  },
];

export function SiteFooter() {
  return (
    <footer className="px-4 pb-10 pt-16 sm:px-6 lg:pt-20">
      <div className="mx-auto max-w-site">
        <div className="grid gap-12 lg:grid-cols-[1.2fr_2fr]">
          <div className="max-w-sm">
            <Logo />
            <p className="mt-5 leading-relaxed text-ink-500">
              A stablecoin-first account for holding, sending and spending digital dollars, built for Nigeria.
            </p>
          </div>
          <nav aria-label="Footer" className="grid grid-cols-2 gap-10 sm:grid-cols-3">
            {columns.map(column => (
              <div key={column.title}>
                <p className="text-sm font-semibold text-ink-900">{column.title}</p>
                <ul className="mt-4 grid gap-3">
                  {column.links.map(link => (
                    <li key={link.label}>
                      <Link href={link.href} className="text-[0.9375rem] text-ink-500 transition-colors hover:text-ink-900">
                        {link.label}
                      </Link>
                    </li>
                  ))}
                </ul>
              </div>
            ))}
          </nav>
        </div>
        <div className="mt-16 flex flex-col gap-4 pt-8 text-sm text-ink-400 shadow-[0_-1px_0_rgb(18_16_25/0.07)] lg:flex-row lg:justify-between">
          <p>© {new Date().getFullYear()} Kenule Africa. DEYPAY is a product of Kenule Africa.</p>
          <p className="max-w-2xl lg:text-right">
            DEYPAY financial services are not yet available. Wallets, transfers, conversion and bank payments will
            open only after identity verification and the required approvals.
          </p>
        </div>
      </div>
    </footer>
  );
}
