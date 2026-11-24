import type { CatalogEntry } from '../types/atlas';

export interface FolderCrumb {
  id: number;
  name: string;
}

export interface EmptyQueryDestination {
  view: 'overview' | 'browse';
  volumeId: number | null;
  parentId: number | null;
}

export interface FolderIdentity {
  id: number;
  name: string;
  volumeId: number;
  parentId: number | null;
}

export function destinationForEmptyQuery(volumeId: number | null, parentId: number | null): EmptyQueryDestination {
  return volumeId === null
    ? { view: 'overview', volumeId: null, parentId: null }
    : { view: 'browse', volumeId, parentId };
}

export function breadcrumbPath(ancestors: FolderCrumb[], index: number): { parentId: number | null; crumbs: FolderCrumb[] } {
  if (index < 0) return { parentId: null, crumbs: [] };
  const crumbs = ancestors.slice(0, index + 1);
  return { parentId: crumbs[crumbs.length - 1]?.id ?? null, crumbs };
}

export function folderChildrenOnPage(items: CatalogEntry[]): CatalogEntry[] {
  return items.filter((item) => item.kind === 'directory');
}

/** Keep a known ancestor visible when it lies beyond the current raw-child page. */
export function includeFocusedFolder(folders: FolderIdentity[], focused: FolderIdentity | null): FolderIdentity[] {
  if (!focused || folders.some((folder) => folder.id === focused.id)) return folders;
  return [...folders, focused];
}
