import Link from "next/link";
import { cn } from "@/lib/utils";

/** The DEYPAY mark: a rounded tile holding a "D" whose bowl carries a single point of value. */
export function LogoMark({ className, inverse = false }: { className?: string; inverse?: boolean }) {
  return (
    <svg viewBox="0 0 32 32" aria-hidden className={cn("size-8 shrink-0", className)}>
      <rect width="32" height="32" rx="9.5" className={inverse ? "fill-white" : "fill-brand-500"} />
      <path
        d="M10.5 8.5h4.25a7.5 7.5 0 0 1 0 15H10.5z"
        fill="none"
        strokeWidth="3"
        strokeLinejoin="round"
        className={inverse ? "stroke-brand-600" : "stroke-white"}
      />
      <circle cx="15" cy="16" r="2.1" className={inverse ? "fill-brand-600" : "fill-white"} />
    </svg>
  );
}

export function Wordmark({ className }: { className?: string }) {
  return (
    <span className={cn("font-display text-[1.15rem] font-semibold tracking-[-0.03em]", className)}>
      DEYPAY
    </span>
  );
}

export function Logo({
  href = "/",
  className,
  inverse = false,
}: {
  href?: string;
  className?: string;
  inverse?: boolean;
}) {
  return (
    <Link
      href={href}
      aria-label="DEYPAY home"
      className={cn(
        "inline-flex items-center gap-2.5 rounded-lg",
        inverse ? "text-white" : "text-ink-900",
        className,
      )}
    >
      <LogoMark inverse={false} />
      <Wordmark />
    </Link>
  );
}
