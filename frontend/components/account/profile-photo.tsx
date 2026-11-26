"use client";

import { Camera, Trash2 } from "lucide-react";
import * as React from "react";
import { useAccount } from "@/components/account/account-context";
import { Avatar } from "@/components/account/avatar";
import { Button } from "@/components/ui/button";
import { ConfirmAction, ConfirmCancel, ConfirmContent, ConfirmDialog, ConfirmTrigger } from "@/components/ui/dialog";
import { Notice } from "@/components/ui/notice";
import { Spinner } from "@/components/ui/spinner";
import { describeFailure, request } from "@/lib/api";
import type { ProfileImage } from "@/lib/contracts";
import { IMAGE_MAX_BYTES, IMAGE_TYPES } from "@/lib/validation";

const rejection: Record<string, string> = {
  unsupported_type: "Choose a JPEG, PNG or WebP image.",
  too_large: "That image is larger than 5 MB. Choose a smaller one.",
  too_many_pixels: "That image is too large. Use one no more than 4096 pixels on each side.",
  too_complex: "We couldn't compress that image enough. Try a simpler or smaller photo.",
  undecodable: "We couldn't read that image. Try a different file.",
  incomplete_upload: "The upload didn't finish. Check your connection and try again.",
  busy: "We're processing a lot of images right now. Try again in a moment.",
};

/** PUT and DELETE /api/v1/me/profile-image. The API re-encodes every upload as a metadata-free JPEG. */
export function ProfilePhoto() {
  const { profile, setProfile } = useAccount();
  const input = React.useRef<HTMLInputElement>(null);
  const [busy, setBusy] = React.useState<"upload" | "remove" | null>(null);
  const [message, setMessage] = React.useState<{ tone: "success" | "error"; text: string } | null>(null);

  async function upload(file: File) {
    setMessage(null);
    if (!IMAGE_TYPES.includes(file.type)) {
      setMessage({ tone: "error", text: rejection.unsupported_type });
      return;
    }
    if (file.size > IMAGE_MAX_BYTES) {
      setMessage({ tone: "error", text: rejection.too_large });
      return;
    }
    setBusy("upload");
    const result = await request<{ profile_image: ProfileImage }>("/api/v1/me/profile-image", {
      method: "PUT",
      body: file,
      contentType: file.type,
    });
    setBusy(null);
    if (result.ok && result.data?.profile_image) {
      setProfile({ ...profile, profile_image: result.data.profile_image });
      setMessage({ tone: "success", text: "Your profile photo has been updated." });
    } else if (!result.ok) {
      setMessage({
        tone: "error",
        text: (result.reason && rejection[result.reason]) || describeFailure(result, "Photo upload"),
      });
    }
  }

  async function remove() {
    setMessage(null);
    setBusy("remove");
    const result = await request("/api/v1/me/profile-image", { method: "DELETE" });
    setBusy(null);
    if (result.ok) {
      setProfile({ ...profile, profile_image: null });
      setMessage({ tone: "success", text: "Your profile photo has been removed." });
    } else {
      setMessage({ tone: "error", text: describeFailure(result, "Photo removal") });
    }
  }

  return (
    <div>
      <div className="flex flex-col gap-5 sm:flex-row sm:items-center">
        <div className="relative w-fit">
          <Avatar profile={profile} className="size-24 text-3xl" />
          {busy === "upload" && (
            <span className="absolute inset-0 inline-flex animate-fade items-center justify-center rounded-full bg-ink-950/50 text-white">
              <Spinner label="Uploading photo" />
            </span>
          )}
        </div>
        <div className="flex flex-wrap gap-2">
          <input
            ref={input}
            type="file"
            accept={IMAGE_TYPES.join(",")}
            className="sr-only"
            tabIndex={-1}
            aria-hidden
            onChange={event => {
              const file = event.target.files?.[0];
              event.target.value = "";
              if (file) void upload(file);
            }}
          />
          <Button variant="soft" onClick={() => input.current?.click()} loading={busy === "upload"} disabled={busy !== null}>
            <Camera aria-hidden /> {profile.profile_image ? "Change photo" : "Upload photo"}
          </Button>
          {profile.profile_image && (
            <ConfirmDialog>
              <ConfirmTrigger asChild>
                <Button variant="ghost" loading={busy === "remove"} disabled={busy !== null}>
                  <Trash2 aria-hidden /> Remove
                </Button>
              </ConfirmTrigger>
              <ConfirmContent
                title="Remove your profile photo?"
                description="Your initials will be shown instead. You can upload a new photo at any time."
              >
                <ConfirmCancel asChild>
                  <Button variant="quiet">Keep photo</Button>
                </ConfirmCancel>
                <ConfirmAction asChild>
                  <Button variant="danger" onClick={remove}>
                    Remove photo
                  </Button>
                </ConfirmAction>
              </ConfirmContent>
            </ConfirmDialog>
          )}
        </div>
      </div>
      <p className="mt-4 text-sm text-ink-500">
        JPEG, PNG or WebP, up to 5 MB. We resize it and remove location and camera data before saving.
      </p>
      {message && (
        <Notice tone={message.tone} className="mt-4">
          {message.text}
        </Notice>
      )}
    </div>
  );
}
