import { describe, expect, it } from "vitest";
import { pageTitle } from "./page-title";

describe("pageTitle", () => {
  for (const { description, section, subject, expected } of [
    {
      description: "a section page names its section",
      section: "People",
      subject: undefined,
      expected: "People · SQLBuild",
    },
    {
      description: "a detail page names its subject",
      section: "Runs",
      subject: "Run 1a2b3c4d · orders_job",
      expected: "Run 1a2b3c4d · orders_job · SQLBuild",
    },
    {
      description: "a subject not loaded yet falls back to the section",
      section: "Jobs",
      subject: null,
      expected: "Jobs · SQLBuild",
    },
    {
      description: "a blank subject falls back to the section",
      section: "Plan",
      subject: "  ",
      expected: "Plan · SQLBuild",
    },
  ])
    it(description, () => expect(pageTitle(section, subject)).toBe(expected));
});
