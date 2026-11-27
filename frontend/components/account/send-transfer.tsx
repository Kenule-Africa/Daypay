"use client";

import { ArrowLeft, CircleCheck, Search, Send } from "lucide-react";
import Link from "next/link";
import * as React from "react";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/field";
import { Notice } from "@/components/ui/notice";
import { Spinner } from "@/components/ui/spinner";
import { request, type ApiFailure } from "@/lib/api";
import type { Recipient, TransferChallenge, TransferProof, TransferResult, Wallet } from "@/lib/contracts";
import {
  MAX_NOTE,
  counterpartyName,
  newIdempotencyKey,
  normalizeHandle,
  transferFailure,
  validAmount,
} from "@/lib/transfers";
import { displayAmount } from "@/lib/wallets";

type Step =
  | { name: "compose" }
  | { name: "review"; challenge: TransferChallenge; key: string; token: string | null }
  | { name: "done"; result: TransferResult };

const failure = (result: ApiFailure) => transferFailure(result.code, result.reason, result.status);

/**
 * Pay a DEYPAY user (FRS §16.4): find the recipient by exact username, enter
 * the amount, review the terms the server bound to this session, confirm, then
 * send once. A retry after an uncertain outcome reuses the same confirmation
 * and Idempotency-Key, so the transfer can never happen twice.
 */
export function SendTransfer({ wallets }: { wallets: Wallet[] }) {
  const senders = wallets.filter(wallet => wallet.balance);
  const [step, setStep] = React.useState<Step>({ name: "compose" });
  const [username, setUsername] = React.useState("");
  const [recipient, setRecipient] = React.useState<Recipient | null>(null);
  const [asset, setAsset] = React.useState(senders[0]?.asset ?? "USDC");
  const [amount, setAmount] = React.useState("");
  const [note, setNote] = React.useState("");
  const [pending, setPending] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [fieldError, setFieldError] = React.useState<{ username?: string; amount?: string; note?: string }>({});
  const wallet = senders.find(w => w.asset === asset);

  async function find() {
    const handle = normalizeHandle(username);
    setRecipient(null);
    setError(null);
    if (!handle) {
      setFieldError({ username: "Usernames are 3–30 letters, numbers, dots or underscores." });
      return;
    }
    setFieldError({});
    setPending(true);
    const result = await request<Recipient>(`/api/v1/recipients/resolve?username=${encodeURIComponent(handle)}`);
    setPending(false);
    if (!result.ok) {
      setFieldError({ username: failure(result) });
      return;
    }
    if (!result.data.can_receive) {
      setFieldError({ username: transferFailure("RECIPIENT_UNAVAILABLE") });
      return;
    }
    setRecipient(result.data);
  }

  async function review(event: React.FormEvent) {
    event.preventDefault();
    setError(null);
    const problems: typeof fieldError = {};
    if (!validAmount(amount)) problems.amount = transferFailure("INVALID_AMOUNT");
    if ([...note.trim()].length > MAX_NOTE) problems.note = `Keep the note to ${MAX_NOTE} characters.`;
    setFieldError(problems);
    if (!recipient || Object.keys(problems).length > 0) return;
    setPending(true);
    const result = await request<TransferChallenge>("/api/v1/transaction-authorizations", {
      method: "POST",
      json: {
        operation_type: "internal_transfer",
        recipient_id: recipient.recipient_id,
        asset,
        amount: amount.trim(),
        description: note.trim() || null,
      },
    });
    setPending(false);
    if (!result.ok) {
      setError(failure(result));
      return;
    }
    setStep({ name: "review", challenge: result.data, key: newIdempotencyKey(), token: null });
  }

  async function send() {
    if (step.name !== "review" || !recipient) return;
    setError(null);
    setPending(true);
    let token = step.token;
    if (!token) {
      const confirmed = await request<TransferProof>(
        `/api/v1/transaction-authorizations/${step.challenge.challenge_id}/confirm`,
        { method: "POST", json: { confirm: true } },
      );
      if (!confirmed.ok) {
        setPending(false);
        setError(failure(confirmed));
        return;
      }
      token = confirmed.data.authorization_token;
      setStep({ ...step, token });
    }
    const result = await request<TransferResult>("/api/v1/transfers/internal", {
      method: "POST",
      headers: { "Idempotency-Key": step.key },
      json: {
        recipient_id: recipient.recipient_id,
        authorization_token: token,
        asset,
        amount: amount.trim(),
        description: note.trim() || null,
      },
    });
    setPending(false);
    if (result.ok) {
      setStep({ name: "done", result: result.data });
      return;
    }
    if (result.code === "AUTHORIZATION_INVALID" || result.code === "IDEMPOTENCY_CONFLICT") {
      setStep({ name: "compose" });
    }
    setError(
      result.status === 0 || result.status >= 500
        ? "We couldn't confirm the result. Try again: the same transfer is never sent twice."
        : failure(result),
    );
  }

  if (step.name === "done") {
    const { result } = step;
    return (
      <section className="rounded-feature bg-surface p-6 shadow-soft sm:p-8">
        <span className="inline-flex size-12 items-center justify-center rounded-feature bg-positive-soft text-positive">
          <CircleCheck aria-hidden className="size-6" />
        </span>
        <h2 className="mt-5 font-display text-xl font-semibold text-ink-950">
          {displayAmount(result.amount)} {result.asset} sent to {counterpartyName(result.recipient)}
        </h2>
        <p className="mt-2 text-[0.9375rem] text-ink-600">
          Reference <span className="font-mono text-ink-900">{result.reference}</span>. The recipient sees the same
          reference.
        </p>
        <div className="mt-6 flex flex-wrap gap-3">
          <Link href={`/app/payments/${result.transaction_id}`} className="text-sm font-medium text-brand-600 hover:underline">
            View payment
          </Link>
          <Link href="/app/payments" className="text-sm font-medium text-ink-600 hover:underline">
            All payments
          </Link>
        </div>
      </section>
    );
  }

  if (step.name === "review") {
    const { terms } = step.challenge;
    return (
      <section className="rounded-feature bg-surface p-6 shadow-soft sm:p-8">
        <h2 className="font-display text-xl font-semibold text-ink-950">Check and confirm</h2>
        <dl className="mt-5 grid gap-3 rounded-panel bg-paper p-4 text-sm sm:grid-cols-2">
          <div>
            <dt className="text-ink-500">To</dt>
            <dd className="mt-0.5 font-medium text-ink-900">{counterpartyName(terms.recipient)}</dd>
          </div>
          <div>
            <dt className="text-ink-500">Amount</dt>
            <dd className="mt-0.5 font-medium tabular-nums text-ink-900">
              {displayAmount(terms.amount)} {terms.asset}
            </dd>
          </div>
          <div>
            <dt className="text-ink-500">Fee</dt>
            <dd className="mt-0.5 font-medium tabular-nums text-ink-900">
              {displayAmount(terms.fee)} {terms.asset}
            </dd>
          </div>
          <div>
            <dt className="text-ink-500">Total</dt>
            <dd className="mt-0.5 font-medium tabular-nums text-ink-900">
              {terms.total ? displayAmount(terms.total) : "—"} {terms.asset}
            </dd>
          </div>
          {terms.description && (
            <div className="sm:col-span-2">
              <dt className="text-ink-500">Note</dt>
              <dd className="mt-0.5 break-words font-medium text-ink-900">{terms.description}</dd>
            </div>
          )}
        </dl>
        <p className="mt-4 text-sm text-ink-500">
          Internal transfers can&apos;t be reversed once sent. This confirmation is valid for a few minutes on this
          device only.
        </p>
        {error && (
          <Notice tone="error" className="mt-4">
            {error}
          </Notice>
        )}
        <div className="mt-6 flex flex-wrap gap-3">
          <Button type="button" onClick={send} disabled={pending}>
            {pending ? <Spinner /> : <Send aria-hidden />}
            Confirm and send
          </Button>
          <Button
            type="button"
            variant="quiet"
            disabled={pending}
            onClick={() => {
              setError(null);
              setStep({ name: "compose" });
            }}
          >
            <ArrowLeft aria-hidden /> Edit
          </Button>
        </div>
      </section>
    );
  }

  return (
    <form onSubmit={review} noValidate className="grid gap-5 rounded-feature bg-surface p-6 shadow-soft sm:p-8">
      <Field
        label="Recipient's DEYPAY username"
        error={fieldError.username}
        status={
          recipient && (
            <span className="font-medium text-positive">
              {counterpartyName({ username: recipient.username, display_name: recipient.display_name })}
            </span>
          )
        }
      >
        {({ id, describedBy, invalid }) => (
          <div className="flex gap-2">
            <Input
              id={id}
              aria-describedby={describedBy}
              invalid={invalid}
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              placeholder="@username"
              value={username}
              onChange={event => {
                setUsername(event.target.value);
                setRecipient(null);
              }}
            />
            <Button type="button" variant="quiet" onClick={find} disabled={pending}>
              <Search aria-hidden /> Find
            </Button>
          </div>
        )}
      </Field>
      <Field
        label="Asset"
        hint={
          wallet?.balance ? `Available: ${displayAmount(wallet.balance.available) ?? "—"} ${wallet.asset}` : undefined
        }
      >
        {({ id, describedBy }) => (
          <select
            id={id}
            aria-describedby={describedBy}
            value={asset}
            onChange={event => setAsset(event.target.value)}
            className="h-12 w-full rounded-control bg-surface px-4 text-[0.9375rem] text-ink-900 shadow-[inset_0_0_0_1px_rgb(18_16_25/0.09)]"
          >
            {senders.map(w => (
              <option key={w.id} value={w.asset}>
                {w.asset}
              </option>
            ))}
          </select>
        )}
      </Field>
      <Field label="Amount" error={fieldError.amount}>
        {({ id, describedBy, invalid }) => (
          <Input
            id={id}
            aria-describedby={describedBy}
            invalid={invalid}
            inputMode="decimal"
            autoComplete="off"
            placeholder="0.00"
            value={amount}
            onChange={event => setAmount(event.target.value)}
          />
        )}
      </Field>
      <Field label="Note (optional)" error={fieldError.note} hint="Both of you will see it.">
        {({ id, describedBy, invalid }) => (
          <Input
            id={id}
            aria-describedby={describedBy}
            invalid={invalid}
            maxLength={MAX_NOTE}
            value={note}
            onChange={event => setNote(event.target.value)}
          />
        )}
      </Field>
      {error && <Notice tone="error">{error}</Notice>}
      <div>
        <Button type="submit" disabled={pending || !recipient}>
          {pending ? <Spinner /> : null}
          Review transfer
        </Button>
      </div>
    </form>
  );
}
