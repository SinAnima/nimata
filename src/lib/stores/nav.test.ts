import { describe, expect, it } from "vitest";
import { Nav } from "./nav.svelte";

describe("Nav", () => {
  it("opens a discussion and returns to the list on phones", () => {
    const nav = new Nav();
    expect(nav.screen).toBe("discussions");
    nav.open("d1");
    expect(nav.screen).toBe("discussion");
    expect(nav.selectedId).toBe("d1");
    nav.back();
    expect(nav.screen).toBe("discussions");
    expect(nav.selectedId).toBe("d1");
  });
});
