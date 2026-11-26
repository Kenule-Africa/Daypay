import { ArrowUpRight, FileText } from "lucide-react";
import type { Metadata } from "next";
import Link from "next/link";
import { LegalFrame } from "@/components/legal/legal-frame";
import { Badge } from "@/components/ui/badge";
import { Notice } from "@/components/ui/notice";
import { legalIndex } from "@/lib/server/legal";

export const metadata: Metadata = { title: "Legal documents" };

export default async function LegalIndexPage() {
  const index = await legalIndex();
  return (
    <LegalFrame>
      <h1 className="font-display text-4xl font-semibold text-ink-950 sm:text-5xl">Legal documents</h1>
      <p className="mt-4 text-lg leading-relaxed text-ink-600">
        The documents that apply when you create a DEYPAY account, at the exact versions you are asked to accept.
      </p>
      {!index ? (
        <Notice tone="warning" className="mt-10" title="Documents are temporarily unavailable">
          We couldn&apos;t load the legal documents right now. Please try again shortly.
        </Notice>
      ) : index.documents.length === 0 ? (
        <Notice tone="info" className="mt-10" title="No documents are published yet">
          There is no current registration release, so no documents are listed.
        </Notice>
      ) : (
        <ul className="mt-10 grid gap-3">
          {index.documents.map(document => (
            <li key={document.document_id}>
              <Link
                href={`/legal/${encodeURIComponent(document.document_id)}/${encodeURIComponent(document.version)}`}
                className="group flex items-center gap-4 rounded-feature bg-surface p-5 shadow-soft transition-[box-shadow,transform] duration-300 ease-out-expo hover:-translate-y-0.5 hover:shadow-lift sm:p-6"
              >
                <span className="inline-flex size-11 shrink-0 items-center justify-center rounded-[0.9rem] bg-brand-50 text-brand-600">
                  <FileText aria-hidden className="size-5" />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block font-display text-lg font-semibold capitalize text-ink-900">
                    {document.title.toLowerCase()}
                  </span>
                  <span className="mt-1 flex flex-wrap items-center gap-2 text-sm text-ink-500">
                    Version {document.version} · {document.document_id}
                    {document.status === "sandbox_draft" && <Badge tone="caution">Sandbox draft</Badge>}
                  </span>
                </span>
                <ArrowUpRight aria-hidden className="size-5 text-ink-400 transition-transform duration-300 group-hover:-translate-y-0.5 group-hover:translate-x-0.5 group-hover:text-brand-500" />
              </Link>
            </li>
          ))}
        </ul>
      )}
    </LegalFrame>
  );
}
