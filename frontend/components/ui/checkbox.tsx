"use client";

import { Check } from "lucide-react";
import { Checkbox as CheckboxPrimitive } from "radix-ui";
import * as React from "react";
import { cn } from "@/lib/utils";

export function Checkbox({ className, ...props }: React.ComponentProps<typeof CheckboxPrimitive.Root>) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      className={cn(
        "peer mt-0.5 inline-flex size-5 shrink-0 items-center justify-center rounded-[0.4rem] bg-surface shadow-[inset_0_0_0_1.5px_rgb(18_16_25/0.28)] transition-[background-color,box-shadow] duration-200 hover:shadow-[inset_0_0_0_1.5px_var(--color-brand-500)] data-[state=checked]:bg-brand-500 data-[state=checked]:shadow-none aria-[invalid=true]:shadow-[inset_0_0_0_2px_var(--color-danger)] disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      {...props}
    >
      <CheckboxPrimitive.Indicator className="animate-fade text-white">
        <Check aria-hidden strokeWidth={3} className="size-3.5" />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}
