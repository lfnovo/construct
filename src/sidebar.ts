import type { SidebarPanelSizes, SidebarSectionId } from "./types";

export const sidebarSectionIds: SidebarSectionId[] = ["locations", "files", "history"];

export const defaultSidebarPanelSizes: SidebarPanelSizes = {
  locations: 0.25,
  files: 0.47,
  history: 0.28,
};

export function sanitizeSidebarPanelSizes(
  value: Partial<SidebarPanelSizes> | undefined,
): SidebarPanelSizes {
  const candidate = Object.fromEntries(sidebarSectionIds.map((section) => {
    const size = value?.[section];
    return [section, typeof size === "number" && Number.isFinite(size) && size > 0
      ? size
      : defaultSidebarPanelSizes[section]];
  })) as SidebarPanelSizes;
  const total = sidebarSectionIds.reduce((sum, section) => sum + candidate[section], 0);
  return Object.fromEntries(sidebarSectionIds.map((section) => [
    section,
    candidate[section] / total,
  ])) as SidebarPanelSizes;
}

export function resizeSidebarPanelPair(
  sizes: SidebarPanelSizes,
  upper: SidebarSectionId,
  lower: SidebarSectionId,
  upperHeight: number,
  lowerHeight: number,
  delta: number,
  minimumHeight = 80,
): SidebarPanelSizes {
  const totalHeight = upperHeight + lowerHeight;
  if (!Number.isFinite(totalHeight) || totalHeight <= minimumHeight * 2) return sizes;
  const nextUpperHeight = Math.max(
    minimumHeight,
    Math.min(totalHeight - minimumHeight, upperHeight + delta),
  );
  const pairWeight = sizes[upper] + sizes[lower];
  return {
    ...sizes,
    [upper]: pairWeight * (nextUpperHeight / totalHeight),
    [lower]: pairWeight * ((totalHeight - nextUpperHeight) / totalHeight),
  };
}
