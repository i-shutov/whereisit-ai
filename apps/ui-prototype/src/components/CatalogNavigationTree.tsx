import React, { useCallback, useEffect, useState } from 'react';
import { ChevronDown, ChevronRight, Folder, HardDrive, LoaderCircle } from 'lucide-react';
import type { CatalogEntry, CatalogVolume } from '../types/atlas';
import { useI18n } from '../i18n';
import { folderChildrenOnPage } from '../utils/catalogNavigation';

interface ChildPage {
  items: CatalogEntry[];
  total: number;
  hasMore: boolean;
  offset: number;
}

interface TreePageState {
  folders: CatalogEntry[];
  nextOffset: number;
  hasMore: boolean;
  loading: boolean;
  loaded: boolean;
}

interface CatalogNavigationTreeProps {
  volumes: CatalogVolume[];
  activeVolumeId: number | null;
  activeParentId: number | null;
  activeCrumbIds: number[];
  browseActive: boolean;
  onOpenVolume: (volume: CatalogVolume) => void;
  onOpenFolder: (folder: CatalogEntry) => void;
}

const rootKey = (volumeId: number) => `v:${volumeId}:root`;
const folderKey = (volumeId: number, folderId: number) => `v:${volumeId}:f:${folderId}`;

/** Lazy folder-only tree over the prototype's existing paginated direct-child browse API. */
export const CatalogNavigationTree: React.FC<CatalogNavigationTreeProps> = ({
  volumes,
  activeVolumeId,
  activeParentId,
  activeCrumbIds,
  browseActive,
  onOpenVolume,
  onOpenFolder,
}) => {
  const { t } = useI18n();
  const [expanded, setExpanded] = useState<Set<string>>(() => new Set());
  const [pages, setPages] = useState<Map<string, TreePageState>>(() => new Map());
  const crumbKey = activeCrumbIds.join(',');

  const loadPage = useCallback(async (volumeId: number, parentId: number | null, append: boolean) => {
    const key = parentId === null ? rootKey(volumeId) : folderKey(volumeId, parentId);
    const old = pages.get(key);
    if (old?.loading || (old?.loaded && !append)) return;
    const offset = append ? old?.nextOffset ?? 0 : 0;
    setPages((prev) => {
      const next = new Map(prev);
      next.set(key, { folders: old?.folders ?? [], nextOffset: offset, hasMore: old?.hasMore ?? false, loading: true, loaded: old?.loaded ?? false });
      return next;
    });
    try {
      const response = await fetch('/api/catalog/query', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ requestId: `tree-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, mode: 'browse', volumeId, parentId, offset, limit: 250 }),
      });
      if (!response.ok) throw new Error(`HTTP ${response.status}`);
      const page = (await response.json()) as ChildPage;
      setPages((prev) => {
        const next = new Map(prev);
        const current = next.get(key);
        if (!current) return next;
        const folders = folderChildrenOnPage(page.items);
        next.set(key, {
          folders: append ? [...current.folders, ...folders] : folders,
          nextOffset: page.offset + page.items.length,
          hasMore: page.hasMore,
          loading: false,
          loaded: true,
        });
        return next;
      });
    } catch {
      setPages((prev) => {
        const next = new Map(prev);
        const current = next.get(key);
        if (current) next.set(key, { ...current, loading: false });
        return next;
      });
    }
  }, [pages]);

  useEffect(() => {
    if (!browseActive || activeVolumeId === null) return;
    const ids = crumbKey ? crumbKey.split(',').map(Number).filter(Number.isFinite) : [];
    if (activeParentId !== null && !ids.includes(activeParentId)) ids.push(activeParentId);
    setExpanded((prev) => {
      const next = new Set(prev);
      next.add(rootKey(activeVolumeId));
      for (const id of ids) next.add(folderKey(activeVolumeId, id));
      return next;
    });
    void loadPage(activeVolumeId, null, false);
    for (const id of ids) void loadPage(activeVolumeId, id, false);
  }, [browseActive, activeVolumeId, activeParentId, crumbKey]);

  const toggle = (volumeId: number, parentId: number | null) => {
    const key = parentId === null ? rootKey(volumeId) : folderKey(volumeId, parentId);
    const wasExpanded = expanded.has(key);
    setExpanded((prev) => {
      const next = new Set(prev);
      if (wasExpanded) next.delete(key); else next.add(key);
      return next;
    });
    if (!wasExpanded) void loadPage(volumeId, parentId, false);
  };

  const renderChildren = (volume: CatalogVolume, parentId: number | null, depth: number): React.ReactNode => {
    const key = parentId === null ? rootKey(volume.id) : folderKey(volume.id, parentId);
    if (!expanded.has(key)) return null;
    const page = pages.get(key);
    if (!page) return <div className="px-2 py-1 text-[10px] text-slate-500" style={{ paddingLeft: `${12 + depth * 12}px` }}>{t('Загрузка…', 'Loading…')}</div>;
    return <>
      {page.folders.map((folder) => {
        const childKey = folderKey(volume.id, folder.id);
        const childExpanded = expanded.has(childKey);
        const selected = activeVolumeId === volume.id && activeParentId === folder.id;
        const childPage = pages.get(childKey);
        return <React.Fragment key={folder.id}>
          <div className={`flex items-center min-w-0 text-[11px] ${selected ? 'bg-blue-600/15 text-slate-950 dark:text-white' : 'text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-900'}`} style={{ paddingLeft: `${4 + depth * 12}px` }}>
            <button type="button" onClick={() => toggle(volume.id, folder.id)} aria-label={`${childExpanded ? 'Collapse' : 'Expand'} ${folder.name}`} className="p-0.5 shrink-0 cursor-pointer">
              {childPage?.loading ? <LoaderCircle className="w-3 h-3 animate-spin" /> : childExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
            </button>
            <button type="button" onClick={() => onOpenFolder(folder)} className="flex items-center gap-1 min-w-0 flex-1 px-1 py-1 text-left cursor-pointer">
              <Folder className="w-3 h-3 text-amber-500 shrink-0" /><span className="truncate" title={folder.name}>{folder.name}</span>
            </button>
          </div>
          {renderChildren(volume, folder.id, depth + 1)}
        </React.Fragment>;
      })}
      {page.folders.length === 0 && !page.hasMore && <div className="px-2 py-1 text-[10px] text-slate-500" style={{ paddingLeft: `${18 + depth * 12}px` }}>{t('Папок нет', 'No folders')}</div>}
      {page.hasMore && <button type="button" disabled={page.loading} onClick={() => void loadPage(volume.id, parentId, true)} style={{ paddingLeft: `${18 + depth * 12}px` }} className="w-full py-1 pr-2 text-left text-[10px] text-blue-600 dark:text-blue-400 hover:underline disabled:opacity-50 cursor-pointer">{page.loading ? t('Загрузка…', 'Loading…') : t('Загрузить ещё папки…', 'More folders…')}</button>}
    </>;
  };

  return <div className="py-1">
    {volumes.map((volume) => {
      const key = rootKey(volume.id);
      const isExpanded = expanded.has(key);
      const rootSelected = activeVolumeId === volume.id && activeParentId === null;
      const page = pages.get(key);
      return <React.Fragment key={volume.id}>
        <div className={`flex items-center min-w-0 text-[11px] ${rootSelected ? 'bg-blue-600/15 text-slate-950 dark:text-white' : 'text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-900'}`}>
          <button type="button" onClick={() => toggle(volume.id, null)} aria-label={`${isExpanded ? 'Collapse' : 'Expand'} ${volume.label}`} className="p-1 shrink-0 cursor-pointer">
            {page?.loading ? <LoaderCircle className="w-3 h-3 animate-spin" /> : isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
          </button>
          <button type="button" onClick={() => onOpenVolume(volume)} className="flex items-center gap-1 min-w-0 flex-1 px-1 py-1 text-left cursor-pointer">
            <HardDrive className="w-3 h-3 text-slate-400 shrink-0" /><span className="font-mono shrink-0">#{volume.id}</span><span className="truncate">{volume.label}</span>
          </button>
        </div>
        {renderChildren(volume, null, 0)}
      </React.Fragment>;
    })}
  </div>;
};
