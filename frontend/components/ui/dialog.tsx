"use client";

import { AlertDialog as AlertDialogPrimitive } from "radix-ui";
import * as React from "react";
import { cn } from "@/lib/utils";

export const ConfirmDialog = AlertDialogPrimitive.Root;
export const ConfirmTrigger = AlertDialogPrimitive.Trigger;
export const ConfirmCancel = AlertDialogPrimitive.Cancel;
export const ConfirmAction = AlertDialogPrimitive.Action;

/** Confirmation for consequential actions such as signing out other devices. */
export function ConfirmContent({
  title,
  description,
  children,
  className,
}: {
  title: string;
  description: React.ReactNode;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <AlertDialogPrimitive.Portal>
      <AlertDialogPrimitive.Overlay className="fixed inset-0 z-50 bg-ink-950/45 backdrop-blur-[2px] data-[state=open]:animate-fade" />
      <AlertDialogPrimitive.Content
        className={cn(
          "fixed inset-x-3 bottom-3 z-50 rounded-feature bg-surface p-6 shadow-pop data-[state=open]:animate-rise sm:inset-x-auto sm:bottom-auto sm:left-1/2 sm:top-1/2 sm:w-full sm:max-w-md sm:-translate-x-1/2 sm:-translate-y-1/2 sm:p-7",
          className,
        )}
      >
        <AlertDialogPrimitive.Title className="font-display text-xl font-semibold text-ink-900">
          {title}
        </AlertDialogPrimitive.Title>
        <AlertDialogPrimitive.Description className="mt-2 text-[0.9375rem] leading-relaxed text-ink-600">
          {description}
        </AlertDialogPrimitive.Description>
        <div className="mt-6 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">{children}</div>
      </AlertDialogPrimitive.Content>
    </AlertDialogPrimitive.Portal>
  );
}
