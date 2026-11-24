import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowDown,
  ArrowLeft,
  ArrowUp,
  BarChart3,
  Check,
  ChevronDown,
  ChevronRight,
  Copy,
  FileSearch,
  Folder,
  HardDrive,
  HelpCircle,
  Info,
  Layers,
  PanelLeft,
  Pencil,
  PieChart,
  RefreshCw,
  Search,
  Settings,
  ShieldAlert,
  Square,
  X,
} from 'lucide-react';
import type {
  CatalogEntry,
  CatalogPageResponse,
  CatalogVolume,
  DuplicateCriterion,
  SystemSnapshot,
} from '../types/atlas';
import {
  buildFullDiskRegisterText,
  buildPickupCardText,
  formatBytes,
  formatDuration,
  formatExactBytes,
  formatUnixDate,
} from '../utils/format';
import { useI18n } from '../i18n';
import { CatalogNavigationTree } from './CatalogNavigationTree';
import { buildSearchJumpDefaults } from '../utils/searchJump';
import { breadcrumbPath, destinationForEmptyQuery, type FolderIdentity } from '../utils/catalogNavigation';

interface NavigationSnapshot {
  mode: 'search' | 'browse' | 'duplicates' | 'copy_candidates';
  duplicateMode: DuplicateCriterion;
  query: string;
  queryInput: string;
  activeTypeChip: string;
  activeSizeChip: string;
  volumeId: number | null;
  parentId: number | null;
  folderPrefix: string | null;
  category: string | null;
  sourceEntryId: number | null;
  candidateVolumeId: number | null;
  selectedEntryId: number | null;
  offset: number;
  crumbs: { id: number; name: string }[];
  catalogOverview: boolean;
}

export interface SearchPresetJump {
  nonce: number;
  volumeId: number | null;
  folderPrefix: string | null;
  folderEntryId?: number | null;
  query?: string;
  copySourceEntryId?: number | null;
}

interface LocatorWorkspaceProps {
  isActive: boolean;
  snapshot: SystemSnapshot;
  presetJump: SearchPresetJump | null;
  onRefreshSnapshot: () => Promise<void>;
  onOpenSunburst: (volumeId: number | null, folderPath: string | null) => void;
  onOpenSlices: (volumeId: number | null) => void;
  onOpenDiskProperties: (volumeId: number) => void;
  onOpenScanModal: (volumeId: number | null) => void;
  onNotify: (message: string, tone?: 'info' | 'error' | 'success') => void;
}

type SortColumn = 'natural' | 'name' | 'disk' | 'duration' | 'size' | 'modified' | 'candidates';
type RegisterFilterMode = 'all' | 'current_page' | 'copy_candidates';
type RegisterOrder = 'id' | 'volume';

export const LocatorWorkspace: React.FC<LocatorWorkspaceProps> = ({
  isActive,
  snapshot,
  presetJump,
  onRefreshSnapshot,
  onOpenSunburst,
  onOpenSlices,
  onOpenDiskProperties,
  onOpenScanModal,
  onNotify,
}) => {
  const { lang, t } = useI18n();

  const quickVideoChips = useMemo(
    () => [
      { label: t('Все', 'All'), token: '' },
      { label: '4K (2160p)', token: 'res:2160p' },
      { label: '1080p', token: 'res:1080p' },
      { label: '720p', token: 'res:720p' },
      { label: 'HEVC', token: 'codec:hevc' },
      { label: 'AVC', token: 'codec:avc' },
      { label: t('Субтитры', 'Subtitles'), token: 'ext:srt' },
    ],
    [t]
  );

  const quickSizeChips = useMemo(
    () => [
      { label: t('Любой размер', 'Any size'), token: '' },
      { label: '> 40 GB', token: 'size:>40gb' },
      { label: '> 15 GB', token: 'size:>15gb' },
      { label: '< 10 GB', token: 'size:<10gb' },
    ],
    [t]
  );

  const translateCategory = (cat: string) => {
    if (lang === 'ru') return cat;
    const map: Record<string, string> = {
      'Кино UHD / BD-Remux': 'UHD Cinema / BD-Remux',
      'Классика и Кино СССР': 'Classics & Soviet Cinema',
      'Сериалы UHD/1080p': 'Series UHD/1080p',
      'Документалистика 4K/HD': 'Documentaries 4K/HD',
      'Концерты и Музыкальное видео': 'Concerts & Music Video',
      'Субтитры и метаданные': 'Subtitles & Metadata',
      'Видео и кино': 'Video & Cinema',
      'Прочие файлы': 'Other Files',
    };
    return map[cat] ?? cat;
  };

  // Search & scope state
  const [queryInput, setQueryInput] = useState('');
  const [appliedQuery, setAppliedQuery] = useState('');
  const [activeTypeChip, setActiveTypeChip] = useState('');
  const [activeSizeChip, setActiveSizeChip] = useState('');
  const [mode, setMode] = useState<'search' | 'browse' | 'duplicates' | 'copy_candidates'>('search');
  const [catalogOverview, setCatalogOverview] = useState(true);
  const [duplicateMode, setDuplicateMode] = useState<DuplicateCriterion>('video_rips');
  const [showDupExplainer, setShowDupExplainer] = useState(false);
  const [scopedVolumeId, setScopedVolumeId] = useState<number | null>(null);
  const [browseParentId, setBrowseParentId] = useState<number | null>(null);
  const [browseCrumbs, setBrowseCrumbs] = useState<{ id: number; name: string }[]>([]);
  const [folderPrefix, setFolderPrefix] = useState<string | null>(null);
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [copySourceEntryId, setCopySourceEntryId] = useState<number | null>(null);
  const [candidateVolumeId, setCandidateVolumeId] = useState<number | null>(null);
  const [offset, setOffset] = useState(0);
  const [navStack, setNavStack] = useState<NavigationSnapshot[]>([]);

  // Left volume rail & inline fast rename state (egui::SidePanel + TextEdit::singleline)
  const [showRail, setShowRail] = useState<boolean>(() =>
    typeof window === 'undefined' ? true : window.innerWidth >= 1024
  );
  const [showHelpPopover, setShowHelpPopover] = useState(false);
  const [diskFilterText, setDiskFilterText] = useState('');
  const [registerMode, setRegisterMode] = useState<RegisterFilterMode>('all');
  const [registerOrder, setRegisterOrder] = useState<RegisterOrder>('id');
  const [selectedRegisterDiskId, setSelectedRegisterDiskId] = useState<number>(1);
  const [categoriesOpen, setCategoriesOpen] = useState(false);

  // Fast inline disk rename (F2 / double-click / pencil icon)
  const [renamingDiskId, setRenamingDiskId] = useState<number | null>(null);
  const [renameDraft, setRenameDraft] = useState('');
  const [savingRename, setSavingRename] = useState(false);

  // Page results & selection
  const [pageData, setPageData] = useState<CatalogPageResponse | null>(null);
  const [selectedEntryId, setSelectedEntryId] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [queryError, setQueryError] = useState<string | null>(null);
  const [activeRequestId, setActiveRequestId] = useState<string | null>(null);

  // Page-only sorting
  const [sortCol, setSortCol] = useState<SortColumn>('size');
  const [sortDesc, setSortDesc] = useState(true);

  const searchInputRef = useRef<HTMLInputElement | null>(null);
  const diskFilterRef = useRef<HTMLInputElement | null>(null);
  const renameInputRef = useRef<HTMLInputElement | null>(null);
  const debounceTimerRef = useRef<number | null>(null);
  const abortControllerRef = useRef<AbortController | null>(null);
  const latestRequestIdRef = useRef<string | null>(null);
  const ancestorsRequestRef = useRef(0);
  const handledPresetNonceRef = useRef<number | null | undefined>(undefined);
  const rowRefs = useRef<Map<number, HTMLTableRowElement>>(new Map());
  const keyboardHandlerRef = useRef<((event: KeyboardEvent) => void) | null>(null);

  const composeEffectiveQuery = (text: string, typeTok: string, sizeTok: string): string => {
    const parts: string[] = [];
    const clean = text.trim();
    if (clean) parts.push(clean.includes(' OR ') ? `(${clean})` : clean);
    if (typeTok) parts.push(typeTok);
    if (sizeTok) parts.push(sizeTok);
    return parts.join(' AND ');
  };

  const runCatalogQuery = async (params: {
    nextMode?: 'search' | 'browse' | 'duplicates' | 'copy_candidates';
    nextDuplicateMode?: DuplicateCriterion;
    nextQuery?: string;
    nextVolumeId?: number | null;
    nextParentId?: number | null;
    nextFolderPrefix?: string | null;
    nextCategory?: string | null;
    nextSourceEntryId?: number | null;
    nextCandidateVolumeId?: number | null;
    nextOffset?: number;
    restoreEntryId?: number | null;
  }) => {
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    const controller = new AbortController();
    abortControllerRef.current = controller;

    const reqId = `req-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`;
    latestRequestIdRef.current = reqId;
    setActiveRequestId(reqId);
    setLoading(true);
    setQueryError(null);

    const m = params.nextMode ?? mode;
    const dMode = params.nextDuplicateMode ?? duplicateMode;
    const q = params.nextQuery !== undefined ? params.nextQuery : appliedQuery;
    const vid = params.nextVolumeId !== undefined ? params.nextVolumeId : scopedVolumeId;
    const pid = params.nextParentId !== undefined ? params.nextParentId : browseParentId;
    const fPref = params.nextFolderPrefix !== undefined ? params.nextFolderPrefix : folderPrefix;
    const cat = params.nextCategory !== undefined ? params.nextCategory : selectedCategory;
    const srcId = params.nextSourceEntryId !== undefined ? params.nextSourceEntryId : copySourceEntryId;
    const candVid = params.nextCandidateVolumeId !== undefined ? params.nextCandidateVolumeId : candidateVolumeId;
    const off = params.nextOffset !== undefined ? params.nextOffset : offset;

    try {
      const resp = await fetch('/api/catalog/query', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        signal: controller.signal,
        body: JSON.stringify({
          requestId: reqId,
          mode: m,
          query: q,
          volumeId: vid,
          parentId: pid,
          folderPrefix: fPref,
          category: cat,
          duplicateMode: dMode,
          sourceEntryId: srcId,
          candidateVolumeId: candVid,
          otherDisksOnly: true,
          offset: off,
          limit: 250,
        }),
      });
      if (latestRequestIdRef.current !== reqId) return;
      const data = await resp.json();
      if (latestRequestIdRef.current !== reqId) return;

      if (!resp.ok) {
        if (resp.status !== 499) {
          setPageData(null);
          setSelectedEntryId(null);
          setQueryError(data.error || t('Ошибка выполнения запроса', 'Query execution failed'));
        }
        return;
      }

      const typed = data as CatalogPageResponse;
      setPageData(typed);

      if (params.restoreEntryId && typed.items.some((i) => i.id === params.restoreEntryId)) {
        setSelectedEntryId(params.restoreEntryId);
      } else if (typed.items.length > 0) {
        setSelectedEntryId(typed.items[0].id);
        setSelectedRegisterDiskId(typed.items[0].volumeId);
      } else {
        setSelectedEntryId(null);
      }
      await onRefreshSnapshot();
    } catch (err) {
      if ((err as Error).name === 'AbortError') return;
      if (latestRequestIdRef.current === reqId) {
        setQueryError((err as Error).message);
      }
    } finally {
      if (latestRequestIdRef.current === reqId) {
        setLoading(false);
        setActiveRequestId(null);
      }
    }
  };

  // Handle cross-tab jump from Archive Slices, Sunburst, or Disks table
  useEffect(() => {
    ancestorsRequestRef.current++;
  }, [snapshot.pinnedGenerationSha]);

  useEffect(() => {
    if (!isActive) return;
    const nonce = presetJump?.nonce ?? null;
    if (handledPresetNonceRef.current === nonce) return;
    handledPresetNonceRef.current = nonce;
    if (!presetJump) {
      setCatalogOverview(true);
      return;
    }
    const jump = buildSearchJumpDefaults(presetJump);
    const emptyGlobalJump = jump.mode === 'search' && !jump.query.trim() && jump.volumeId === null && !jump.folderPrefix;
    const emptyFolderJump = jump.mode === 'browse' && jump.parentId !== null;
    const jumpMode = jump.mode;
    setQueryInput(jump.query);
    setActiveTypeChip('');
    setActiveSizeChip('');
    setAppliedQuery(jump.query);
    setMode(jumpMode);
    setCatalogOverview(emptyGlobalJump);
    setDuplicateMode(jump.duplicateMode);
    setScopedVolumeId(jump.volumeId);
    setBrowseParentId(jump.parentId);
    setBrowseCrumbs([]);
    setFolderPrefix(emptyFolderJump ? null : jump.folderPrefix);
    setSelectedCategory(jump.category);
    setCopySourceEntryId(jump.sourceEntryId);
    setCandidateVolumeId(jump.candidateVolumeId);
    setOffset(jump.offset);
    setNavStack([]);
    setPageData(null);
    setSelectedEntryId(null);
    setSelectedRegisterDiskId(jump.volumeId ?? 1);

    const ancestorRequest = ++ancestorsRequestRef.current;
    if (jump.parentId !== null && jump.volumeId !== null) {
      void fetch('/api/catalog/ancestors', {
        method: 'POST', headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ volumeId: jump.volumeId, parentId: jump.parentId }),
      }).then((response) => response.json()).then((data) => {
        if (ancestorRequest === ancestorsRequestRef.current) setBrowseCrumbs(Array.isArray(data.ancestors) ? data.ancestors : []);
      }).catch(() => {
        if (ancestorRequest === ancestorsRequestRef.current) setBrowseCrumbs([]);
      });
    }

    if (emptyGlobalJump) {
      setPageData(null);
      return;
    }
    void runCatalogQuery({
      nextMode: jumpMode,
      nextDuplicateMode: jump.duplicateMode,
      nextQuery: jump.query,
      nextVolumeId: jump.volumeId,
      nextParentId: jump.parentId,
      nextFolderPrefix: emptyFolderJump ? null : jump.folderPrefix,
      nextCategory: jump.category,
      nextSourceEntryId: jump.sourceEntryId,
      nextCandidateVolumeId: jump.candidateVolumeId,
      nextOffset: jump.offset,
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isActive, presetJump?.nonce]);

  const volumeMap = useMemo(() => {
    const m = new Map<number, CatalogVolume>();
    for (const v of snapshot.volumes) m.set(v.id, v);
    return m;
  }, [snapshot.volumes]);

  const sortedItems = useMemo(() => {
    if (!pageData) return [];
    if (mode === 'duplicates' || sortCol === 'natural') return pageData.items;
    const copy = [...pageData.items];
    copy.sort((a, b) => {
      let cmp = 0;
      if (sortCol === 'name') cmp = a.nameNorm.localeCompare(b.nameNorm);
      else if (sortCol === 'disk') cmp = a.volumeId - b.volumeId;
      else if (sortCol === 'duration') cmp = (a.videoMeta?.durationSec ?? 0) - (b.videoMeta?.durationSec ?? 0);
      else if (sortCol === 'size') cmp = a.size - b.size;
      else if (sortCol === 'modified') cmp = a.modifiedUnix - b.modifiedUnix;
      else if (sortCol === 'candidates') cmp = a.duplicateCount - b.duplicateCount;
      return sortDesc ? -cmp : cmp;
    });
    return copy;
  }, [pageData, mode, sortCol, sortDesc]);

  const maxPageBytes = useMemo(() => {
    let max = 0;
    for (const item of sortedItems) {
      if (item.size > max) max = item.size;
    }
    return max;
  }, [sortedItems]);

  const selectedEntry = useMemo(
    () => sortedItems.find((i) => i.id === selectedEntryId) ?? null,
    [sortedItems, selectedEntryId]
  );

  const activeDisk = useMemo(() => {
    const id = scopedVolumeId ?? selectedEntry?.volumeId ?? selectedRegisterDiskId;
    return volumeMap.get(id) ?? snapshot.volumes[0];
  }, [scopedVolumeId, selectedEntry, selectedRegisterDiskId, volumeMap, snapshot.volumes]);

  const startInlineRename = (vol: CatalogVolume) => {
    setShowRail(true);
    setRenamingDiskId(vol.id);
    setRenameDraft(vol.label);
    window.setTimeout(() => {
      renameInputRef.current?.focus();
      renameInputRef.current?.select();
    }, 20);
  };

  const commitInlineRename = async (volId: number) => {
    const nextLabel = renameDraft.trim();
    if (!nextLabel) {
      setRenamingDiskId(null);
      return;
    }
    setSavingRename(true);
    try {
      const resp = await fetch('/api/volumes/update', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ volumeId: volId, label: nextLabel }),
      });
      const data = await resp.json();
      if (!resp.ok || !data.ok) {
        onNotify(data.error || t('Не удалось переименовать диск', 'Failed to rename disk'), 'error');
        return;
      }
      setRenamingDiskId(null);
      await onRefreshSnapshot();
      void runCatalogQuery({ restoreEntryId: selectedEntryId });
      onNotify(
        t(`Диск #${volId} переименован в «${nextLabel}».`, `Disk #${volId} renamed to "${nextLabel}".`),
        'success'
      );
    } catch (err) {
      onNotify((err as Error).message, 'error');
    } finally {
      setSavingRename(false);
    }
  };

  // Global keyboard ergonomics (F2 -> Fast Disk Rename, Alt+Enter -> Disk Properties, / or Ctrl+K -> Search, ↑↓ -> Rows)
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      const tag = (e.target as HTMLElement | null)?.tagName;
      const inInput = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';

      if (!inInput && e.key === 'F2' && activeDisk) {
        e.preventDefault();
        startInlineRename(activeDisk);
      } else if (!inInput && e.altKey && e.key === 'Enter' && activeDisk) {
        e.preventDefault();
        onOpenDiskProperties(activeDisk.id);
      } else if (
        ((e.metaKey || e.ctrlKey) && (e.key.toLowerCase() === 'f' || e.key.toLowerCase() === 'k')) ||
        (!inInput && e.key === '/')
      ) {
        e.preventDefault();
        searchInputRef.current?.focus();
        searchInputRef.current?.select();
      } else if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === 'l' && !e.shiftKey) {
        e.preventDefault();
        setShowRail(true);
        window.setTimeout(() => diskFilterRef.current?.focus(), 20);
      } else if ((e.metaKey || e.ctrlKey) && e.shiftKey && e.key.toLowerCase() === 'x') {
        e.preventDefault();
        void handleAbortServerQuery();
      } else if (e.key === 'Escape') {
        if (loading) {
          e.preventDefault();
          void handleAbortServerQuery();
        } else if (showHelpPopover) {
          setShowHelpPopover(false);
        }
      } else if (e.altKey && e.key === 'ArrowLeft' && navStack.length > 0) {
        e.preventDefault();
        handleBack();
      } else if (!inInput && sortedItems.length > 0) {
        if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
          e.preventDefault();
          const curIdx = sortedItems.findIndex((i) => i.id === selectedEntryId);
          const nextIdx =
            curIdx === -1
              ? 0
              : e.key === 'ArrowDown'
                ? Math.min(sortedItems.length - 1, curIdx + 1)
                : Math.max(0, curIdx - 1);
          const nextRow = sortedItems[nextIdx];
          if (nextRow) {
            setSelectedEntryId(nextRow.id);
            setSelectedRegisterDiskId(nextRow.volumeId);
            rowRefs.current.get(nextRow.id)?.scrollIntoView({ block: 'nearest' });
          }
        } else if (e.key === 'Enter' && selectedEntry) {
          e.preventDefault();
          if (selectedEntry.kind === 'directory') {
            handleOpenFolder(selectedEntry);
          } else if (selectedEntry.duplicateCount > 1) {
            handleOpenCopyCandidates(selectedEntry);
          }
        }
      }
    };
    keyboardHandlerRef.current = onKeyDown;
  });

  useEffect(() => {
    if (!isActive) return;
    const onKeyDown = (event: KeyboardEvent) => keyboardHandlerRef.current?.(event);
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [isActive]);

  const pushHistory = (restoreEntryId: number | null = selectedEntryId) => {
    setNavStack((prev) => [
      ...prev.slice(-15),
      {
        mode,
        duplicateMode,
        query: appliedQuery,
        queryInput,
        activeTypeChip,
        activeSizeChip,
        volumeId: scopedVolumeId,
        parentId: browseParentId,
        folderPrefix,
        category: selectedCategory,
        sourceEntryId: copySourceEntryId,
        candidateVolumeId,
        selectedEntryId: restoreEntryId,
        offset,
        crumbs: browseCrumbs,
        catalogOverview,
      },
    ]);
  };

  const handleQueryInputChange = (val: string) => {
    setQueryInput(val);
    if (debounceTimerRef.current) window.clearTimeout(debounceTimerRef.current);
  };

  const handleSearchSubmit = (e?: React.FormEvent) => {
    e?.preventDefault();
    if (debounceTimerRef.current) window.clearTimeout(debounceTimerRef.current);
    const composed = composeEffectiveQuery(queryInput, activeTypeChip, activeSizeChip);
    if (!composed.trim()) {
      setAppliedQuery('');
      setCopySourceEntryId(null);
      setOffset(0);
      const destination = destinationForEmptyQuery(scopedVolumeId, browseParentId);
      if (destination.view === 'browse') {
        setCatalogOverview(false);
        setMode('browse');
        setFolderPrefix(null);
        void runCatalogQuery({ nextMode: 'browse', nextVolumeId: destination.volumeId, nextParentId: destination.parentId, nextFolderPrefix: null, nextQuery: '', nextOffset: 0 });
      } else {
        setCatalogOverview(true);
        setPageData(null);
      }
      return;
    }
    const nextM = mode === 'duplicates' ? 'duplicates' : 'search';
    setCatalogOverview(false);
    setMode(nextM);
    setCopySourceEntryId(null);
    setAppliedQuery(composed);
    setOffset(0);
    void runCatalogQuery({
      nextMode: nextM,
      nextQuery: composed,
      nextSourceEntryId: null,
      nextOffset: 0,
    });
  };

  const applyFilterChips = (nextType: string, nextSize: string) => {
    setActiveTypeChip(nextType);
    setActiveSizeChip(nextSize);
    const composed = composeEffectiveQuery(queryInput, nextType, nextSize);
    const nextM = mode === 'duplicates' ? 'duplicates' : 'search';
    setCatalogOverview(false);
    setMode(nextM);
    setCopySourceEntryId(null);
    setAppliedQuery(composed);
    setOffset(0);
    void runCatalogQuery({
      nextMode: nextM,
      nextQuery: composed,
      nextSourceEntryId: null,
      nextOffset: 0,
    });
  };

  const handleSelectScopeDisk = (targetVolId: number | null) => {
    ancestorsRequestRef.current++;
    pushHistory();
    setScopedVolumeId(targetVolId);
    setCatalogOverview(targetVolId === null);
    if (targetVolId !== null) setSelectedRegisterDiskId(targetVolId);
    setFolderPrefix(null);
    setBrowseParentId(null);
    setBrowseCrumbs([]);
    setSelectedCategory(null);
    if (targetVolId === null) setRegisterMode('all');
    setAppliedQuery('');
    setQueryInput('');
    setActiveTypeChip('');
    setActiveSizeChip('');
    if (targetVolId !== null) setMode('browse');
    else setMode('search');
    setCopySourceEntryId(null);
    setOffset(0);
    if (targetVolId === null) {
      setPageData(null);
      setSelectedEntryId(null);
      return;
    }
    void runCatalogQuery({
      nextMode: targetVolId !== null ? 'browse' : 'search',
      nextVolumeId: targetVolId,
      nextParentId: null,
      nextQuery: '',
      nextFolderPrefix: null,
      nextSourceEntryId: null,
      nextOffset: 0,
    });
  };

  const handleOpenCopyCandidates = (entry: CatalogEntry) => {
    if (entry.kind !== 'file') return;
    pushHistory();
    setMode('copy_candidates');
    setCopySourceEntryId(entry.id);
    setCandidateVolumeId(null);
    setOffset(0);
    void runCatalogQuery({
      nextMode: 'copy_candidates',
      nextSourceEntryId: entry.id,
      nextCandidateVolumeId: null,
      nextOffset: 0,
    });
  };

  const handleBrowseParentFolder = (entry: CatalogEntry) => {
    pushHistory();
    setMode('browse');
    setCatalogOverview(false);
    setScopedVolumeId(entry.volumeId);
    setBrowseParentId(entry.parentId);
    setFolderPrefix(null);
    setCopySourceEntryId(null);
    setQueryInput('');
    setAppliedQuery('');
    setActiveTypeChip('');
    setActiveSizeChip('');
    setSelectedCategory(null);
    setOffset(0);
    void runCatalogQuery({
      nextMode: 'browse',
      nextVolumeId: entry.volumeId,
      nextParentId: entry.parentId,
      nextFolderPrefix: null,
      nextQuery: '',
      nextCategory: null,
      nextSourceEntryId: null,
      nextOffset: 0,
      restoreEntryId: entry.id,
    });
    const request = ++ancestorsRequestRef.current;
    void fetch('/api/catalog/ancestors', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ volumeId: entry.volumeId, parentId: entry.parentId }),
    }).then((response) => response.json()).then((data) => {
      if (request === ancestorsRequestRef.current) setBrowseCrumbs(Array.isArray(data.ancestors) ? data.ancestors : []);
    }).catch(() => {
      if (request === ancestorsRequestRef.current) setBrowseCrumbs([]);
    });
  };

  const handleUp = () => {
    if (scopedVolumeId === null || browseParentId === null || browseCrumbs.length === 0) return;
    ancestorsRequestRef.current++;
    pushHistory();
    const nextCrumbs = browseCrumbs.slice(0, -1);
    const nextParentId = nextCrumbs.length ? nextCrumbs[nextCrumbs.length - 1].id : null;
    setBrowseCrumbs(nextCrumbs);
    setBrowseParentId(nextParentId);
    setMode('browse');
    setCatalogOverview(false);
    setQueryInput('');
    setAppliedQuery('');
    setActiveTypeChip('');
    setActiveSizeChip('');
    setSelectedCategory(null);
    setOffset(0);
    void runCatalogQuery({ nextMode: 'browse', nextVolumeId: scopedVolumeId, nextParentId, nextFolderPrefix: null, nextQuery: '', nextOffset: 0 });
  };

  const handleOpenBreadcrumb = (crumbIndex: number) => {
    if (scopedVolumeId === null) return;
    ancestorsRequestRef.current++;
    pushHistory();
    const target = breadcrumbPath(browseCrumbs, crumbIndex);
    const nextCrumbs = target.crumbs;
    setBrowseCrumbs(nextCrumbs);
    setBrowseParentId(target.parentId);
    setMode('browse');
    setCatalogOverview(false);
    setFolderPrefix(null);
    setQueryInput('');
    setAppliedQuery('');
    setActiveTypeChip('');
    setActiveSizeChip('');
    setSelectedCategory(null);
    setOffset(0);
    void runCatalogQuery({ nextMode: 'browse', nextVolumeId: scopedVolumeId, nextParentId: target.parentId, nextFolderPrefix: null, nextQuery: '', nextOffset: 0 });
  };

  const handleOpenFolder = (folder: FolderIdentity) => {
    pushHistory(folder.id);
    setCatalogOverview(false);
    setMode('browse');
    setScopedVolumeId(folder.volumeId);
    setBrowseParentId(folder.id);
    setFolderPrefix(null);
    setCopySourceEntryId(null);
    setQueryInput('');
    setAppliedQuery('');
    setActiveTypeChip('');
    setActiveSizeChip('');
    setSelectedCategory(null);
    setOffset(0);
    const request = ++ancestorsRequestRef.current;
    void fetch('/api/catalog/ancestors', {
      method: 'POST', headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ volumeId: folder.volumeId, parentId: folder.id }),
    }).then((response) => response.json()).then((data) => { if (request === ancestorsRequestRef.current) setBrowseCrumbs(Array.isArray(data.ancestors) ? data.ancestors : []); });
    void runCatalogQuery({ nextMode: 'browse', nextVolumeId: folder.volumeId, nextParentId: folder.id, nextFolderPrefix: null, nextQuery: '', nextSourceEntryId: null, nextOffset: 0 });
  };

  const handleBack = () => {
    const prev = navStack[navStack.length - 1];
    if (!prev) return;
    ancestorsRequestRef.current++;
    setNavStack((s) => s.slice(0, -1));
    setMode(prev.mode);
    setCatalogOverview(prev.catalogOverview);
    setDuplicateMode(prev.duplicateMode);
    setAppliedQuery(prev.query);
    setQueryInput(prev.queryInput);
    setActiveTypeChip(prev.activeTypeChip);
    setActiveSizeChip(prev.activeSizeChip);
    setScopedVolumeId(prev.volumeId);
    setBrowseParentId(prev.parentId);
    setBrowseCrumbs(prev.crumbs);
    setFolderPrefix(prev.folderPrefix);
    setSelectedCategory(prev.category);
    setCopySourceEntryId(prev.sourceEntryId);
    setCandidateVolumeId(prev.candidateVolumeId);
    setOffset(prev.offset);
    if (prev.catalogOverview) {
      setPageData(null);
      setSelectedEntryId(null);
      return;
    }
    void runCatalogQuery({
      nextMode: prev.mode,
      nextDuplicateMode: prev.duplicateMode,
      nextQuery: prev.query,
      nextVolumeId: prev.volumeId,
      nextParentId: prev.parentId,
      nextFolderPrefix: prev.folderPrefix,
      nextCategory: prev.category,
      nextSourceEntryId: prev.sourceEntryId,
      nextCandidateVolumeId: prev.candidateVolumeId,
      nextOffset: prev.offset,
      restoreEntryId: prev.selectedEntryId,
    });
  };

  const handleAbortServerQuery = async () => {
    if (!loading) return;
    const reqToCancel = activeRequestId;
    if (abortControllerRef.current) {
      abortControllerRef.current.abort();
    }
    setLoading(false);
    setActiveRequestId(null);
    await fetch('/api/catalog/cancel', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ requestId: reqToCancel }),
    });
    onNotify(t('Поисковый запрос остановлен.', 'Search query cancelled.'), 'info');
    await onRefreshSnapshot();
  };

  const displayedVolumes = useMemo(() => {
    const needle = diskFilterText.trim().toLowerCase();
    const candidateGroupSet = new Set(
      pageData?.copyCandidatesMeta?.diskGroups.map((g) => g.volumeId) ?? []
    );

    const list = snapshot.volumes.filter((v) => {
      if (registerMode === 'current_page' && !pageData?.currentPageDiskCounts[v.id]) return false;
      if (registerMode === 'copy_candidates' && !candidateGroupSet.has(v.id)) return false;

      if (!needle) return true;
      return (
        `#${v.id}`.includes(needle) ||
        String(v.id) === needle ||
        v.diskCode.toLowerCase().includes(needle) ||
        v.label.toLowerCase().includes(needle) ||
        v.note.toLowerCase().includes(needle)
      );
    });

    list.sort((a, b) => {
      if (registerOrder === 'volume') return b.indexedBytes - a.indexedBytes || a.id - b.id;
      return a.id - b.id;
    });
    return list;
  }, [snapshot.volumes, diskFilterText, registerMode, registerOrder, pageData]);

  const copyWithFreshPinCheck = async (kind: 'pickup_card' | 'full_register') => {
    const resp = await fetch('/api/catalog/verify-pin', { method: 'POST' });
    const data = await resp.json();
    if (!resp.ok || !data.ok) {
      onNotify(data.error || t('Ошибка проверки снимка каталога', 'Catalog snapshot check failed'), 'error');
      return;
    }
    const text =
      kind === 'pickup_card'
        ? buildPickupCardText(activeDisk, selectedEntry, data.verifiedGenerationSha, lang)
        : buildFullDiskRegisterText(data.volumes as CatalogVolume[], data.verifiedGenerationSha, lang);
    await navigator.clipboard.writeText(text);
    onNotify(
      kind === 'pickup_card'
        ? t(
            `Карточка диска #${activeDisk.id} (${activeDisk.label}) скопирована.`,
            `Card for Disk #${activeDisk.id} (${activeDisk.label}) copied.`
          )
        : t(
            `Реестр (${snapshot.volumes.length} дисков) скопирован в буфер обмена.`,
            `Register (${snapshot.volumes.length} disks) copied to clipboard.`
          ),
      'success'
    );
  };

  const handleHeaderSortClick = (col: SortColumn) => {
    if (sortCol === col) {
      if (sortDesc) setSortDesc(false);
      else {
        setSortCol('natural');
        setSortDesc(false);
      }
    } else {
      setSortCol(col);
      setSortDesc(col === 'size' || col === 'duration' || col === 'candidates' || col === 'modified');
    }
  };

  const hasActiveScopeOrFilters =
    scopedVolumeId !== null ||
    folderPrefix !== null ||
    selectedCategory !== null ||
    mode !== 'search' ||
    appliedQuery.trim().length > 0;

  return (
    <div className="flex flex-col border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 overflow-hidden">
      {/* Compact egui::TopBottomPanel Command Header */}
      <div className="border-b border-slate-200 dark:border-slate-800 bg-slate-50/90 dark:bg-slate-900/70 px-3 py-2 flex flex-col gap-1.5 relative">
        <form onSubmit={handleSearchSubmit} className="flex flex-wrap items-center gap-1.5">
          <button
            type="button"
            onClick={() => setShowRail((v) => !v)}
            title={t('Показать/скрыть панель дисков (⌘L)', 'Toggle disks panel (⌘L)')}
            className={`inline-flex items-center gap-1 px-2.5 py-1.5 text-xs font-medium rounded border transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
              showRail
                ? 'border-blue-500/60 bg-blue-50 dark:bg-blue-950/40 text-blue-700 dark:text-blue-300'
                : 'border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            <PanelLeft className="w-3.5 h-3.5" />
            <span className="hidden sm:inline">{t('Диски', 'Disks')}</span>
          </button>

          {navStack.length > 0 && (
            <button
              type="button"
              onClick={handleBack}
              title={t('Назад (Alt+←)', 'Back (Alt+←)')}
              className="inline-flex items-center gap-1 px-2 py-1.5 text-xs font-medium rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 cursor-pointer"
            >
              <ArrowLeft className="w-3.5 h-3.5" />
              {t('Назад', 'Back')}
            </button>
          )}

          <div className="relative flex-1 min-w-[200px]">
            <Search className="w-3.5 h-3.5 text-slate-400 absolute left-2.5 top-1/2 -translate-y-1/2 pointer-events-none" />
            <input
              ref={searchInputRef}
              type="text"
              value={queryInput}
              onChange={(e) => handleQueryInputChange(e.target.value)}
              placeholder={t(
                'Поиск видео по 60 дискам: название, res:2160p, codec:hevc, dur:>120m (/ или Ctrl+K)…',
                'Search video across 60 USBs: title, res:2160p, codec:hevc, dur:>120m (/ or Ctrl+K)…'
              )}
              aria-label={t('Поисковый запрос по видеокаталогу', 'Video catalog search query')}
              className="w-full pl-8 pr-20 py-1.5 text-xs sm:text-sm rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
            />
            <div className="absolute right-1.5 top-1/2 -translate-y-1/2 flex items-center gap-1">
              {loading && (
                <button
                  type="button"
                  onClick={() => void handleAbortServerQuery()}
                  className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-red-600 text-white text-[11px] font-medium cursor-pointer"
                >
                  <Square className="w-2.5 h-2.5" />
                  {t('Стоп', 'Stop')}
                </button>
              )}
              {queryInput && !loading && (
                <button
                  type="button"
                  onClick={() => {
                    setQueryInput('');
                    handleQueryInputChange('');
                    searchInputRef.current?.focus();
                  }}
                  aria-label={t('Очистить поиск', 'Clear search')}
                  className="p-1 text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 cursor-pointer"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              )}
              <button
                type="button"
                onClick={() => setShowHelpPopover((v) => !v)}
                title={t('Синтаксис видеопоиска и горячие клавиши', 'Video search syntax & shortcuts')}
                className={`p-1 rounded cursor-pointer ${
                  showHelpPopover
                    ? 'text-blue-600 dark:text-blue-400'
                    : 'text-slate-400 hover:text-slate-700 dark:hover:text-slate-200'
                }`}
              >
                <HelpCircle className="w-3.5 h-3.5" />
              </button>
            </div>
          </div>

          <button
            type="button"
            onClick={() => {
              pushHistory();
              const nextMode = mode === 'duplicates' ? 'search' : 'duplicates';
              setMode(nextMode);
              setCopySourceEntryId(null);
              setOffset(0);
              void runCatalogQuery({
                nextMode,
                nextSourceEntryId: null,
                nextOffset: 0,
              });
            }}
            className={`px-2.5 py-1.5 text-xs font-semibold rounded border transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
              mode === 'duplicates'
                ? 'border-blue-600 bg-blue-600 text-white'
                : 'border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            {t('Дубликаты видео', 'Video Duplicates')}
          </button>

          <button
            type="button"
            onClick={() => onOpenSlices(scopedVolumeId)}
            className="inline-flex items-center gap-1 px-2.5 py-1.5 text-xs font-medium rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 cursor-pointer"
          >
            <BarChart3 className="w-3.5 h-3.5 text-teal-500" />
            {t('Разрезы', 'Slices')}
          </button>

          <button
            type="button"
            onClick={() => onOpenSunburst(scopedVolumeId, mode === 'browse' ? browseCrumbs.map((crumb) => crumb.name).join('/') || null : folderPrefix)}
            className="inline-flex items-center gap-1 px-2.5 py-1.5 text-xs font-medium rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 cursor-pointer"
          >
            <PieChart className="w-3.5 h-3.5 text-blue-500" />
            {t('Карта', 'Map')}
          </button>
        </form>

        {/* Syntax & Desktop Shortcuts Popover */}
        {showHelpPopover && (
          <div className="p-3 rounded border border-slate-200 dark:border-slate-700 bg-white dark:bg-slate-950 shadow-md text-xs flex flex-col gap-2">
            <div className="flex items-center justify-between">
              <span className="font-semibold text-slate-900 dark:text-white">
                {t('Операторы поиска по видеоархиву и клавиши (egui)', 'Video Archive Operators & Desktop Shortcuts')}
              </span>
              <button
                type="button"
                onClick={() => setShowHelpPopover(false)}
                className="text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 cursor-pointer"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {[
                'res:2160p',
                'res:1080p',
                'codec:hevc',
                'dur:>150m',
                'size:>40gb',
                'ext:m2ts',
                'dup:>1',
                'Солярис',
                'Blade OR Dune',
              ].map((sample) => (
                <button
                  key={sample}
                  type="button"
                  onClick={() => {
                    setQueryInput(sample);
                    handleQueryInputChange(sample);
                    setShowHelpPopover(false);
                  }}
                  className="px-2 py-0.5 rounded border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900 font-mono text-[11px] text-slate-700 dark:text-slate-300 hover:border-blue-500 cursor-pointer"
                >
                  {sample}
                </button>
              ))}
            </div>
            <div className="pt-1.5 border-t border-slate-200 dark:border-slate-800 text-[11px] text-slate-500">
              {t(
                'Клавиши: F2 (быстро переименовать выбранный диск) · Alt+Enter (Свойства диска) · Ctrl+N (Окно сканирования) · / или Ctrl+K (поиск) · ↑↓ + Enter (навигация)',
                'Shortcuts: F2 (fast rename disk) · Alt+Enter (Disk Properties) · Ctrl+N (Scan Disk Window) · / or Ctrl+K (search) · ↑↓ + Enter (rows)'
              )}
            </div>
          </div>
        )}

        {/* 4-Mode Video Duplicate Detection Bar */}
        {mode === 'duplicates' && (
          <div className="p-2 rounded border border-blue-500/35 bg-blue-50/55 dark:bg-blue-950/25 flex flex-col gap-2 text-xs">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="flex flex-wrap items-center gap-1">
                {(
                  [
                    [
                      'video_rips',
                      t('1. Разные рипы фильма (4K vs 1080p vs 720p)', '1. Multi-Quality Rips (4K vs 1080p vs 720p)'),
                    ],
                    [
                      'video_stream',
                      t('2. Похожие потоки (длит.+разр.+кодек)', '2. Similar stream metadata'),
                    ],
                    [
                      'exact_bytes',
                      t('3. Кандидаты: совпадает размер (не подтверждено)', '3. Same-size candidates (unverified)'),
                    ],
                    [
                      'same_name',
                      t('4. Одноимённые разного размера', '4. Same Filename, Diff Size'),
                    ],
                  ] as const
                ).map(([crit, label]) => (
                  <button
                    key={crit}
                    type="button"
                    onClick={() => {
                      setDuplicateMode(crit);
                      setOffset(0);
                      void runCatalogQuery({
                        nextMode: 'duplicates',
                        nextDuplicateMode: crit,
                        nextOffset: 0,
                      });
                    }}
                    className={`px-2 py-1 rounded font-medium cursor-pointer transition-colors whitespace-nowrap ${
                      duplicateMode === crit
                        ? 'bg-blue-600 text-white'
                        : 'bg-white dark:bg-slate-900 border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800'
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>

              <div className="flex flex-wrap items-center gap-3 font-mono tabular-nums">
                {pageData?.duplicateSummary && (
                  <>
                    <span className="text-slate-700 dark:text-slate-300">
                      {t('Групп:', 'Groups:')} <strong>{pageData.duplicateSummary.groupCount}</strong>
                    </span>
                    {pageData.duplicateSummary.reclaimableBytes > 0 && (
                      <span className="text-blue-700 dark:text-blue-300 font-semibold">
                        {t('Условный избыток:', 'Potential excess:')} +{formatBytes(pageData.duplicateSummary.reclaimableBytes)}
                      </span>
                    )}
                  </>
                )}
                <button
                  type="button"
                  onClick={() => setShowDupExplainer((v) => !v)}
                  className="inline-flex items-center gap-1 font-sans text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
                >
                  <Info className="w-3.5 h-3.5" />
                  {t('Как ищет без тяжёлой БД?', 'How it works without DB bloat?')}
                </button>
              </div>
            </div>

            {showDupExplainer && (
              <div className="p-2.5 rounded border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950 text-[11px] leading-relaxed text-slate-700 dark:text-slate-300 grid grid-cols-1 md:grid-cols-3 gap-2.5">
                <div>
                  <div className="font-semibold text-slate-900 dark:text-white">
                    {t('Плагин «Видео-заголовки Light» (+16 байт/файл)', 'Video Header Light (+16 B/file)')}
                  </div>
                  <p className="mt-0.5 text-slate-500 dark:text-slate-400">
                    {t(
                      'В отличие от тяжёлых плагинов WhereIsIt (эскизы кадров), читаются только первые 16–64 КБ контейнера MKV/MP4/M2TS (+2.5 сек на 5 ТБ). На 100 000 видео база растёт всего на 1.6 МБ!',
                      'Unlike heavy WhereIsIt thumbnail plugins, reads only 16–64 KB of MKV/MP4/M2TS header (+2.5s per 5 TB). Adds just 1.6 MB per 100,000 videos!'
                    )}
                  </p>
                </div>
                <div>
                  <div className="font-semibold text-slate-900 dark:text-white">
                    {t('Ремуксы и разные озвучки', 'Remuxes & Audio Tracks')}
                  </div>
                  <p className="mt-0.5 text-slate-500 dark:text-slate-400">
                    {t(
                      'У ремукса с новой звуковой дорожкой меняется размер и SHA-256, но длительность (±1 сек), разрешение и видеокодек остаются прежними — режим №2 находит их мгновенно.',
                      'Adding an audio track changes file size and SHA-256, while duration (±1s), resolution & video codec stay identical — Mode #2 catches them instantly.'
                    )}
                  </p>
                </div>
                <div>
                  <div className="font-semibold text-slate-900 dark:text-white">
                    {t('Апгрейд качества (4K vs 1080p vs 720p)', 'Quality Upgrades (4K vs 1080p vs 720p)')}
                  </div>
                  <p className="mt-0.5 text-slate-500 dark:text-slate-400">
                    {t(
                      'Когда фильм обновлён до 2160p UHD на новом диске, а старый 1080p/720p рип забыт на старом USB — режим №1 сводит все разрешения в одну группу и считает выгоду очистки.',
                      'When you save a new 2160p UHD master while an older 1080p/720p rip sits on another USB, Mode #1 groups them from 4K down to 720p.'
                    )}
                  </p>
                </div>
              </div>
            )}
          </div>
        )}

        {/* Row 2: Video Resolution/Codec Quick Chips + Size Chips */}
        <div className="flex flex-wrap items-center justify-between gap-2 text-xs">
          <div className="flex flex-wrap items-center gap-1.5">
            <div className="inline-flex items-center p-0.5 rounded bg-slate-200/75 dark:bg-slate-800/90 overflow-x-auto max-w-full">
              {quickVideoChips.map((chip) => (
                <button
                  key={chip.token || 'all'}
                  type="button"
                  onClick={() => applyFilterChips(chip.token, activeSizeChip)}
                  className={`px-2 py-0.5 rounded transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                    activeTypeChip === chip.token
                      ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white font-semibold shadow-xs'
                      : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                  }`}
                >
                  {chip.label}
                </button>
              ))}
            </div>

            <div className="inline-flex items-center p-0.5 rounded bg-slate-200/75 dark:bg-slate-800/90">
              {quickSizeChips.map((chip) => (
                <button
                  key={chip.token || 'any'}
                  type="button"
                  onClick={() => applyFilterChips(activeTypeChip, chip.token)}
                  className={`px-2 py-0.5 rounded font-mono text-[11px] tabular-nums transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                    activeSizeChip === chip.token
                      ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white font-semibold shadow-xs'
                      : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                  }`}
                >
                  {chip.label}
                </button>
              ))}
            </div>
          </div>

          <div className="flex items-center gap-3 text-slate-500 dark:text-slate-400 shrink-0 text-[11px]">
            <button
              type="button"
              onClick={() => void copyWithFreshPinCheck('full_register')}
              className="hover:text-slate-900 dark:hover:text-white cursor-pointer whitespace-nowrap"
            >
              {t('Копировать реестр 60 дисков', 'Copy 60-Disk Register')}
            </button>
          </div>
        </div>

        {/* Active Scope Breadcrumb Strip */}
        <div className="flex flex-wrap items-center gap-1.5 text-[11px] pt-1 border-t border-slate-200/70 dark:border-slate-800/70 text-slate-600 dark:text-slate-400">
          <button type="button" onClick={() => handleSelectScopeDisk(null)} className="text-blue-600 dark:text-blue-400 hover:underline cursor-pointer">{t('Каталог', 'Catalog')}</button>
          {scopedVolumeId !== null && <>
            <span>›</span>
                <button type="button" onClick={() => handleOpenBreadcrumb(-1)} className="text-blue-600 dark:text-blue-400 hover:underline cursor-pointer">#{scopedVolumeId} {volumeMap.get(scopedVolumeId)?.label}</button>
            {browseCrumbs.map((crumb, index) => <React.Fragment key={crumb.id}><span>›</span><button type="button" onClick={() => handleOpenBreadcrumb(index)} className={`hover:underline cursor-pointer ${index === browseCrumbs.length - 1 ? 'font-semibold text-slate-900 dark:text-white' : 'text-blue-600 dark:text-blue-400'}`}>{crumb.name}</button></React.Fragment>)}
            <button type="button" disabled={browseParentId === null || browseCrumbs.length === 0} onClick={handleUp} className="ml-auto px-2 py-0.5 rounded border border-slate-300 dark:border-slate-700 disabled:opacity-40 cursor-pointer">↑ {t('Вверх', 'Up')}</button>
          </>}
        </div>
        {hasActiveScopeOrFilters && mode !== 'browse' && (
          <div className="flex flex-wrap items-center gap-2 text-[11px] pt-1 border-t border-slate-200/70 dark:border-slate-800/70 text-slate-600 dark:text-slate-400">
            <span className="font-medium text-slate-700 dark:text-slate-300">{t('Охват:', 'Scope:')}</span>
            {scopedVolumeId !== null && (
              <button
                type="button"
                onClick={() => handleSelectScopeDisk(null)}
                className="text-blue-600 dark:text-blue-400 hover:underline font-mono cursor-pointer"
              >
                USB #{scopedVolumeId} ({volumeMap.get(scopedVolumeId)?.label}) ×
              </button>
            )}
            {folderPrefix && (
              <button
                type="button"
                onClick={() => {
                  setFolderPrefix(null);
                  void runCatalogQuery({ nextFolderPrefix: null, nextOffset: 0 });
                }}
                className="text-blue-600 dark:text-blue-400 hover:underline font-mono cursor-pointer"
              >
                /{folderPrefix} ×
              </button>
            )}
            {selectedCategory && (
              <button
                type="button"
                onClick={() => {
                  setSelectedCategory(null);
                  void runCatalogQuery({ nextCategory: null, nextOffset: 0 });
                }}
                className="text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
              >
                {translateCategory(selectedCategory)} ×
              </button>
            )}
            <button
              type="button"
              onClick={() => {
                setQueryInput('');
                setActiveTypeChip('');
                setActiveSizeChip('');
                setScopedVolumeId(null);
                setBrowseParentId(null);
                setBrowseCrumbs([]);
                setFolderPrefix(null);
                setSelectedCategory(null);
                setCopySourceEntryId(null);
                setMode('search');
                setAppliedQuery('');
                setOffset(0);
                setCatalogOverview(true);
                setPageData(null);
              }}
              className="ml-auto text-slate-500 hover:text-slate-900 dark:hover:text-white underline cursor-pointer"
            >
              {t('Показать все 60 дисков', 'Reset to all 60 disks')}
            </button>
          </div>
        )}
      </div>

      {/* Main Desktop Split: Left Disk Panel (egui::SidePanel) + Central Table (egui_extras::TableBuilder) */}
      <div className="flex min-h-[540px] lg:h-[calc(100vh-168px)] max-h-[820px] divide-x divide-slate-200 dark:divide-slate-800">
        {showRail && (
          <aside className="w-[238px] xl:w-[252px] shrink-0 flex flex-col bg-slate-50/40 dark:bg-slate-900/30 select-none">
            <div className="p-2 border-b border-slate-200 dark:border-slate-800 flex flex-col gap-1.5">
              <div className="flex items-center justify-between gap-1 text-[11px]">
                <span className="font-semibold text-slate-800 dark:text-slate-200">
                  {t('USB-диски', 'USB Drives')} ({displayedVolumes.length}/{snapshot.volumes.length})
                </span>
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => onOpenScanModal(null)}
                    title={t('Сканировать новый USB-диск (Ctrl+N)', 'Scan new USB drive (Ctrl+N)')}
                    className="text-[11px] font-medium text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
                  >
                    + {t('Скан', 'Scan')}
                  </button>
                  <button
                    type="button"
                    onClick={() => setRegisterOrder((prev) => (prev === 'id' ? 'volume' : 'id'))}
                    className="text-[11px] text-slate-500 hover:text-slate-900 dark:hover:text-white cursor-pointer"
                  >
                    {registerOrder === 'id' ? '#ID' : t('Объём↓', 'Size↓')}
                  </button>
                </div>
              </div>

              <input
                ref={diskFilterRef}
                type="text"
                value={diskFilterText}
                onChange={(e) => setDiskFilterText(e.target.value)}
                placeholder={t('Фильтр #, USB-04, имя…', 'Filter #, USB-04, name…')}
                aria-label={t('Фильтр носителей', 'Filter disks')}
                className="w-full px-2 py-1 text-xs rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-1 focus:ring-blue-500"
              />

              <div className="grid grid-cols-3 gap-0.5 p-0.5 rounded bg-slate-200/70 dark:bg-slate-800 text-[11px]">
                {(
                  [
                    ['all', t('Все', 'All')],
                    ['current_page', t('На стр.', 'Hits')],
                    ['copy_candidates', t('Копии', 'Copies')],
                  ] as const
                ).map(([key, label]) => (
                  <button
                    key={key}
                    type="button"
                    onClick={() => setRegisterMode(key)}
                    className={`py-0.5 rounded font-medium text-center transition-colors whitespace-nowrap cursor-pointer ${
                      registerMode === key
                        ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white shadow-xs'
                        : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                    }`}
                  >
                    {label}
                  </button>
                ))}
              </div>
            </div>

            {/* Pinned Root Row: All 60 Disks (classic WhereIsIt Catalog Root) */}
            <button
              type="button"
              onClick={() => handleSelectScopeDisk(null)}
              className={`px-2.5 py-1.5 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between text-xs font-semibold transition-colors cursor-pointer ${
                scopedVolumeId === null
                  ? 'bg-blue-600 text-white'
                  : 'bg-white dark:bg-slate-950 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-900'
              }`}
            >
              <span>
                {t('Все диски каталога', 'All Catalog Disks')} ({snapshot.volumes.length})
              </span>
              <span className="font-mono text-[11px] tabular-nums">
                {formatBytes(snapshot.totalIndexedBytes)}
              </span>
            </button>

            <div className="flex-1 min-h-0 overflow-y-auto">
              <CatalogNavigationTree
                key={snapshot.pinnedGenerationSha}
                volumes={displayedVolumes}
                activeVolumeId={scopedVolumeId}
                activeParentId={browseParentId}
                activeCrumbIds={browseCrumbs.map((crumb) => crumb.id)}
                browseActive={mode === 'browse'}
                focusedAncestors={browseCrumbs}
                onOpenVolume={(volume) => handleSelectScopeDisk(volume.id)}
                onOpenFolder={handleOpenFolder}
              />
            </div>

            {/* Collapsible Categories */}
            <div className="border-t border-slate-200 dark:border-slate-800">
              <button
                type="button"
                onClick={() => setCategoriesOpen((v) => !v)}
                className="w-full px-2.5 py-1.5 flex items-center justify-between text-xs font-medium text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-900 cursor-pointer"
              >
                <span>
                  {t('Разделы видеоархива', 'Archive Sections')} ({snapshot.categories.length})
                </span>
                {categoriesOpen ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
              </button>
              {categoriesOpen && (
                <div className="px-1.5 pb-1.5 max-h-36 overflow-y-auto flex flex-col gap-0.5">
                  <button
                    type="button"
                    onClick={() => {
                      setSelectedCategory(null);
                      void runCatalogQuery({ nextCategory: null, nextOffset: 0 });
                    }}
                    className={`px-2 py-1 text-left text-[11px] rounded cursor-pointer ${
                      selectedCategory === null
                        ? 'bg-slate-200 dark:bg-slate-800 font-medium text-slate-900 dark:text-white'
                        : 'text-slate-600 dark:text-slate-400'
                    }`}
                  >
                    {t('Все разделы', 'All sections')}
                  </button>
                  {snapshot.categories.map((cat) => (
                    <button
                      key={cat}
                      type="button"
                      onClick={() => {
                        setSelectedCategory(cat);
                        setMode('search');
                        void runCatalogQuery({ nextMode: 'search', nextCategory: cat, nextOffset: 0 });
                      }}
                      className={`px-2 py-1 text-left text-[11px] rounded truncate cursor-pointer ${
                        selectedCategory === cat
                          ? 'bg-blue-600 text-white font-medium'
                          : 'text-slate-600 dark:text-slate-400 hover:bg-slate-100 dark:hover:bg-slate-900'
                      }`}
                    >
                      {translateCategory(cat)}
                    </button>
                  ))}
                </div>
              )}
            </div>
          </aside>
        )}

        {/* Central Dense Video Results Table + Bottom Disk & File Bar */}
        <div className="flex-1 min-w-0 flex flex-col">
          {queryError && (
            <div className="m-2.5 p-2.5 rounded border border-red-300 dark:border-red-800 bg-red-50 dark:bg-red-950/40 flex items-center justify-between gap-3 text-xs text-red-800 dark:text-red-200">
              <div className="flex items-center gap-2">
                <ShieldAlert className="w-4 h-4 shrink-0 text-red-600 dark:text-red-400" />
                <span>{queryError}</span>
              </div>
              <button
                type="button"
                onClick={() => void runCatalogQuery({})}
                className="px-2.5 py-1 rounded bg-red-600 text-white font-medium hover:bg-red-500 cursor-pointer"
              >
                {t('Повторить', 'Retry')}
              </button>
            </div>
          )}

          <div className="flex-1 overflow-auto">
            {catalogOverview ? (
              <table className="w-full text-left border-collapse text-xs">
                <thead className="sticky top-0 z-10 bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-400">
                  <tr><th className="py-2 px-3 font-medium">{t('Диск каталога', 'Catalog disk')}</th><th className="py-2 px-3 font-medium">{t('Записей', 'Entries')}</th><th className="py-2 px-3 font-medium">{t('Индексировано', 'Indexed')}</th><th className="py-2 px-3 font-medium">{t('Корень', 'Recorded root')}</th></tr>
                </thead>
                <tbody>{displayedVolumes.map((volume) => (
                  <tr key={volume.id} onClick={() => handleSelectScopeDisk(volume.id)} className="border-b border-slate-100 dark:border-slate-900 hover:bg-blue-50 dark:hover:bg-slate-900/70 cursor-pointer">
                    <td className="px-3 py-2"><span className="font-mono font-semibold">#{volume.id}</span><span className="ml-2 font-medium">{volume.label}</span><span className="ml-2 text-slate-500">{volume.diskCode}</span></td>
                    <td className="px-3 py-2 font-mono tabular-nums">{volume.entryCount.toLocaleString()}</td>
                    <td className="px-3 py-2 font-mono tabular-nums">{formatBytes(volume.indexedBytes)}</td>
                    <td className="px-3 py-2 text-slate-500 truncate max-w-[320px]" title={volume.recordedRoot}>{volume.recordedRoot}</td>
                  </tr>
                ))}</tbody>
              </table>
            ) : (
            <table className="w-full text-left border-collapse text-xs">
              <thead className="sticky top-0 z-10 bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-400 select-none">
                <tr>
                  <th
                    onClick={() => handleHeaderSortClick('name')}
                    className="py-1.5 px-2.5 font-medium cursor-pointer hover:text-slate-900 dark:hover:text-white"
                  >
                    <span className="inline-flex items-center gap-1">
                      {t('Файл', 'File')}
                      {sortCol === 'name' && (sortDesc ? <ArrowDown className="w-3 h-3" /> : <ArrowUp className="w-3 h-3" />)}
                    </span>
                  </th>
                  <th className="py-1.5 px-2 font-medium hidden xl:table-cell">{t('Папка', 'Folder')}</th>
                  <th
                    onClick={() => handleHeaderSortClick('disk')}
                    className="py-1.5 px-2 font-medium cursor-pointer hover:text-slate-900 dark:hover:text-white whitespace-nowrap"
                  >
                    <span className="inline-flex items-center gap-1">
                      {t('Диск', 'Disk')}
                      {sortCol === 'disk' && (sortDesc ? <ArrowDown className="w-3 h-3" /> : <ArrowUp className="w-3 h-3" />)}
                    </span>
                  </th>
                  <th
                    onClick={() => handleHeaderSortClick('duration')}
                    title={t('Из плагина «Видео-заголовки Light» (16 байт/файл)', 'From Video Header Light plugin (16 B/file)')}
                    className="py-1.5 px-2 font-medium cursor-pointer hover:text-slate-900 dark:hover:text-white whitespace-nowrap hidden sm:table-cell"
                  >
                    <span className="inline-flex items-center gap-1">
                      {t('Видео (Длит. · Поток)', 'Video (Dur · Stream)')}
                      {sortCol === 'duration' && (sortDesc ? <ArrowDown className="w-3 h-3" /> : <ArrowUp className="w-3 h-3" />)}
                    </span>
                  </th>
                  <th
                    onClick={() => handleHeaderSortClick('size')}
                    className="py-1.5 px-2.5 font-medium text-right cursor-pointer hover:text-slate-900 dark:hover:text-white whitespace-nowrap w-28"
                  >
                    <span className="inline-flex items-center justify-end gap-1">
                      {t('Объём', 'Size')}
                      {sortCol === 'size' && (sortDesc ? <ArrowDown className="w-3 h-3" /> : <ArrowUp className="w-3 h-3" />)}
                    </span>
                  </th>
                  <th
                    onClick={() => handleHeaderSortClick('candidates')}
                    className="py-1.5 px-2 font-medium text-right cursor-pointer hover:text-slate-900 dark:hover:text-white whitespace-nowrap"
                  >
                    <span className="inline-flex items-center justify-end gap-1">
                      {t('Копии', 'Copies')}
                      {sortCol === 'candidates' && (sortDesc ? <ArrowDown className="w-3 h-3" /> : <ArrowUp className="w-3 h-3" />)}
                    </span>
                  </th>
                  <th
                    onClick={() => handleHeaderSortClick('modified')}
                    className="py-1.5 px-2.5 font-medium cursor-pointer hover:text-slate-900 dark:hover:text-white whitespace-nowrap hidden md:table-cell"
                  >
                    <span className="inline-flex items-center gap-1">
                      {t('Дата', 'Date')}
                      {sortCol === 'modified' && (sortDesc ? <ArrowDown className="w-3 h-3" /> : <ArrowUp className="w-3 h-3" />)}
                    </span>
                  </th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-200/60 dark:divide-slate-800/60">
                {loading && sortedItems.length === 0 ? (
                  Array.from({ length: 10 }).map((_, idx) => (
                    <tr key={idx} className="h-8 animate-pulse">
                      <td colSpan={7} className="px-2.5 py-1.5">
                        <div className="h-3.5 rounded bg-slate-200 dark:bg-slate-800 w-full" />
                      </td>
                    </tr>
                  ))
                ) : sortedItems.length === 0 ? (
                  <tr>
                    <td colSpan={7} className="py-14 text-center text-slate-500 dark:text-slate-400">
                      <div className="flex flex-col items-center gap-2">
                        <FileSearch className="w-6 h-6 text-slate-400" />
                        <p className="text-sm font-medium text-slate-700 dark:text-slate-300">
                          {t('Совпадений в текущей выборке не найдено', 'No matches in current scope')}
                        </p>
                        <button
                          type="button"
                          onClick={() => {
                            setQueryInput('');
                            setActiveTypeChip('');
                            setActiveSizeChip('');
                            setScopedVolumeId(null);
                            setFolderPrefix(null);
                            setAppliedQuery('');
                            setMode('search');
                            void runCatalogQuery({
                              nextMode: 'search',
                              nextQuery: '',
                              nextVolumeId: null,
                              nextFolderPrefix: null,
                              nextOffset: 0,
                            });
                          }}
                          className="mt-1 px-3 py-1 text-xs font-medium rounded bg-blue-600 text-white hover:bg-blue-500 cursor-pointer"
                        >
                          {t('Сбросить фильтры и показать все 60 дисков', 'Reset filters & show all 60 disks')}
                        </button>
                      </div>
                    </td>
                  </tr>
                ) : (
                  sortedItems.map((row, idx) => {
                    const selected = row.id === selectedEntryId;
                    const weightPct =
                      row.kind === 'file' && maxPageBytes > 0
                        ? Math.min(100, Math.round((row.size / maxPageBytes) * 100))
                        : 0;
                    const dirPath = row.relativePath.includes('/')
                      ? row.relativePath.slice(0, row.relativePath.lastIndexOf('/'))
                      : '/';

                    const prevGroupKey = idx > 0 ? sortedItems[idx - 1].dupGroupKey : null;
                    const showGroupHeader =
                      mode === 'duplicates' &&
                      Boolean(row.dupGroupKey) &&
                      row.dupGroupKey !== prevGroupKey;

                    const vm = row.videoMeta;
                    const resBadge = vm
                      ? vm.height >= 2000
                        ? '2160p'
                        : vm.height >= 1000
                          ? '1080p'
                          : `${vm.height}p`
                      : null;

                    return (
                      <React.Fragment key={row.id}>
                        {showGroupHeader && (
                          <tr className="bg-slate-100/95 dark:bg-slate-900/95 border-y border-slate-200 dark:border-slate-800">
                            <td colSpan={7} className="px-2.5 py-1 text-[11px]">
                              <div className="flex flex-wrap items-center justify-between gap-2">
                                <div className="flex items-center gap-1.5 font-medium text-slate-800 dark:text-slate-200">
                                  <Layers className="w-3.5 h-3.5 text-blue-500 shrink-0" />
                                  <span>{row.dupGroupLabel || row.name}</span>
                                  <span className="font-mono text-slate-500">
                                    · {row.dupGroupCount ?? 2}× · USB #{(row.dupGroupDisks ?? []).join(', #')}
                                  </span>
                                </div>
                                {typeof row.dupGroupExcessBytes === 'number' && row.dupGroupExcessBytes > 0 && (
                                  <span className="font-mono font-semibold tabular-nums text-blue-600 dark:text-blue-400">
                                    {t('Лишние копии/рипы:', 'Reclaimable:')} +{formatBytes(row.dupGroupExcessBytes)}
                                  </span>
                                )}
                              </div>
                            </td>
                          </tr>
                        )}

                        <tr
                          ref={(el) => {
                            if (el) rowRefs.current.set(row.id, el);
                            else rowRefs.current.delete(row.id);
                          }}
                          onClick={() => {
                            setSelectedEntryId(row.id);
                            setSelectedRegisterDiskId(row.volumeId);
                          }}
                          onDoubleClick={() => {
                            if (row.kind === 'directory') {
                              handleOpenFolder(row);
                            } else if (row.duplicateCount > 1) {
                              handleOpenCopyCandidates(row);
                            }
                          }}
                          className={`transition-colors cursor-pointer ${
                            selected
                              ? 'bg-blue-600/12 dark:bg-blue-500/20 text-slate-950 dark:text-white'
                              : 'hover:bg-slate-100/80 dark:hover:bg-slate-900/60 text-slate-800 dark:text-slate-200'
                          }`}
                        >
                          <td className="px-2.5 py-1.5 max-w-[280px]">
                            <div className="flex items-center gap-1.5 min-w-0">
                              {row.kind === 'directory' && <Folder className="w-3.5 h-3.5 text-amber-500 shrink-0" />}
                              <span className="truncate font-medium" title={row.relativePath}>
                                {row.name}
                              </span>
                            </div>
                          </td>

                          <td
                            className="px-2 py-1.5 hidden xl:table-cell max-w-[180px] font-mono text-[11px] text-slate-500 dark:text-slate-400 truncate"
                            title={dirPath}
                          >
                            {dirPath}
                          </td>

                          <td className="px-2 py-1.5 whitespace-nowrap">
                            <div className="flex items-center gap-1">
                              <span className="font-mono font-semibold tabular-nums">#{row.volumeId}</span>
                              <span className="truncate max-w-[135px] text-slate-600 dark:text-slate-300">
                                {row.volumeLabel}
                              </span>
                            </div>
                          </td>

                          <td
                            className="px-2 py-1.5 hidden sm:table-cell whitespace-nowrap font-mono text-[11px] tabular-nums"
                            title={
                              vm
                                ? `${vm.width}×${vm.height} · ${vm.codec} · ${vm.bitrateMbps.toFixed(1)} Mbps · ${vm.audioSummary}`
                                : undefined
                            }
                          >
                            {vm ? (
                              <div className="flex items-center gap-1.5">
                                <span className="text-slate-800 dark:text-slate-200">
                                  {formatDuration(vm.durationSec)}
                                </span>
                                <span
                                  className={
                                    vm.height >= 2000
                                      ? 'font-semibold text-blue-600 dark:text-blue-400'
                                      : vm.height >= 1000
                                        ? 'text-teal-600 dark:text-teal-400'
                                        : 'text-amber-600 dark:text-amber-400'
                                  }
                                >
                                  {resBadge}
                                </span>
                                <span className="text-slate-500">{vm.codec}</span>
                              </div>
                            ) : (
                              <span className="text-slate-400">—</span>
                            )}
                          </td>

                          <td
                            className="px-2.5 py-1.5 text-right font-mono tabular-nums whitespace-nowrap"
                            title={formatExactBytes(row.size, lang)}
                          >
                            {row.kind === 'directory' ? (
                              <span className="text-slate-400">{t('Папка', 'Folder')}</span>
                            ) : (
                              <div className="flex flex-col items-end">
                                <span className="font-semibold text-slate-900 dark:text-white leading-tight">
                                  {formatBytes(row.size)}
                                </span>
                                <div className="mt-0.5 w-20 h-[2px] rounded-full bg-slate-200 dark:bg-slate-800 overflow-hidden">
                                  <div
                                    className="h-full rounded-full bg-blue-600 dark:bg-blue-400"
                                    style={{ width: `${Math.max(4, weightPct)}%` }}
                                  />
                                </div>
                              </div>
                            )}
                          </td>

                          <td className="px-2 py-1.5 text-right font-mono tabular-nums whitespace-nowrap">
                            {row.kind === 'file' && row.duplicateCount > 1 ? (
                              <button
                                type="button"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleOpenCopyCandidates(row);
                                }}
                                className="text-blue-600 dark:text-blue-400 hover:underline font-semibold cursor-pointer"
                              >
                                {row.duplicateCount}×
                              </button>
                            ) : (
                              <span className="text-slate-400">1</span>
                            )}
                          </td>

                          <td className="px-2.5 py-1.5 hidden md:table-cell font-mono tabular-nums whitespace-nowrap text-slate-500 dark:text-slate-400">
                            {formatUnixDate(row.modifiedUnix)}
                          </td>
                        </tr>
                      </React.Fragment>
                    );
                  })
                )}
              </tbody>
            </table>
            )}
          </div>

          {/* Docked Disk & File Inspector Bar (with direct Rename, Properties & Rescan actions!) */}
          {activeDisk && !catalogOverview && (
            <div className="border-t border-slate-200 dark:border-slate-800 bg-slate-50/95 dark:bg-slate-900/95 px-3 py-2">
              <div className="grid grid-cols-1 lg:grid-cols-12 gap-2.5 items-center">
                {/* Active Disk Summary + Direct Management Buttons */}
                <div className="lg:col-span-5 flex flex-col gap-1 min-w-0">
                  <div className="flex flex-wrap items-center gap-1.5 text-xs">
                    <span className="font-mono font-bold tabular-nums text-slate-900 dark:text-white">
                      USB #{activeDisk.id}
                    </span>
                    <span className="font-mono text-[11px] text-slate-500">[{activeDisk.diskCode}]</span>
                    <span className="font-semibold text-slate-800 dark:text-slate-100 truncate max-w-[180px]">
                      {activeDisk.label}
                    </span>

                    <div className="inline-flex items-center gap-1 ml-1">
                      <button
                        type="button"
                        onClick={() => startInlineRename(activeDisk)}
                        title={t('Быстро переименовать диск (F2)', 'Fast rename disk (F2)')}
                        className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-[11px] text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                      >
                        <Pencil className="w-3 h-3" />
                        <span>F2</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => onOpenDiskProperties(activeDisk.id)}
                        title={t('Свойства и заметка диска (Alt+Enter)', 'Disk Properties & Note (Alt+Enter)')}
                        className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-[11px] text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                      >
                        <Settings className="w-3 h-3" />
                        <span>{t('Свойства', 'Props')}</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => onOpenScanModal(activeDisk.id)}
                        title={t('Открыть окно сканирования для этого диска', 'Open Scan Window for this disk')}
                        className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-[11px] text-blue-600 dark:text-blue-400 hover:bg-slate-100 dark:hover:bg-slate-700 cursor-pointer"
                      >
                        <RefreshCw className="w-3 h-3" />
                        <span>{t('Скан', 'Scan')}</span>
                      </button>
                    </div>
                  </div>

                  <div className="flex flex-wrap items-center gap-1.5 text-[11px] text-slate-500 dark:text-slate-400 truncate">
                    <span className="font-mono tabular-nums">
                      {activeDisk.filesystem} · {formatBytes(activeDisk.usedBytes)} / {formatBytes(activeDisk.capacityBytes)}
                    </span>
                    {activeDisk.note && (
                      <>
                        <span>·</span>
                        <span className="truncate italic text-slate-600 dark:text-slate-300" title={activeDisk.note}>
                          {activeDisk.note}
                        </span>
                      </>
                    )}
                  </div>
                </div>

                {/* Selected Video File Telemetry */}
                <div className="lg:col-span-4 text-xs min-w-0">
                  {selectedEntry ? (
                    <>
                      <div className="text-slate-500 dark:text-slate-400 font-mono text-[11px] tabular-nums truncate">
                        {formatExactBytes(selectedEntry.size, lang)}
                        {selectedEntry.videoMeta
                          ? ` · ${formatDuration(selectedEntry.videoMeta.durationSec)} · ${selectedEntry.videoMeta.width}×${selectedEntry.videoMeta.height} ${selectedEntry.videoMeta.codec} · ${selectedEntry.videoMeta.bitrateMbps.toFixed(1)} Mbps`
                          : ''}
                      </div>
                      <div
                        className="font-mono text-slate-800 dark:text-slate-200 truncate select-all"
                        title={selectedEntry.relativePath}
                      >
                        {selectedEntry.relativePath}
                      </div>
                    </>
                  ) : (
                    <span className="text-slate-500">
                      {t('Выберите видеофайл для просмотра параметров потока и пути', 'Select a video row to inspect stream & path')}
                    </span>
                  )}
                </div>

                {/* Row Actions */}
                <div className="lg:col-span-3 flex flex-wrap items-center lg:justify-end gap-1.5">
                  {selectedEntry && (
                    <button
                      type="button"
                      onClick={async () => {
                        await navigator.clipboard.writeText(
                          `#${activeDisk.id} [${activeDisk.diskCode}] (${activeDisk.label}): ${selectedEntry.relativePath}`
                        );
                        onNotify(t('Диск и путь скопированы.', 'Disk and path copied.'), 'success');
                      }}
                      className="inline-flex items-center gap-1 px-2 py-1 text-xs font-medium rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 whitespace-nowrap shrink-0 cursor-pointer"
                    >
                      <Copy className="w-3.5 h-3.5" />
                      {t('Путь', 'Path')}
                    </button>
                  )}

                  {selectedEntry && selectedEntry.kind === 'file' && selectedEntry.duplicateCount > 1 && (
                    <button
                      type="button"
                      onClick={() => handleOpenCopyCandidates(selectedEntry)}
                      className="px-2 py-1 text-xs font-medium rounded bg-blue-600 text-white hover:bg-blue-500 whitespace-nowrap shrink-0 cursor-pointer"
                    >
                      {t('Копии', 'Copies')} ({selectedEntry.duplicateCount})
                    </button>
                  )}

                  {selectedEntry && (
                    <button
                      type="button"
                      onClick={() => handleBrowseParentFolder(selectedEntry)}
                      className="px-2 py-1 text-xs font-medium rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 whitespace-nowrap shrink-0 cursor-pointer"
                    >
                      {t('В папку', 'Folder')}
                    </button>
                  )}

                  <button
                    type="button"
                    onClick={() => {
                      const folder = selectedEntry && selectedEntry.relativePath.includes('/')
                        ? selectedEntry.relativePath.slice(0, selectedEntry.relativePath.lastIndexOf('/'))
                        : mode === 'browse'
                          ? browseCrumbs.map((crumb) => crumb.name).join('/') || null
                          : folderPrefix;
                      onOpenSunburst(activeDisk.id, folder);
                    }}
                    className="inline-flex items-center gap-1 px-2 py-1 text-xs font-medium rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 whitespace-nowrap shrink-0 cursor-pointer"
                  >
                    <PieChart className="w-3.5 h-3.5 text-blue-500" />
                    {t('Карта', 'Map')}
                  </button>
                </div>
              </div>

              {pageData?.copyCandidatesMeta && (
                <div className="mt-1.5 pt-1.5 border-t border-slate-200 dark:border-slate-800 flex flex-wrap items-center gap-1.5 text-xs">
                  <span className="text-slate-500">{t('Копии на других USB:', 'Copies on other USBs:')}</span>
                  <button
                    type="button"
                    onClick={() => {
                      setCandidateVolumeId(null);
                      void runCatalogQuery({ nextCandidateVolumeId: null, nextOffset: 0 });
                    }}
                    className={`px-2 py-0.5 rounded cursor-pointer ${
                      candidateVolumeId === null
                        ? 'bg-slate-900 text-white dark:bg-slate-100 dark:text-slate-900 font-medium'
                        : 'text-slate-600 dark:text-slate-400 hover:underline'
                    }`}
                  >
                    {t('Все', 'All')} ({pageData.copyCandidatesMeta.otherTotal})
                  </button>
                  {pageData.copyCandidatesMeta.diskGroups.map((g) => (
                    <button
                      key={g.volumeId}
                      type="button"
                      onClick={() => {
                        setCandidateVolumeId(g.volumeId);
                        void runCatalogQuery({ nextCandidateVolumeId: g.volumeId, nextOffset: 0 });
                      }}
                      className={`px-2 py-0.5 rounded font-mono tabular-nums cursor-pointer ${
                        candidateVolumeId === g.volumeId
                          ? 'bg-blue-600 text-white font-medium'
                          : 'bg-slate-200/70 dark:bg-slate-800 text-slate-800 dark:text-slate-200'
                      }`}
                    >
                      #{g.volumeId} · {g.label} ({g.count})
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Status & Pagination Footer */}
          <div className="border-t border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-900/60 px-3 py-1.5 flex flex-wrap items-center justify-between gap-2 text-xs font-mono tabular-nums text-slate-600 dark:text-slate-400">
            <div className="flex flex-wrap items-center gap-2">
              {loading ? (
                <span className="inline-flex items-center gap-1.5 text-blue-600 dark:text-blue-400">
                  <RefreshCw className="w-3.5 h-3.5 animate-spin" />
                  {t('Поиск…', 'Searching…')}
                </span>
              ) : pageData ? (
                <>
                  <span>
                    {pageData.items.length === 0 ? 0 : pageData.offset + 1}–{pageData.offset + pageData.items.length}{' '}
                    {t('из', 'of')} {pageData.total}
                  </span>
                  <span>·</span>
                  <span>
                    {t('объём выборки', 'matched')} {formatBytes(pageData.totalMatchedBytes)}
                  </span>
                  <span>·</span>
                  <span>{pageData.elapsedMs} ms</span>
                </>
              ) : (
                <span>{t('Ожидание…', 'Idle…')}</span>
              )}
            </div>

            <div className="flex items-center gap-1.5">
              <button
                type="button"
                disabled={!pageData || offset === 0 || loading}
                onClick={() => {
                  const nextOff = Math.max(0, offset - 250);
                  setOffset(nextOff);
                  void runCatalogQuery({ nextOffset: nextOff });
                }}
                className="px-2 py-0.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 disabled:opacity-40 cursor-pointer"
              >
                {t('Пред.', 'Prev')}
              </button>
              <button
                type="button"
                disabled={!pageData?.hasMore || loading}
                onClick={() => {
                  const nextOff = offset + 250;
                  setOffset(nextOff);
                  void runCatalogQuery({ nextOffset: nextOff });
                }}
                className="px-2 py-0.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 disabled:opacity-40 cursor-pointer"
              >
                {t('След.', 'Next')}
              </button>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
