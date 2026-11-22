import { describe, expect, it } from "vitest";
import { clickGuard } from "./click-guard";

describe("clickGuard", () => {
  for (const { description, down, up, expected } of [
    {
      description: "a press that did not move is a click",
      down: { clientX: 10, clientY: 10 },
      up: { clientX: 10, clientY: 10 },
      expected: true,
    },
    {
      description: "a press within the slop is still a click",
      down: { clientX: 10, clientY: 10 },
      up: { clientX: 13, clientY: 12 },
      expected: true,
    },
    {
      description: "a press that moved further is a drag, not a click",
      down: { clientX: 10, clientY: 10 },
      up: { clientX: 40, clientY: 10 },
      expected: false,
    },
    {
      description:
        "a click without a recorded press, such as from the keyboard, counts",
      down: null,
      up: { clientX: 40, clientY: 10 },
      expected: true,
    },
  ])
    it(description, () => {
      const guard = clickGuard();
      if (down) guard.down(down);
      expect(guard.still(up)).toBe(expected);
    });

  it("forgets a press once its click is judged", () => {
    const guard = clickGuard();
    guard.down({ clientX: 0, clientY: 0 });
    guard.still({ clientX: 50, clientY: 0 });
    expect(guard.still({ clientX: 50, clientY: 0 })).toBe(true);
  });
});
