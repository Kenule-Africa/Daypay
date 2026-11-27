"use client";

import { Check, Copy, Plus } from "lucide-react";
import { useRouter } from "next/navigation";
import * as React from "react";
import { Button } from "@/components/ui/button";
import { Notice } from "@/components/ui/notice";
import { Spinner } from "@/components/ui/spinner";
import { request } from "@/lib/api";

/** Copies a verified deposit address (FR-WAL-009). */
export function CopyAddress({ address }: { address: string }) {
  const [copied, setCopied] = React.useState(false);
  return (
    <Button
      type="button"
      size="sm"
      variant="quiet"
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(address);
          setCopied(true);
          window.setTimeout(() => setCopied(false), 2000);
        } catch {
          setCopied(false);
        }
      }}
    >
      {copied ? <Check aria-hidden /> : <Copy aria-hidden />}
      {copied ? "Copied" : "Copy address"}
    </Button>
  );
}

const failures: Record<string, string> = {
  KYC_REQUIRED: "Identity verification is needed before you can receive deposits.",
  ACCOUNT_RESTRICTED: "This wallet can't receive deposits right now.",
  PROVIDER_UNAVAILABLE: "Deposit addresses aren't available yet. Please try again later.",
  UNSUPPORTED_NETWORK: "This network isn't supported for this wallet.",
};

/**
 * Asks DEYPAY to prepare a deposit address. Shown only where the API reports a
 * request is possible; the address appears once the provider has assigned it
 * and DEYPAY has verified it.
 */
export function RequestAddress({ walletId, network }: { walletId: string; network: string }) {
  const router = useRouter();
  const [pending, setPending] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  return (
    <div className="grid gap-2">
      <Button
        type="button"
        size="sm"
        disabled={pending}
        onClick={async () => {
          setPending(true);
          setError(null);
          const result = await request(`/api/v1/wallets/${walletId}/deposit-addresses`, {
            method: "POST",
            json: { network },
          });
          setPending(false);
          if (result.ok) {
            router.refresh();
            return;
          }
          setError(failures[result.code] ?? "We couldn't request an address. Please try again.");
        }}
      >
        {pending ? <Spinner /> : <Plus aria-hidden />}
        Get deposit address
      </Button>
      {error && (
        <Notice tone="error" className="text-[0.8125rem]">
          {error}
        </Notice>
      )}
    </div>
  );
}
