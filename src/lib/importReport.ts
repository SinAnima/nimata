// What an import did, in a sentence or two.

import type { ImportReport, ImportSource } from "./types";

const SOURCES: Record<ImportSource, string> = {
  nimataDiscussion: "the Nimata discussion",
  nimataArchive: "the Nimata archive",
  chatGpt: "the ChatGPT export",
};

const plural = (n: number, one: string, many = `${one}s`) =>
  `${n} ${n === 1 ? one : many}`;

export function describeImport(report: ImportReport): string {
  const count = (result: string) =>
    report.outcomes.filter((o) => o.result === result);
  const added = count("added");
  const updated = count("updated");
  const unchanged = count("unchanged");
  const deleted = count("skippedDeleted");
  const posts = (list: typeof added) =>
    list.reduce((sum, o) => sum + o.postsAdded, 0);

  const parts: string[] = [];
  if (added.length > 0) {
    parts.push(
      `${plural(added.length, "new discussion")} (${plural(posts(added), "post")})`,
    );
  }
  if (updated.length > 0) {
    parts.push(
      `${plural(posts(updated), "new post")} in ${plural(updated.length, "discussion")} you already had`,
    );
  }
  let text =
    parts.length > 0
      ? `Imported ${parts.join(" and ")} from ${SOURCES[report.source]}.`
      : `Nothing new in ${SOURCES[report.source]}.`;
  if (unchanged.length > 0) {
    text += ` ${plural(unchanged.length, "discussion was", "discussions were")} already here.`;
  }
  if (deleted.length > 0) {
    text += ` ${plural(deleted.length, "discussion you deleted was", "discussions you deleted were")} left out.`;
  }
  const missing = report.outcomes.reduce(
    (sum, o) => sum + (o.attachmentsMissing ?? 0),
    0,
  );
  if (missing > 0) {
    text += ` ${plural(missing, "attached file was", "attached files were")} not in the file and ${missing === 1 ? "was" : "were"} left out.`;
  }
  if (report.failures.length > 0) {
    text += ` ${plural(report.failures.length, "discussion")} could not be imported: ${report.failures.join("; ")}`;
  }
  return text;
}
