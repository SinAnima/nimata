import { describe, expect, it } from "vitest";
import { authorWallClock, dayGroup, formatOffset } from "./time";

describe("authorWallClock", () => {
  const instant = Date.UTC(2026, 9, 4, 16, 41, 7);

  it("shows the author's local time and offset, whatever the viewer's zone", () => {
    expect(authorWallClock(instant, -240)).toBe(
      "2026-10-04 12:41:07 UTC-04:00",
    );
    expect(authorWallClock(instant, 180)).toBe("2026-10-04 19:41:07 UTC+03:00");
  });

  it("formats half-hour offsets", () => {
    expect(formatOffset(330)).toBe("UTC+05:30");
  });
});

describe("dayGroup", () => {
  it("groups by local calendar day", () => {
    const now = new Date(2026, 9, 4, 9, 0).getTime();
    expect(dayGroup(new Date(2026, 9, 4, 0, 5).getTime(), now)).toBe("Today");
    expect(dayGroup(new Date(2026, 9, 3, 23, 55).getTime(), now)).toBe(
      "Yesterday",
    );
    expect(dayGroup(new Date(2026, 8, 30).getTime(), now)).toBe("Earlier");
  });
});
