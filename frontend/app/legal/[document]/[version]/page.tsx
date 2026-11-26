import type { Metadata } from "next";
import { notFound } from "next/navigation";
import { LegalFrame } from "@/components/legal/legal-frame";
import { LegalMarkdown } from "@/components/legal/markdown";
import { Badge } from "@/components/ui/badge";
import { Notice } from "@/components/ui/notice";
import { legalText } from "@/lib/server/legal";

export const metadata: Metadata = { title: "Legal document" };

type Params = Promise<{ document: string; version: string }>;

function decode(value: string): string | null {
  try {
    return decodeURIComponent(value);
  } catch {
    return null;
  }
}

export default async function LegalDocumentPage({ params }: { params: Params }) {
  const { document, version } = await params;
  const documentId = decode(document);
  const revision = decode(version);
  if (!documentId || !revision || !/^DEP-LEG-\d{3}$/.test(documentId) || revision.length > 64) notFound();
  const result = await legalText({ document_id: documentId, version: revision });
  if (result.status === "missing") notFound();
  return (
    <LegalFrame>
      <div className="mb-8 flex flex-wrap items-center gap-2 text-sm text-ink-500">
        <span>
          {documentId} · Version {revision}
        </span>
        {result.status === "ok" && result.label === "sandbox_draft" && <Badge tone="caution">Sandbox draft</Badge>}
      </div>
      {result.status === "ok" ? (
        <article>
          <LegalMarkdown source={result.text} />
        </article>
      ) : (
        <Notice tone="warning" title="This document is temporarily unavailable">
          We couldn&apos;t load it right now. Please try again shortly.
        </Notice>
      )}
    </LegalFrame>
  );
}
