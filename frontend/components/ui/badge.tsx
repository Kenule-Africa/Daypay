import { cva, type VariantProps } from "class-variance-authority";
import * as React from "react";
import { cn } from "@/lib/utils";

export const badgeVariants = cva(
  "inline-flex items-center gap-1.5 whitespace-nowrap rounded-full px-2.5 py-1 text-xs font-medium [&_svg]:size-3.5",
  {
    variants: {
      tone: {
        brand: "bg-brand-50 text-brand-700",
        live: "bg-positive-soft text-positive",
        soon: "bg-paper-2 text-ink-600",
        caution: "bg-caution-soft text-caution",
        danger: "bg-danger-soft text-danger",
        dark: "bg-white/8 text-ink-200",
      },
    },
    defaultVariants: { tone: "brand" },
  },
);

export function Badge({
  className,
  tone,
  ...props
}: React.ComponentProps<"span"> & VariantProps<typeof badgeVariants>) {
  return <span className={cn(badgeVariants({ tone }), className)} {...props} />;
}

/** A pulsing dot for status that is live right now. */
export function LiveDot({ className }: { className?: string }) {
  return (
    <span aria-hidden className={cn("relative inline-flex size-2", className)}>
      <span className="absolute inset-0 animate-ping rounded-full bg-current opacity-40 motion-reduce:hidden" />
      <span className="relative inline-flex size-2 rounded-full bg-current" />
    </span>
  );
}
