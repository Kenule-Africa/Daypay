import { LoaderCircle } from "lucide-react";
import { cn } from "@/lib/utils";

export function Spinner({ className, label }: { className?: string; label?: string }) {
  return (
    <span role={label ? "status" : undefined} className={cn("inline-flex", className)}>
      <LoaderCircle aria-hidden className="size-[1.1em] animate-spin" />
      {label && <span className="sr-only">{label}</span>}
    </span>
  );
}
