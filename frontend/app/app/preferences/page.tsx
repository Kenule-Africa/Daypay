import type { Metadata } from "next";
import { PreferencesForm } from "@/components/account/preferences-form";
import { loadPreferences, requireCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Preferences" };

export default async function PreferencesPage() {
  await requireCustomer("/app/preferences");
  return <PreferencesForm initial={await loadPreferences()} />;
}
