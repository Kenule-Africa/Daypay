import { cva, type VariantProps } from "class-variance-authority";
import { Slot } from "radix-ui";
import * as React from "react";
import { cn } from "@/lib/utils";
import { Spinner } from "./spinner";

export const buttonVariants = cva(
  "relative inline-flex select-none items-center justify-center gap-2 whitespace-nowrap font-medium transition-[background-color,color,box-shadow,transform] duration-200 ease-out-expo active:scale-[0.98] disabled:pointer-events-none disabled:opacity-55 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        primary:
          "bg-brand-500 text-white shadow-glow hover:bg-brand-600 hover:shadow-[0_14px_34px_-12px_rgb(108_60_255/0.7)]",
        ink: "bg-ink-900 text-paper hover:bg-ink-800",
        soft: "bg-brand-50 text-brand-700 hover:bg-brand-100",
        quiet: "bg-paper-2 text-ink-800 hover:bg-paper-3",
        ghost: "text-ink-700 hover:bg-ink-900/5 hover:text-ink-900",
        inverse: "bg-paper text-ink-900 hover:bg-white",
        danger: "bg-danger-soft text-danger hover:bg-danger hover:text-white",
        link: "h-auto rounded-none p-0 text-brand-600 underline-offset-4 hover:underline active:scale-100",
      },
      size: {
        sm: "h-9 rounded-control px-3.5 text-sm [&_svg]:size-4",
        md: "h-11 rounded-control px-5 text-[0.9375rem] [&_svg]:size-[1.125rem]",
        lg: "h-13 rounded-[0.9rem] px-6 text-base [&_svg]:size-5",
        icon: "size-10 rounded-control [&_svg]:size-5",
      },
    },
    compoundVariants: [{ variant: "link", class: "h-auto px-0" }],
    defaultVariants: { variant: "primary", size: "md" },
  },
);

type ButtonProps = React.ComponentProps<"button"> &
  VariantProps<typeof buttonVariants> & { asChild?: boolean; loading?: boolean };

export function Button({
  className,
  variant,
  size,
  asChild = false,
  loading = false,
  disabled,
  children,
  ...props
}: ButtonProps) {
  const Component = asChild ? Slot.Root : "button";
  return (
    <Component
      data-slot="button"
      className={cn(buttonVariants({ variant, size }), className)}
      disabled={asChild ? undefined : disabled || loading}
      aria-busy={loading || undefined}
      {...props}
    >
      {asChild ? (
        children
      ) : (
        <>
          {loading && <Spinner className="absolute" />}
          <span className={cn("inline-flex items-center gap-2", loading && "invisible")}>{children}</span>
        </>
      )}
    </Component>
  );
}
