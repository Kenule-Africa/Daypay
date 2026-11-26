"use client";

import { Eye, EyeOff } from "lucide-react";
import * as React from "react";
import { cn } from "@/lib/utils";

export const inputClass =
  "h-12 w-full rounded-control bg-surface px-4 text-[0.9375rem] text-ink-900 shadow-[inset_0_0_0_1px_rgb(18_16_25/0.09)] outline-none transition-[box-shadow,background-color] duration-200 placeholder:text-ink-400 hover:shadow-[inset_0_0_0_1px_rgb(18_16_25/0.18)] focus-visible:shadow-[inset_0_0_0_2px_var(--color-brand-500)] focus-visible:outline-none disabled:cursor-not-allowed disabled:bg-paper-2 disabled:text-ink-500 aria-[invalid=true]:shadow-[inset_0_0_0_2px_var(--color-danger)]";

type FieldProps = {
  label: string;
  error?: string | null;
  hint?: React.ReactNode;
  status?: React.ReactNode;
  className?: string;
  children: (control: { id: string; describedBy: string | undefined; invalid: boolean }) => React.ReactNode;
};

/** Label, control, hint and error with the ARIA wiring done once. */
export function Field({ label, error, hint, status, className, children }: FieldProps) {
  const id = React.useId();
  const hintId = hint ? `${id}-hint` : undefined;
  const errorId = error ? `${id}-error` : undefined;
  const statusId = status ? `${id}-status` : undefined;
  const describedBy = [errorId, statusId, hintId].filter(Boolean).join(" ") || undefined;
  return (
    <div className={cn("flex flex-col gap-2", className)}>
      <label htmlFor={id} className="text-sm font-medium text-ink-800">
        {label}
      </label>
      {children({ id, describedBy, invalid: Boolean(error) })}
      {status && (
        <div id={statusId} aria-live="polite" className="text-sm">
          {status}
        </div>
      )}
      {error && (
        <p id={errorId} className="animate-fade text-sm font-medium text-danger">
          {error}
        </p>
      )}
      {hint && !error && (
        <div id={hintId} className="text-sm text-ink-500">
          {hint}
        </div>
      )}
    </div>
  );
}

type InputProps = React.ComponentProps<"input"> & { invalid?: boolean };

export function Input({ className, invalid, ...props }: InputProps) {
  return <input aria-invalid={invalid || undefined} className={cn(inputClass, className)} {...props} />;
}

export function PasswordInput({ className, invalid, ...props }: InputProps) {
  const [visible, setVisible] = React.useState(false);
  return (
    <div className="relative">
      <input
        type={visible ? "text" : "password"}
        aria-invalid={invalid || undefined}
        className={cn(inputClass, "pr-13", className)}
        autoCapitalize="none"
        autoCorrect="off"
        spellCheck={false}
        {...props}
      />
      <button
        type="button"
        onClick={() => setVisible(value => !value)}
        aria-label={visible ? "Hide password" : "Show password"}
        aria-pressed={visible}
        className="absolute inset-y-1 right-1 inline-flex w-11 items-center justify-center rounded-[0.6rem] text-ink-500 transition-colors hover:bg-paper-2 hover:text-ink-900"
      >
        {visible ? <EyeOff aria-hidden className="size-[1.1rem]" /> : <Eye aria-hidden className="size-[1.1rem]" />}
      </button>
    </div>
  );
}
