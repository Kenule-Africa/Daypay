import { CircleAlert, CircleCheck, Info, TriangleAlert } from "lucide-react";
import * as React from "react";
import { cn } from "@/lib/utils";

const tones = {
  info: { icon: Info, className: "bg-brand-50 text-brand-800" },
  success: { icon: CircleCheck, className: "bg-positive-soft text-positive" },
  warning: { icon: TriangleAlert, className: "bg-caution-soft text-caution" },
  error: { icon: CircleAlert, className: "bg-danger-soft text-danger" },
};

type NoticeProps = {
  tone?: keyof typeof tones;
  title?: string;
  children?: React.ReactNode;
  className?: string;
  live?: boolean;
};

/** Inline message. Errors are announced assertively, others politely. */
export function Notice({ tone = "info", title, children, className, live = true }: NoticeProps) {
  const { icon: Icon, className: toneClass } = tones[tone];
  return (
    <div
      role={live ? (tone === "error" ? "alert" : "status") : undefined}
      className={cn("flex animate-rise gap-3 rounded-[0.9rem] px-4 py-3.5 text-sm leading-relaxed", toneClass, className)}
    >
      <Icon aria-hidden className="mt-0.5 size-[1.1rem] shrink-0" />
      <div className="min-w-0">
        {title && <p className="font-semibold">{title}</p>}
        {children && <div className={cn(title && "mt-0.5", "text-current/90")}>{children}</div>}
      </div>
    </div>
  );
}
