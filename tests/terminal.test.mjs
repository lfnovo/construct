import assert from "node:assert/strict";
import test from "node:test";
import {
  relativeDirectoryForFile,
  selectedTerminal,
} from "../src/terminal.ts";

test("derives a portable relative directory from a file path", () => {
  assert.equal(relativeDirectoryForFile("README.md"), "");
  assert.equal(relativeDirectoryForFile("docs/guide.md"), "docs");
  assert.equal(relativeDirectoryForFile(String.raw`docs\guides\windows.md`), "docs/guides");
});

test("keeps a saved terminal selection only while it remains available", () => {
  const applications = [
    { id: "apple-terminal", label: "Terminal" },
    { id: "ghostty", label: "Ghostty" },
  ];
  assert.equal(selectedTerminal(applications, "ghostty")?.label, "Ghostty");
  assert.equal(selectedTerminal(applications, "wezterm"), undefined);
  assert.equal(selectedTerminal(applications, undefined), undefined);
});
