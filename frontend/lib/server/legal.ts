import type { LegalDocument, LegalIndex } from "../contracts";
import { apiBase } from "./session";

/** The current registration release's legal documents, or null when the registry is unavailable. */
export async function legalIndex(): Promise<LegalIndex | null> {
  try {
    const response = await fetch(new URL("/api/v1/legal/documents", apiBase()), {
      headers: { accept: "application/json" },
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(4000),
    });
    if (!response.ok) return null;
    const body = (await response.json()) as { data?: LegalIndex };
    return Array.isArray(body.data?.documents) ? body.data : null;
  } catch {
    return null;
  }
}

export type LegalText = { status: "ok"; text: string; label: string | null } | { status: "missing" | "unavailable" };

/** The exact Markdown revision, addressed by document ID and version. */
export async function legalText(document: LegalDocument | { document_id: string; version: string }): Promise<LegalText> {
  const path = `/api/v1/legal/documents/${encodeURIComponent(document.document_id)}/${encodeURIComponent(document.version)}`;
  try {
    const response = await fetch(new URL(path, apiBase()), {
      cache: "no-store",
      redirect: "error",
      signal: AbortSignal.timeout(4000),
    });
    if (response.status === 404) return { status: "missing" };
    if (!response.ok) return { status: "unavailable" };
    return { status: "ok", text: await response.text(), label: response.headers.get("x-deypay-legal-status") };
  } catch {
    return { status: "unavailable" };
  }
}
