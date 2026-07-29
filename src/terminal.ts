import type { TerminalApplication, TerminalApplicationId } from "./types";

export function relativeDirectoryForFile(relativePath: string): string {
  const normalized = relativePath.replace(/\\/g, "/");
  const separator = normalized.lastIndexOf("/");
  return separator < 0 ? "" : normalized.slice(0, separator);
}

export function selectedTerminal(
  applications: TerminalApplication[],
  selectedId: TerminalApplicationId | undefined,
): TerminalApplication | undefined {
  return applications.find((application) => application.id === selectedId);
}
