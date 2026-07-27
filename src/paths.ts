function normalizeWindowsPath(path: string) {
  const slashPath = path.replace(/\\/g, "/");
  const withoutVerbatimPrefix = slashPath.startsWith("//?/UNC/")
    ? `//${slashPath.slice("//?/UNC/".length)}`
    : slashPath.startsWith("//?/")
      ? slashPath.slice("//?/".length)
      : slashPath;
  return withoutVerbatimPrefix.toLocaleLowerCase("en-US");
}

export function pathIdentity(path: string) {
  const normalized = /^[a-zA-Z]:[\\/]/.test(path) || path.startsWith("\\\\")
    ? normalizeWindowsPath(path)
    : path.replace(/\\/g, "/");
  return normalized.length > 1 ? normalized.replace(/\/+$/, "") : normalized;
}

export function pathsEqual(left: string, right: string) {
  return pathIdentity(left) === pathIdentity(right);
}

export function pathBelongsToLocation(path: string, locationPath: string) {
  const candidate = pathIdentity(path);
  const root = pathIdentity(locationPath);
  return candidate === root || candidate.startsWith(`${root}/`);
}
