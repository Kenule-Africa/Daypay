import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";

async function readDocument(relativePath) {
  const url = new URL(relativePath, import.meta.url);
  return (await readFile(url, "utf8")).replaceAll("\r\n", "\n");
}

const roadmap = await readDocument("../docs/08-IMPLEMENTATION-PLAN.md");
const tasks = await readDocument("../docs/engineering/TASKS.md");

test("each phase tracks the roadmap's tasks in order, with their original titles", () => {
  const sourcePhases = [...roadmap.matchAll(/^# (\d+)\. PHASE (\d+) — .+$/gm)];
  const trackedPhases = [...tasks.matchAll(/^## Phase (\d+) · .+$/gm)];
  assert.deepEqual(trackedPhases.map(m => m[1]), sourcePhases.map(m => m[2]));

  for (const [i, sourceHeading] of sourcePhases.entries()) {
    const source = roadmap.slice(sourceHeading.index, sourcePhases[i + 1]?.index);
    const tracked = tasks.slice(trackedPhases[i].index, trackedPhases[i + 1]?.index);
    const planned = [...source.matchAll(/^\*\*(IMP-P\d+-\d+):\*\* (.+)$/gm)];
    const rows = [...tracked.matchAll(/^\| (IMP-P\d+-\d+) \| ([^|]+) \| ([^|]+) \|$/gm)];
    const location = `Phase ${sourceHeading[2]}`;
    assert.deepEqual(rows.map(m => [m[1], m[2]]), planned.map(m => [m[1], m[2]]), location);
    for (const row of rows) {
      assert.match(row[3], /^(Backlog|Ready|In progress|In review|In testing|Blocked|Deferred — Post-MVP|Completed)$/, row[1]);
    }

    if (planned.length === 0) {
      const sections = [...source.matchAll(/^## (\d+\.\d+) (.+)$/gm)];
      const trackedSections = [...tracked.matchAll(/^\| §(\d+\.\d+) \| ([^|]+) \|/gm)];
      assert.deepEqual(trackedSections.map(m => [m[1], m[2]]), sections.map(m => [m[1], m[2]]), location);
    }
  }
});

test("provider register covers each written-confirmation item in roadmap §42.2", async () => {
  const validation = roadmap.match(/^## 42\.2 Required Validation\n([\s\S]*?)^## 42\.3 /m);
  assert.ok(validation, "Roadmap provider validation section");
  const requirements = [...validation[1].matchAll(/^\d+\. (.+)\.$/gm)].map(m => m[1]);
  assert.ok(requirements.length > 0);
  const register = await readDocument("../docs/engineering/PROVIDER-CAPABILITIES.md");
  const rows = [...register.matchAll(/^\| ([^|]+) \| (Confirmed|Confirmed with restrictions|Unverified|Unsupported) \|/gm)];
  const recorded = new Set(rows.map(m => m[1]));
  for (const capability of requirements) assert.ok(recorded.has(capability), capability);
});
