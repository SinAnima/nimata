import { describe, expect, it } from "vitest";
import { canRead, humanSize, typeLabel, unreadable } from "./attach";
import type {
  Capabilities,
  ModelParticipant,
  ProviderView,
  StagedAttachment,
} from "./types";

const caps = (images: boolean, pdfs: boolean): Capabilities => ({
  requiresKey: true,
  customEndpoint: false,
  multiple: false,
  modelDiscovery: true,
  streaming: true,
  usage: true,
  images,
  pdfs,
});

const file = (
  filename: string,
  kind: StagedAttachment["kind"],
  mediaType: string,
): StagedAttachment => ({
  filename,
  kind,
  mediaType,
  size: 10,
  contentHash: `sha256:${filename}`,
});

const model = (id: string, providerId: string): ModelParticipant =>
  ({
    participant: {
      id,
      kind: "model",
      displayName: id,
      provider: null,
      model: id,
    },
    providerId,
    enabled: true,
    aliases: [],
  }) as ModelParticipant;

describe("attached files", () => {
  it("labels types and sizes for people", () => {
    expect(typeLabel(file("notes.md", "text", "text/markdown"))).toBe(
      "Markdown",
    );
    expect(typeLabel(file("main.rs", "text", "text/x-rust"))).toBe("Text");
    expect(typeLabel(file("report.docx", "other", "application/x"))).toBe(
      "DOCX file",
    );
    expect(humanSize(512)).toBe("512 bytes");
    expect(humanSize(2355)).toBe("2.3 KB");
    expect(humanSize(3 * 1024 * 1024)).toBe("3.0 MB");
  });

  it("knows which models cannot read which files", () => {
    expect(canRead(caps(false, false), "text")).toBe(true);
    expect(canRead(caps(true, false), "pdf")).toBe(false);
    const providers = [
      { provider: { id: "anthropic" }, capabilities: caps(true, true) },
      { provider: { id: "ollama" }, capabilities: caps(false, false) },
    ] as unknown as ProviderView[];
    const files = [
      file("notes.md", "text", "text/markdown"),
      file("paper.pdf", "pdf", "application/pdf"),
      file("data.zip", "other", "application/zip"),
    ];
    const result = unreadable(
      [model("claude", "anthropic"), model("qwen", "ollama")],
      providers,
      files,
    );
    expect(
      result.map((r) => [
        r.model.participant.id,
        r.files.map((f) => f.filename),
      ]),
    ).toEqual([
      ["claude", ["data.zip"]],
      ["qwen", ["paper.pdf", "data.zip"]],
    ]);
  });
});
