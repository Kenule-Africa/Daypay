import type { Metadata } from "next";
import { ProfileDetails } from "@/components/account/profile-details";
import { requireCustomer } from "@/lib/server/session";

export const metadata: Metadata = { title: "Profile" };

export default async function ProfilePage() {
  await requireCustomer("/app/profile");
  return <ProfileDetails />;
}
