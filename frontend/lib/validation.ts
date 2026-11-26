// Client-side checks mirroring the backend rules, so customers see field-level
// feedback before submitting. The backend remains authoritative: registration
// answers every invalid value with a single generic 422.
// Sources: deypay-application registration.rs (valid_email, validate),
// usernames.rs (handle), passwords.rs (PasswordPolicy::PROVISIONAL),
// profile.rs (profile_name, MAX_NAME_CHARS).

export const USERNAME_MIN = 3;
export const USERNAME_MAX = 30;
export const NAME_MAX = 100;
export const IMAGE_MAX_BYTES = 5 * 1024 * 1024;
export const IMAGE_TYPES = ["image/jpeg", "image/png", "image/webp"];

const EMAIL_LOCAL = /^[\p{L}\p{N}.!#$%&'*+\-/=?^_`{|}~]+$/u;
const EMAIL_LABEL = /^[\p{L}\p{N}-]+$/u;

export function validEmail(input: string): boolean {
  const email = input.trim();
  const at = email.indexOf("@");
  if (at < 0) return false;
  const local = email.slice(0, at);
  const domain = email.slice(at + 1);
  return (
    email.length <= 254 &&
    local.length > 0 &&
    local.length <= 64 &&
    !local.startsWith(".") &&
    !local.endsWith(".") &&
    !local.includes("..") &&
    EMAIL_LOCAL.test(local) &&
    domain.split(".").every(
      label =>
        label.length > 0 &&
        label.length <= 63 &&
        !label.startsWith("-") &&
        !label.endsWith("-") &&
        EMAIL_LABEL.test(label),
    )
  );
}

/** Removes spacing and punctuation people commonly type in phone numbers. */
export function normalizePhone(input: string): string {
  return input.replace(/[\s().-]/g, "");
}

/** E.164: a plus sign, then 2–15 digits, not starting with zero. */
export function validPhone(input: string): boolean {
  return /^\+[1-9]\d{1,14}$/.test(normalizePhone(input));
}

/** Returns the handle without its optional `@` when it has the approved format. */
export function usernameHandle(input: string): string | null {
  const handle = input.startsWith("@") ? input.slice(1) : input;
  const valid =
    /^[A-Za-z0-9_.]+$/.test(handle) &&
    handle.length >= USERNAME_MIN &&
    handle.length <= USERNAME_MAX &&
    !handle.startsWith(".") &&
    !handle.endsWith(".") &&
    !handle.includes("..");
  return valid ? handle : null;
}

export type PasswordRule = { id: string; label: string; met: boolean };

/** The provisional password policy currently enforced by registration and reset. */
export function passwordRules(password: string): PasswordRule[] {
  return [
    { id: "length", label: "At least 6 characters", met: [...password].length >= 6 },
    { id: "upper", label: "An uppercase letter", met: /[A-Z]/.test(password) },
    { id: "digit", label: "A number", met: /[0-9]/.test(password) },
    { id: "symbol", label: "A symbol", met: /[\p{P}\p{S}]/u.test(password) },
  ];
}

export function passwordAccepted(password: string): boolean {
  return passwordRules(password).every(rule => rule.met);
}

/** Trimmed, 1–100 characters, ordinary spaces only, no control or invisible characters. */
export function validName(input: string): boolean {
  const name = input.trim();
  return (
    name.length > 0 &&
    [...name].length <= NAME_MAX &&
    ![...name].some(c => c !== " " && /[\p{White_Space}\p{Cc}\p{Cf}\p{Zl}\p{Zp}]/u.test(c))
  );
}

/** Only internal account paths are accepted as post-sign-in destinations. */
export function safeNextPath(input: string | null | undefined): string {
  if (!input) return "/app";
  return /^\/app(\/[a-z-]+)?$/.test(input) ? input : "/app";
}
