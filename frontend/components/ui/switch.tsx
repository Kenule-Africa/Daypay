"use client";

import { Switch as SwitchPrimitive } from "radix-ui";
import * as React from "react";
import { cn } from "@/lib/utils";

export function Switch({ className, ...props }: React.ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        "peer relative inline-flex h-7 w-12 shrink-0 cursor-pointer items-center rounded-full bg-ink-200 p-0.5 transition-colors duration-300 ease-out-expo data-[state=checked]:bg-brand-500 disabled:cursor-not-allowed disabled:opacity-50",
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb className="pointer-events-none block size-6 rounded-full bg-white shadow-[0_1px_3px_rgb(18_16_25/0.25)] transition-transform duration-300 ease-out-expo data-[state=checked]:translate-x-5" />
    </SwitchPrimitive.Root>
  );
}
