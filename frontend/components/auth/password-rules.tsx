import { Check, Circle } from "lucide-react";
import { cn } from "@/lib/utils";
import { passwordRules } from "@/lib/validation";

/** Live checklist for the password policy enforced by registration and reset. */
export function PasswordRules({ password, id }: { password: string; id?: string }) {
  return (
    <ul id={id} className="grid grid-cols-2 gap-x-4 gap-y-1.5 text-[0.8125rem]">
      {passwordRules(password).map(rule => (
        <li
          key={rule.id}
          className={cn("flex items-center gap-2 transition-colors duration-300", rule.met ? "text-positive" : "text-ink-500")}
        >
          {rule.met ? (
            <Check aria-hidden strokeWidth={3} className="size-3.5 animate-fade" />
          ) : (
            <Circle aria-hidden className="size-3 text-ink-300" />
          )}
          <span>
            {rule.label}
            <span className="sr-only">{rule.met ? " (met)" : " (not yet met)"}</span>
          </span>
        </li>
      ))}
    </ul>
  );
}
