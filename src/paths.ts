function normalizePortablePath(path: string) {
  const slashPath = path.replace(/\\/g, "/");
  return slashPath.startsWith("//?/UNC/")
    ? `//${slashPath.slice("//?/UNC/".length)}`
    : slashPath.startsWith("//?/")
      ? slashPath.slice("//?/".length)
      : slashPath;
}

function normalizeWindowsPath(path: string) {
  return normalizePortablePath(path).toLocaleLowerCase("en-US");
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

export function parentPath(path: string) {
  if (path.startsWith("\\\\?\\")) {
    const normalized = path.replace(/\//g, "\\").replace(/\\+$/, "");
    const separator = normalized.lastIndexOf("\\");
    if (separator === 6 && /^\\\\\?\\[a-zA-Z]:\\/.test(normalized)) {
      return normalized.slice(0, 7);
    }
    return separator > 3 ? normalized.slice(0, separator) : normalized;
  }
  const normalized = path.replace(/\\/g, "/").replace(/\/+$/, "");
  const separator = normalized.lastIndexOf("/");
  if (separator === 0) return "/";
  if (separator === 2 && /^[a-zA-Z]:\//.test(normalized)) return normalized.slice(0, 3);
  return separator > 0 ? normalized.slice(0, separator) : ".";
}

export function mostSpecificContainingLocation<T extends { path: string }>(
  locations: T[],
  path: string,
) {
  return locations
    .filter((location) => pathBelongsToLocation(path, location.path))
    .sort((left, right) => pathIdentity(right.path).length - pathIdentity(left.path).length)[0];
}

export function relativePathWithinLocation(path: string, locationPath: string) {
  const normalizedPath = normalizePortablePath(path);
  const normalizedRoot = normalizePortablePath(locationPath).replace(/\/+$/, "");
  return normalizedPath.slice(normalizedRoot.length).replace(/^\/+/, "");
}
