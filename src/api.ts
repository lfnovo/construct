import { invoke } from "@tauri-apps/api/core";
import type { OkfBundleSnapshot, OkfInspection } from "./okf";
import type { FileContent, FileEntry, FileSystemChange, GitDiff, GitInfo, SavedWorkspace } from "./types";

export const api = {
  loadState: () => invoke<Partial<SavedWorkspace>>("load_app_state"),
  saveState: (state: SavedWorkspace) => invoke<void>("save_app_state", { state }),
  setWatchedLocations: (locations: string[]) => invoke<string[]>("set_watched_locations", { locations }),
  listMarkdownFiles: (path: string) => invoke<FileEntry[]>("list_markdown_files", { path }),
  readMarkdownFile: (path: string) => invoke<FileContent>("read_markdown_file", { path }),
  inspectOkfDocument: (request: { content: string; relativePath: string; sourcePath: string; bundleRoot: string; isBundleRoot?: boolean }) =>
    invoke<OkfInspection>("inspect_okf_document", { request }),
  inspectOkfBundle: (path: string) => invoke<OkfBundleSnapshot>("inspect_okf_bundle", { path }),
  readImageDataUrl: (path: string) => invoke<string>("read_image_data_url", { path }),
  writeMarkdownFile: (path: string, content: string) => invoke<void>("write_markdown_file", { request: { path, content } }),
  getGitInfo: (path: string) => invoke<GitInfo>("get_git_info", { path }),
  getGitDiff: (path: string, content?: string) => invoke<GitDiff>("get_git_diff", { path, content }),
  revealInFileManager: (path: string) => invoke<void>("reveal_in_file_manager", { path }),
  openExternalUrl: (url: string) => invoke<void>("open_external_url", { url }),
};

export type { FileSystemChange };
