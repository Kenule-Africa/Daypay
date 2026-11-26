import * as React from "react";

// A small Markdown renderer for DEYPAY's legal documents. It builds React
// elements directly (never raw HTML), so document text cannot inject markup.
// Supported: headings, paragraphs with hard line breaks, bullet and numbered
// lists, block quotes, pipe tables, rules, **bold**, *italic* and `code`.

function inline(text: string, keyPrefix: string): React.ReactNode[] {
  const parts: React.ReactNode[] = [];
  const pattern = /(\*\*[^*]+\*\*|`[^`]+`|\*[^*\s][^*]*\*)/g;
  let last = 0;
  let index = 0;
  for (const match of text.matchAll(pattern)) {
    const start = match.index ?? 0;
    if (start > last) parts.push(text.slice(last, start));
    const token = match[0];
    const key = `${keyPrefix}-${index++}`;
    if (token.startsWith("**")) parts.push(<strong key={key} className="font-semibold text-ink-900">{token.slice(2, -2)}</strong>);
    else if (token.startsWith("`")) parts.push(<code key={key} className="rounded bg-paper-2 px-1 py-0.5 text-[0.9em]">{token.slice(1, -1)}</code>);
    else parts.push(<em key={key}>{token.slice(1, -1)}</em>);
    last = start + token.length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return parts;
}

function lines(paragraph: string[], key: string): React.ReactNode[] {
  return paragraph.flatMap((line, index) => {
    const content = inline(line.replace(/\s+$/, ""), `${key}-${index}`);
    return index < paragraph.length - 1 ? [...content, <br key={`${key}-br-${index}`} />] : content;
  });
}

const headingClasses = [
  "font-display text-3xl font-semibold text-ink-950 sm:text-4xl",
  "mt-12 font-display text-2xl font-semibold text-ink-950",
  "mt-9 font-display text-xl font-semibold text-ink-900",
  "mt-7 font-display text-lg font-semibold text-ink-900",
];

export function LegalMarkdown({ source }: { source: string }) {
  const blocks: React.ReactNode[] = [];
  const rows = source.replace(/\r\n?/g, "\n").split("\n");
  let i = 0;
  let key = 0;
  const isBlank = (line: string) => line.trim() === "";
  while (i < rows.length) {
    const line = rows[i];
    if (isBlank(line)) {
      i++;
      continue;
    }
    const heading = /^(#{1,6})\s+(.*)$/.exec(line);
    if (heading) {
      const level = Math.min(heading[1].length, 4);
      const Tag = `h${level}` as "h1";
      blocks.push(<Tag key={key++} className={headingClasses[level - 1]}>{inline(heading[2], `h${key}`)}</Tag>);
      i++;
      continue;
    }
    if (/^(-{3,}|\*{3,}|_{3,})\s*$/.test(line)) {
      blocks.push(<hr key={key++} className="my-10 h-px border-0 bg-ink-900/8" />);
      i++;
      continue;
    }
    if (line.trimStart().startsWith("|")) {
      const table: string[][] = [];
      while (i < rows.length && rows[i].trimStart().startsWith("|")) {
        const cells = rows[i].trim().replace(/^\||\|$/g, "").split("|").map(cell => cell.trim());
        if (!cells.every(cell => /^:?-{2,}:?$/.test(cell))) table.push(cells);
        i++;
      }
      const [head, ...body] = table;
      blocks.push(
        <div key={key++} className="my-6 overflow-x-auto rounded-panel bg-surface shadow-soft">
          <table className="w-full text-left text-sm">
            <thead className="bg-paper-2 text-ink-800">
              <tr>{head?.map((cell, c) => <th key={c} className="px-4 py-3 font-semibold">{inline(cell, `th${key}${c}`)}</th>)}</tr>
            </thead>
            <tbody>
              {body.map((row, r) => (
                <tr key={r} className="even:bg-paper/60">
                  {row.map((cell, c) => <td key={c} className="px-4 py-3 align-top text-ink-600">{inline(cell, `td${key}${r}${c}`)}</td>)}
                </tr>
              ))}
            </tbody>
          </table>
        </div>,
      );
      continue;
    }
    const listItem = /^\s*([-*+]|\d+[.)])\s+(.*)$/;
    if (listItem.test(line)) {
      const ordered = /^\s*\d/.test(line);
      const items: string[] = [];
      while (i < rows.length && listItem.test(rows[i])) {
        items.push(listItem.exec(rows[i])![2]);
        i++;
      }
      const ListTag = ordered ? "ol" : "ul";
      blocks.push(
        <ListTag key={key++} className={`my-4 grid gap-2 pl-6 text-ink-700 ${ordered ? "list-decimal" : "list-disc"} marker:text-brand-400`}>
          {items.map((item, n) => <li key={n} className="pl-1 leading-relaxed">{inline(item, `li${key}${n}`)}</li>)}
        </ListTag>,
      );
      continue;
    }
    if (line.startsWith(">")) {
      const quote: string[] = [];
      while (i < rows.length && rows[i].startsWith(">")) {
        quote.push(rows[i].replace(/^>\s?/, ""));
        i++;
      }
      blocks.push(
        <blockquote key={key++} className="my-6 rounded-panel bg-brand-50 px-5 py-4 text-brand-900">
          {lines(quote, `q${key}`)}
        </blockquote>,
      );
      continue;
    }
    const paragraph: string[] = [];
    while (
      i < rows.length &&
      !isBlank(rows[i]) &&
      !/^(#{1,6})\s/.test(rows[i]) &&
      !rows[i].trimStart().startsWith("|") &&
      !listItem.test(rows[i]) &&
      !rows[i].startsWith(">")
    ) {
      paragraph.push(rows[i]);
      i++;
    }
    blocks.push(<p key={key++} className="my-4 leading-relaxed text-ink-700">{lines(paragraph, `p${key}`)}</p>);
  }
  return <div className="text-[1.0625rem]">{blocks}</div>;
}
