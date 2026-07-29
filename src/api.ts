import { invoke } from "@tauri-apps/api/core";
import type { OkfBundleSnapshot, OkfInspection } from "./okf";
import type {
  ContextDocumentRef, ContextPackResponse, FileContent, FileEntry, FileSystemChange,
  GitDiff, GitInfo, IndexedDocument,
  IndexSearchResult, IndexStatus, KnowledgeSearchFilters, KnowledgeSearchResponse,
  LocationRecord, OpenTerminalResult, RelatedDocumentsResponse, SavedWorkspace,
  SearchFacets, TerminalApplication, TerminalApplicationId,
} from "./types";

export const api = {
  loadState: () => invoke<Partial<SavedWorkspace>>("load_app_state"),
  saveState: (state: SavedWorkspace) => invoke<void>("save_app_state", { state }),
  setWatchedLocations: (locations: Pick<LocationRecord, "id" | "path">[]) =>
    invoke<string[]>("set_watched_locations", { locations }),
  listMarkdownFiles: (path: string) => invoke<FileEntry[]>("list_markdown_files", { path }),
  readMarkdownFile: (path: string) => invoke<FileContent>("read_markdown_file", { path }),
  inspectOkfDocument: (request: { content: string; relativePath: string; sourcePath: string; bundleRoot: string; isBundleRoot?: boolean }) =>
    invoke<OkfInspection>("inspect_okf_document", { request }),
  inspectOkfBundle: (path: string) => invoke<OkfBundleSnapshot>("inspect_okf_bundle", { path }),
  syncLocationIndex: (request: { locationId: string; rootPath: string; displayName: string; okfBundle: boolean; rebuild?: boolean }) =>
    invoke<IndexStatus>("sync_location_index", { request }),
  getLocationIndexStatus: (locationId: string) => invoke<IndexStatus>("get_location_index_status", { locationId }),
  searchLocationIndex: (request: { locationId: string; query: string; limit?: number }) =>
    invoke<IndexSearchResult[]>("search_location_index", { request }),
  searchKnowledge: (request: { locationIds: string[]; query: string; filters: KnowledgeSearchFilters; limit?: number }) =>
    invoke<KnowledgeSearchResponse>("search_knowledge", { request }),
  getSearchFacets: (locationIds: string[]) =>
    invoke<SearchFacets>("get_search_facets", { request: { locationIds } }),
  getIndexedDocument: (locationId: string, relativePath: string) =>
    invoke<IndexedDocument | null>("get_indexed_document", { locationId, relativePath }),
  getRelatedDocuments: (locationId: string, relativePath: string, limit = 20) =>
    invoke<RelatedDocumentsResponse>("get_related_documents", {
      request: { locationId, relativePath, limit },
    }),
  buildContextPack: (request: {
    query: string;
    documents: ContextDocumentRef[];
    maxCharacters: number;
    maxDocuments?: number;
  }) => invoke<ContextPackResponse>("build_context_pack", { request }),
  deleteLocationIndex: (locationId: string) => invoke<void>("delete_location_index", { locationId }),
  getMcpConfiguration: (locationId: string) =>
    invoke<string>("get_mcp_configuration", { locationId }),
  readImageDataUrl: (path: string) => invoke<string>("read_image_data_url", { path }),
  writeMarkdownFile: (path: string, content: string) => invoke<void>("write_markdown_file", { request: { path, content } }),
  getGitInfo: (path: string) => invoke<GitInfo>("get_git_info", { path }),
  getGitDiff: (path: string, content?: string) => invoke<GitDiff>("get_git_diff", { path, content }),
  revealInFileManager: (path: string) => invoke<void>("reveal_in_file_manager", { path }),
  openExternalUrl: (url: string) => invoke<void>("open_external_url", { url }),
  listTerminalApplications: () =>
    invoke<TerminalApplication[]>("list_terminal_applications"),
  openTerminal: (request: {
    locationId: string;
    relativeDirectory?: string;
    terminalApplicationId: TerminalApplicationId;
  }) => invoke<OpenTerminalResult>("open_terminal", { request }),
};

export type { FileSystemChange };
