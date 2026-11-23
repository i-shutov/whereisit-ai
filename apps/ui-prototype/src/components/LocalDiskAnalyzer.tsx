import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowUpLeft,
  Check,
  ChevronDown,
  ChevronRight,
  Download,
  FileCode2,
  FolderOpen,
  FolderUp,
  HardDrive,
  History,
  Info,
  Layers,
  Plus,
  RefreshCw,
  ShieldAlert,
  ShieldCheck,
  X,
} from 'lucide-react';
import type { DuplicateCriterion, SystemSnapshot } from '../types/atlas';
import { formatBytes, formatExactBytes } from '../utils/format';
import { metadataCandidateKey, normalizeMetadataName } from '../utils/metadataCandidates';
import { escapeReviewCsvCell } from '../utils/reviewCsv';
import { useI18n } from '../i18n';

interface LocalDiskAnalyzerProps {
  snapshot: SystemSnapshot;
  onNotify: (message: string, tone?: 'info' | 'error' | 'success') => void;
}

type LocalMode = 'tree_map' | 'largest' | 'bloat' | 'duplicates' | 'backup' | 'diff';
type BloatCategory = 'dev' | 'cache' | 'installers' | 'archives';

interface LocalFileItem {
  id: string;
  name: string;
  nameNorm: string;
  relativePath: string; // relative to root folder, using '/'
  parentPath: string;
  ext: string;
  size: number;
  lastModifiedMs: number;
  fileObj?: File;
  fileHandle?: FileSystemFileHandle;
}

interface LocalDirNode {
  id: string;
  name: string;
  fullPath: string; // '' for root
  size: number;
  fileCount: number;
  folderCount: number;
  latestMtimeMs: number;
  colorIndex: number;
  subdirs: LocalDirNode[];
  files: LocalFileItem[];
}

interface SunburstArc {
  id: string;
  kind: 'directory' | 'file' | 'other';
  name: string;
  fullPath: string;
  parentPath: string;
  size: number;
  fileCount: number;
  colorIndex: number;
  depth: number;
  pathD: string;
  fill: string;
  ancestorIds: string[];
  fileItem?: LocalFileItem;
}

interface UsbMatchEntry {
  volumeId: number;
  volumeLabel: string;
  relativePath: string;
}

interface BaselineStore {
  sourceSessionId: number;
  savedAtMs: number;
  folders: Record<string, number>;
}

const GB = 1024 ** 3;
const MB = 1024 ** 2;

const DAISY_PALETTES: string[][] = [
  ['#2563EB', '#3B82F6', '#60A5FA', '#93C5FD'], // Sapphire
  ['#0D9488', '#14B8A6', '#2DD4BF', '#5EEAD4'], // Teal
  ['#D97706', '#F59E0B', '#FBBF24', '#FCD34D'], // Amber
  ['#7C3AED', '#8B5CF6', '#A78BFA', '#C4B5FD'], // Violet
  ['#E11D48', '#F43F5E', '#FB7185', '#FDA4AF'], // Rose
  ['#0284C7', '#0EA5E9', '#38BDF8', '#7DD3FC'], // Sky
  ['#059669', '#10B981', '#34D399', '#6EE7B7'], // Emerald
  ['#4F46E5', '#6366F1', '#818CF8', '#A5B4FC'], // Indigo
];

const RING_RADII: [number, number][] = [
  [84, 128],
  [131, 171],
  [174, 210],
  [213, 244],
];

function polarToCartesian(cx: number, cy: number, r: number, angleRad: number) {
  return {
    x: cx + r * Math.cos(angleRad),
    y: cy + r * Math.sin(angleRad),
  };
}

function describeAnnularSector(
  cx: number,
  cy: number,
  rInner: number,
  rOuter: number,
  startAngle: number,
  endAngle: number
): string {
  const sweep = endAngle - startAngle;
  if (sweep <= 0.0005) return '';
  if (sweep >= Math.PI * 2 - 0.001) {
    const mid = startAngle + Math.PI;
    const p1 = polarToCartesian(cx, cy, rOuter, startAngle);
    const p2 = polarToCartesian(cx, cy, rOuter, mid);
    const p3 = polarToCartesian(cx, cy, rInner, mid);
    const p4 = polarToCartesian(cx, cy, rInner, startAngle);
    return [
      `M ${p1.x.toFixed(2)} ${p1.y.toFixed(2)}`,
      `A ${rOuter} ${rOuter} 0 1 1 ${p2.x.toFixed(2)} ${p2.y.toFixed(2)}`,
      `A ${rOuter} ${rOuter} 0 1 1 ${p1.x.toFixed(2)} ${p1.y.toFixed(2)}`,
      `M ${p4.x.toFixed(2)} ${p4.y.toFixed(2)}`,
      `A ${rInner} ${rInner} 0 1 0 ${p3.x.toFixed(2)} ${p3.y.toFixed(2)}`,
      `A ${rInner} ${rInner} 0 1 0 ${p4.x.toFixed(2)} ${p4.y.toFixed(2)}`,
      'Z',
    ].join(' ');
  }
  const largeArc = sweep > Math.PI ? 1 : 0;
  const outerStart = polarToCartesian(cx, cy, rOuter, startAngle);
  const outerEnd = polarToCartesian(cx, cy, rOuter, endAngle);
  const innerEnd = polarToCartesian(cx, cy, rInner, endAngle);
  const innerStart = polarToCartesian(cx, cy, rInner, startAngle);
  return [
    `M ${outerStart.x.toFixed(2)} ${outerStart.y.toFixed(2)}`,
    `A ${rOuter} ${rOuter} 0 ${largeArc} 1 ${outerEnd.x.toFixed(2)} ${outerEnd.y.toFixed(2)}`,
    `L ${innerEnd.x.toFixed(2)} ${innerEnd.y.toFixed(2)}`,
    `A ${rInner} ${rInner} 0 ${largeArc} 0 ${innerStart.x.toFixed(2)} ${innerStart.y.toFixed(2)}`,
    'Z',
  ].join(' ');
}

function buildLocalDirectoryTree(rootLabel: string, files: LocalFileItem[]): {
  root: LocalDirNode;
  dirMap: Map<string, LocalDirNode>;
} {
  interface MutableDir {
    id: string;
    name: string;
    fullPath: string;
    size: number;
    fileCount: number;
    latestMtimeMs: number;
    subdirs: Map<string, MutableDir>;
    files: LocalFileItem[];
  }

  const rootMut: MutableDir = {
    id: 'dir::',
    name: rootLabel,
    fullPath: '',
    size: 0,
    fileCount: 0,
    latestMtimeMs: 0,
    subdirs: new Map(),
    files: [],
  };

  for (const f of files) {
    const parts = f.relativePath.split('/').filter(Boolean);
    rootMut.size += f.size;
    rootMut.fileCount += 1;
    if (f.lastModifiedMs > rootMut.latestMtimeMs) rootMut.latestMtimeMs = f.lastModifiedMs;

    let curr = rootMut;
    let accPath = '';
    for (let i = 0; i < parts.length - 1; i++) {
      const seg = parts[i];
      accPath = accPath ? `${accPath}/${seg}` : seg;
      let child = curr.subdirs.get(seg);
      if (!child) {
        child = {
          id: `dir::${accPath}`,
          name: seg,
          fullPath: accPath,
          size: 0,
          fileCount: 0,
          latestMtimeMs: 0,
          subdirs: new Map(),
          files: [],
        };
        curr.subdirs.set(seg, child);
      }
      child.size += f.size;
      child.fileCount += 1;
      if (f.lastModifiedMs > child.latestMtimeMs) child.latestMtimeMs = f.lastModifiedMs;
      curr = child;
    }
    curr.files.push(f);
  }

  const dirMap = new Map<string, LocalDirNode>();

  const freeze = (m: MutableDir, depth: number, inheritedColor: number): LocalDirNode => {
    const subdirs = Array.from(m.subdirs.values()).sort((a, b) => b.size - a.size);
    const frozenSubdirs = subdirs.map((sub, idx) =>
      freeze(sub, depth + 1, depth === 0 ? idx % 8 : inheritedColor)
    );
    const frozenFiles = [...m.files].sort((a, b) => b.size - a.size);
    let totalSubCount = frozenSubdirs.length;
    for (const s of frozenSubdirs) totalSubCount += s.folderCount;

    const node: LocalDirNode = {
      id: m.id,
      name: m.name,
      fullPath: m.fullPath,
      size: m.size,
      fileCount: m.fileCount,
      folderCount: totalSubCount,
      latestMtimeMs: m.latestMtimeMs,
      colorIndex: depth === 0 ? 0 : inheritedColor,
      subdirs: frozenSubdirs,
      files: frozenFiles,
    };
    dirMap.set(m.fullPath, node);
    return node;
  };

  const root = freeze(rootMut, 0, 0);
  return { root, dirMap };
}

function buildSampleWorkstationFiles(): { rootName: string; files: LocalFileItem[] } {
  const now = Date.now();
  const day = 86400 * 1000;
  const raw: { path: string; size: number; daysAgo: number }[] = [
    // Exact copy & already backed up on USB #4
    { path: 'Cinema_Staging/Солярис_1972_Mosfilm_4K_Remux.mkv', size: Math.round(48.65 * GB), daysAgo: 210 },
    { path: 'Downloads/Solaris_Backup_Copy/Солярис_1972_Mosfilm_4K_Remux.mkv', size: Math.round(48.65 * GB), daysAgo: 35 },
    // Same filename, different byte size (1080p re-encode vs 4K Remux)
    { path: 'Downloads/Mobile_Transcode/Солярис_1972_Mosfilm_4K_Remux.mkv', size: Math.round(6.85 * GB), daysAgo: 14 },
    // Backed up on USB #1
    { path: 'Cinema_Staging/Blade_Runner_1982_Final_Cut_UHD.mkv', size: Math.round(62.11 * GB), daysAgo: 440 },
    { path: 'Music_Masters/Bach_Goldberg_Variations_Gould_1981_24-96.flac', size: Math.round(1.85 * GB), daysAgo: 150 },
    // Renamed byte-exact duplicate inside ML_Pipeline
    { path: 'Projects/ML_Pipeline/checkpoints/epoch_048_weights.safetensors', size: 12240656794, daysAgo: 12 },
    { path: 'Projects/ML_Pipeline/checkpoints/epoch_048_weights_backup_old.safetensors', size: 12240656794, daysAgo: 10 },
    // Dev & cache bloat
    { path: 'Projects/WhereIsIt_UI/node_modules/chromium-bundle/chrome_linux.bin', size: Math.round(1.9 * GB), daysAgo: 95 },
    { path: 'Projects/WhereIsIt_UI/node_modules/typescript-llvm/bundle.wasm', size: Math.round(1.25 * GB), daysAgo: 95 },
    { path: 'Projects/WhereIsIt_UI/dist/assets/vendor-chunk.js', size: Math.round(420 * MB), daysAgo: 18 },
    { path: 'Projects/ML_Pipeline/.venv/lib/torch_cuda_kernels.so', size: Math.round(4.8 * GB), daysAgo: 120 },
    { path: 'Downloads/Installers/Xcode_16_2_Universal.dmg', size: Math.round(7.6 * GB), daysAgo: 290 },
    { path: 'Downloads/Installers/Ubuntu_24_04_2_Desktop_amd64.iso', size: Math.round(5.9 * GB), daysAgo: 380 },
    { path: 'Downloads/Installers/DaVinci_Resolve_Studio_19.zip', size: Math.round(4.1 * GB), daysAgo: 195 },
    { path: 'Caches_Temp/Docker_VM/DockerDesktop.raw', size: Math.round(14.3 * GB), daysAgo: 42 },
    { path: 'Caches_Temp/Xcode_DerivedData/BuildCache.db', size: Math.round(6.7 * GB), daysAgo: 78 },
    { path: 'Caches_Temp/Browser_Media_Cache/stream_dump_01.tmp', size: Math.round(2.3 * GB), daysAgo: 61 },
    { path: 'Workstation_RAW/2025_Caucasus/DSC_8801_Panorama.tiff', size: Math.round(3.8 * GB), daysAgo: 65 },
    { path: 'Workstation_RAW/2025_Caucasus/DSC_8802_Glacier.dng', size: Math.round(1.4 * GB), daysAgo: 65 },
    { path: 'Workstation_RAW/2024_Altai/Altai_Timelapse_ProRes.mov', size: Math.round(18.2 * GB), daysAgo: 410 },
    { path: 'Documents/Taxes_And_Contracts_2025.pdf', size: Math.round(145 * MB), daysAgo: 22 },
  ];

  const files: LocalFileItem[] = raw.map((r, idx) => {
    const parts = r.path.split('/');
    const name = parts[parts.length - 1];
    const parentPath = parts.slice(0, -1).join('/');
    const dot = name.lastIndexOf('.');
    return {
      id: `local-${idx}-${r.path}`,
      name,
      nameNorm: normalizeMetadataName(name),
      relativePath: r.path,
      parentPath,
      ext: dot > 0 ? name.slice(dot + 1).toLowerCase() : '',
      size: r.size,
      lastModifiedMs: now - r.daysAgo * day,
    };
  });

  return { rootName: 'Demo_PC_Workspace', files };
}

function classifyBloatItem(file: LocalFileItem): BloatCategory | null {
  const p = `/${file.relativePath.toLowerCase()}`;
  const ext = file.ext.toLowerCase();
  if (
    p.includes('/node_modules/') ||
    p.includes('/.venv/') ||
    p.includes('/__pycache__/') ||
    p.includes('/target/debug/') ||
    p.includes('/target/release/') ||
    p.includes('/.next/') ||
    p.includes('/dist/') ||
    p.includes('/build/') ||
    p.includes('/deriveddata/')
  ) {
    return 'dev';
  }
  if (
    p.includes('/cache') ||
    p.includes('/caches') ||
    p.includes('/temp') ||
    p.includes('/tmp/') ||
    ['tmp', 'temp', 'bak', 'log', 'dmp'].includes(ext) ||
    file.nameNorm === '.ds_store' ||
    file.nameNorm === 'thumbs.db'
  ) {
    return 'cache';
  }
  if (['dmg', 'pkg', 'exe', 'msi', 'iso', 'ova'].includes(ext)) {
    return 'installers';
  }
  if (['zip', 'tar', 'gz', 'tgz', '7z', 'rar'].includes(ext) && file.size >= 50 * MB) {
    return 'archives';
  }
  return null;
}

// Hash-based file inspection is intentionally deferred; this prototype uses metadata only.
export const LocalDiskAnalyzer: React.FC<LocalDiskAnalyzerProps> = ({ snapshot, onNotify }) => {
  const { lang, t } = useI18n();
  const numLocale = lang === 'en' ? 'en-US' : 'ru-RU';

  // Detect whether File System Access API is genuinely available (not inside a cross-origin iframe)
  const canUseDirectFsApi = useMemo(() => {
    if (typeof window === 'undefined') return false;
    const hasPicker = typeof (window as unknown as { showDirectoryPicker?: unknown }).showDirectoryPicker === 'function';
    let isTopFrame = false;
    try {
      isTopFrame = window.self === window.top;
    } catch {
      isTopFrame = false;
    }
    return hasPicker && isTopFrame;
  }, []);

  const [rootLabel, setRootLabel] = useState<string>('');
  const [files, setFiles] = useState<LocalFileItem[]>([]);
  const [sourceKind, setSourceKind] = useState<'none' | 'webkit' | 'fs_api' | 'sample'>('none');
  const [rootDirHandle, setRootDirHandle] = useState<FileSystemDirectoryHandle | null>(null);

  const [scanning, setScanning] = useState(false);
  const [scanCount, setScanCount] = useState(0);
  const scanRunRef = useRef(0);
  const [activeMode, setActiveMode] = useState<LocalMode>('tree_map');

  // Tree + Sunburst Navigation & Filter
  const [currentFolderPath, setCurrentFolderPath] = useState<string>('');
  const [expandedFolders, setExpandedFolders] = useState<Set<string>>(() => new Set());
  const [hoveredArc, setHoveredArc] = useState<SunburstArc | null>(null);
  const [hoveredAncestors, setHoveredAncestors] = useState<Set<string>>(() => new Set());
  const [localFilter, setLocalFilter] = useState('');

  // Largest & Stale filters
  const [minSizeFilterMb, setMinSizeFilterMb] = useState<number>(100);
  const [minAgeDaysFilter, setMinAgeDaysFilter] = useState<number>(0);

  // Collector Staging Basket (DaisyDisk Collector equivalent)
  const [stagedPaths, setStagedPaths] = useState<Set<string>>(() => new Set());

  // Metadata candidate review only. Content fingerprints/hashes are deferred.
  const [dupCriterion, setDupCriterion] = useState<DuplicateCriterion>('name_size');

  // Read-only USB metadata candidate matches
  const [usbMatches, setUsbMatches] = useState<Record<string, UsbMatchEntry[]>>({});
  const [checkingUsb, setCheckingUsb] = useState(false);
  const [usbCheckedAt, setUsbCheckedAt] = useState<number | null>(null);
  const [usbCheckProgress, setUsbCheckProgress] = useState<{ processed: number; total: number } | null>(null);
  const usbMatchRunRef = useRef(0);
  const usbMatchAbortRef = useRef<AbortController | null>(null);
  const [backupFilter, setBackupFilter] = useState<'all' | 'no_candidate' | 'candidate'>('all');

  // Baseline snapshot diff ("What grew?")
  const [baseline, setBaseline] = useState<BaselineStore | null>(null);
  const sourceSessionIdRef = useRef(0);

  // Read-only, chunked metadata candidate lookup. A run token rejects stale responses after reset/cancel.
  const runUsbBackupMatch = useCallback(
    async (itemsToCheck: LocalFileItem[], silent = false) => {
      usbMatchAbortRef.current?.abort();
      const controller = new AbortController();
      usbMatchAbortRef.current = controller;
      const runId = ++usbMatchRunRef.current;
      setUsbMatches({});
      setUsbCheckedAt(null);
      setCheckingUsb(itemsToCheck.length > 0);
      setUsbCheckProgress(itemsToCheck.length > 0 ? { processed: 0, total: itemsToCheck.length } : null);
      if (itemsToCheck.length === 0) {
        usbMatchAbortRef.current = null;
        setCheckingUsb(false);
        return;
      }

      let matchedKeys = 0;
      try {
        const chunkSize = 1000;
        for (let start = 0; start < itemsToCheck.length; start += chunkSize) {
          const chunk = itemsToCheck.slice(start, start + chunkSize);
          const resp = await fetch('/api/catalog/match-local', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            signal: controller.signal,
            body: JSON.stringify({
              files: chunk.map((f) => ({ key: f.relativePath, name: f.name, size: f.size })),
            }),
          });
          const data = await resp.json();
          if (runId !== usbMatchRunRef.current) return;
          if (!resp.ok) throw new Error(data.error || t('Ошибка поиска кандидатов USB', 'USB candidate lookup failed'));
          const matches = (data.matches ?? {}) as Record<string, UsbMatchEntry[]>;
          matchedKeys += Object.keys(matches).length;
          setUsbMatches((prev) => ({ ...prev, ...matches }));
          setUsbCheckProgress({ processed: Math.min(start + chunk.length, itemsToCheck.length), total: itemsToCheck.length });
        }
        if (runId !== usbMatchRunRef.current) return;
        setUsbCheckedAt(Date.now());
        if (!silent) {
          onNotify(
            t(
              `Найдено совпадений-кандидатов по имени и размеру: ${matchedKeys}. Равенство содержимого и резерв не проверялись.`,
              `Found ${matchedKeys} name-and-size metadata candidates. Content equality and backup status were not verified.`
            ),
            'success'
          );
        }
      } catch (err) {
        if (runId === usbMatchRunRef.current && (err as Error).name !== 'AbortError' && !silent) {
          onNotify((err as Error).message, 'error');
        }
      } finally {
        if (runId === usbMatchRunRef.current) {
          usbMatchAbortRef.current = null;
          setCheckingUsb(false);
          setUsbCheckProgress(null);
        }
      }
    },
    [onNotify, t]
  );

  const cancelUsbMatch = useCallback(() => {
    usbMatchRunRef.current += 1;
    usbMatchAbortRef.current?.abort();
    usbMatchAbortRef.current = null;
    setCheckingUsb(false);
    setUsbCheckProgress(null);
  }, []);

  const cancelLocalScan = useCallback(() => {
    scanRunRef.current += 1;
    setScanning(false);
    setScanCount(0);
  }, []);

  useEffect(() => () => {
    scanRunRef.current += 1;
    usbMatchRunRef.current += 1;
    usbMatchAbortRef.current?.abort();
  }, []);

  const resetAnalysisWithFiles = useCallback(
    (
      label: string,
      loaded: LocalFileItem[],
      kind: 'webkit' | 'fs_api' | 'sample',
      dirHandle: FileSystemDirectoryHandle | null = null,
      rw = false
    ) => {
      cancelUsbMatch();
      setRootLabel(label);
      sourceSessionIdRef.current += 1;
      setFiles(loaded);
      setSourceKind(kind);
      setRootDirHandle(dirHandle);
      setCurrentFolderPath('');
      setExpandedFolders(new Set());
      setHoveredArc(null);
      setStagedPaths(new Set());
      setLocalFilter('');
      setBackupFilter('all');
      setUsbMatches({});
      setUsbCheckedAt(null);
      setBaseline(null);
    },
    [cancelUsbMatch]
  );

  // Primary Browser Folder Picker (<input webkitdirectory>) — works in 100% of desktop browsers & embedded frames
  const handleWebkitDirectoryPick = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const fileList = e.target.files;
    if (!fileList || fileList.length === 0) return;
    const runId = ++scanRunRef.current;
    setScanning(true);
    setScanCount(0);

    try {
      let label = 'Local_Folder';
      const parsed: LocalFileItem[] = [];
      for (let i = 0; i < fileList.length; i++) {
        const f = fileList[i] as File & { webkitRelativePath?: string };
        const rawRel = f.webkitRelativePath || f.name;
        if (i === 0 && rawRel.includes('/')) {
          label = rawRel.split('/')[0];
        }
        const rel = rawRel.includes('/') ? rawRel.split('/').slice(1).join('/') : rawRel;
        const cleanRel = rel.replace(/^\/+/, '');
        const lastSlash = cleanRel.lastIndexOf('/');
        const parentPath = lastSlash >= 0 ? cleanRel.slice(0, lastSlash) : '';
        const dot = f.name.lastIndexOf('.');
        parsed.push({
          id: `wk-${i}-${cleanRel}`,
          name: f.name,
          nameNorm: normalizeMetadataName(f.name),
          relativePath: cleanRel,
          parentPath,
          ext: dot > 0 ? f.name.slice(dot + 1).toLowerCase() : '',
          size: f.size,
          lastModifiedMs: f.lastModified || Date.now(),
          fileObj: f,
        });
        if ((i + 1) % 500 === 0 || i === fileList.length - 1) {
          setScanCount(i + 1);
          await new Promise<void>((resolve) => window.setTimeout(resolve, 0));
          if (runId !== scanRunRef.current) return;
        }
      }
      if (runId !== scanRunRef.current) return;
      setScanCount(parsed.length);
      resetAnalysisWithFiles(label, parsed, 'webkit', null, false);
      onNotify(
        t(
          `Открыта папка ПК «${label}»: ${parsed.length} файлов. Анализ только в памяти, в архив не сохраняется.`,
          `Opened PC folder "${label}": ${parsed.length} files. Analyzed in RAM only, nothing saved to archive.`
        ),
        'success'
      );
    } finally {
      if (runId === scanRunRef.current) setScanning(false);
      e.target.value = '';
    }
  };

  // Optional Read-only Folder Picker (shown ONLY in top-level standalone Chromium window)
  const handleFsApiPick = async () => {
    const w = window as unknown as {
      showDirectoryPicker?: (opts?: { mode?: 'read' | 'readwrite' }) => Promise<FileSystemDirectoryHandle>;
    };
    if (typeof w.showDirectoryPicker !== 'function') return;

    let scanRunId: number | null = null;
    try {
      let rw = false;
      let dirHandle: FileSystemDirectoryHandle;
      try {
        dirHandle = await w.showDirectoryPicker({ mode: 'read' });
      } catch (err) {
        if ((err as Error).name === 'AbortError') return;
        rw = false;
        dirHandle = await w.showDirectoryPicker({ mode: 'read' });
      }

      const runId = ++scanRunRef.current;
      scanRunId = runId;
      setScanning(true);
      setScanCount(0);
      const collected: LocalFileItem[] = [];

      const walk = async (handle: FileSystemDirectoryHandle, prefix: string) => {
        const iter = (handle as unknown as {
          values: () => AsyncIterable<FileSystemHandle>;
        }).values();
        for await (const entry of iter) {
          if (runId !== scanRunRef.current) return;
          if (entry.kind === 'file') {
            const fh = entry as FileSystemFileHandle;
            const file = await fh.getFile();
            const cleanRel = prefix ? `${prefix}/${entry.name}` : entry.name;
            const dot = entry.name.lastIndexOf('.');
            collected.push({
              id: `fs-${collected.length}-${cleanRel}`,
              name: entry.name,
              nameNorm: normalizeMetadataName(entry.name),
              relativePath: cleanRel,
              parentPath: prefix,
              ext: dot > 0 ? entry.name.slice(dot + 1).toLowerCase() : '',
              size: file.size,
              lastModifiedMs: file.lastModified || Date.now(),
              fileObj: file,
              fileHandle: fh,
            });
            if (collected.length % 200 === 0) {
              setScanCount(collected.length);
              await new Promise<void>((resolve) => window.setTimeout(resolve, 0));
            }
            if (collected.length >= 40000) return;
          } else if (entry.kind === 'directory') {
            await walk(entry as FileSystemDirectoryHandle, prefix ? `${prefix}/${entry.name}` : entry.name);
            if (collected.length >= 40000) return;
          }
        }
      };

      await walk(dirHandle, '');
      if (runId !== scanRunRef.current) return;
      setScanCount(collected.length);
      resetAnalysisWithFiles(dirHandle.name, collected, 'fs_api', dirHandle, rw);
      onNotify(
        t(
          `Открыт каталог «${dirHandle.name}» (${collected.length} файлов) с прямым доступом.`,
          `Opened "${dirHandle.name}" (${collected.length} files) with direct FS access.`
        ),
        'success'
      );
    } catch (err) {
      if ((err as Error).name !== 'AbortError') {
        onNotify((err as Error).message, 'error');
      }
    } finally {
      if (scanRunId !== null && scanRunId === scanRunRef.current) setScanning(false);
    }
  };

  // Pre-load clearly-labeled demo dataset on first visit so user can inspect all 6 views right away
  useEffect(() => {
    if (sourceKind === 'none') {
      const sample = buildSampleWorkstationFiles();
      resetAnalysisWithFiles(sample.rootName, sample.files, 'sample', null, false);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const { root: treeRoot, dirMap } = useMemo(
    () => buildLocalDirectoryTree(rootLabel || t('Локальный диск', 'Local Disk'), files),
    [rootLabel, files, t]
  );

  const activeDirNode = useMemo(
    () => dirMap.get(currentFolderPath) ?? treeRoot,
    [dirMap, currentFolderPath, treeRoot]
  );

  const breadcrumbs = useMemo(() => {
    const list: { label: string; path: string }[] = [{ label: treeRoot.name, path: '' }];
    if (!currentFolderPath) return list;
    const segs = currentFolderPath.split('/').filter(Boolean);
    let acc = '';
    for (const seg of segs) {
      acc = acc ? `${acc}/${seg}` : seg;
      list.push({ label: seg, path: acc });
    }
    return list;
  }, [treeRoot.name, currentFolderPath]);

  // Precompute metadata candidate coverage per folder path.
  const folderCandidateStats = useMemo(() => {
    const map = new Map<string, { backedBytes: number; uniqueBytes: number }>();
    for (const f of files) {
      const hasCandidate = (usbMatches[f.relativePath]?.length ?? 0) > 0;
      const parts = f.parentPath ? f.parentPath.split('/') : [];
      const paths = [''];
      let acc = '';
      for (const p of parts) {
        acc = acc ? `${acc}/${p}` : p;
        paths.push(acc);
      }
      for (const fp of paths) {
        const slot = map.get(fp) ?? { backedBytes: 0, uniqueBytes: 0 };
        if (hasCandidate) slot.backedBytes += f.size;
        else slot.uniqueBytes += f.size;
        map.set(fp, slot);
      }
    }
    return map;
  }, [files, usbMatches]);

  // Build DaisyDisk 4-Ring Sunburst Arcs from activeDirNode
  const { arcs } = useMemo(() => {
    if (!activeDirNode || activeDirNode.size <= 0) return { arcs: [] as SunburstArc[] };
    const cx = 260;
    const cy = 260;
    const computed: SunburstArc[] = [];

    const walkRing = (
      dir: LocalDirNode,
      startAng: number,
      endAng: number,
      depth: number,
      ancestors: string[]
    ) => {
      if (depth > 4 || dir.size <= 0) return;
      const totalSpan = endAng - startAng;
      if (totalSpan <= 0.015) return;

      const [rIn, rOut] = RING_RADII[depth - 1];
      type ChildItem =
        | { kind: 'directory'; node: LocalDirNode; size: number; colorIndex: number }
        | { kind: 'file'; file: LocalFileItem; size: number; colorIndex: number }
        | { kind: 'other'; size: number; count: number; colorIndex: number };

      const rawChildren: ChildItem[] = [
        ...dir.subdirs.map((sd, i) => ({
          kind: 'directory' as const,
          node: sd,
          size: sd.size,
          colorIndex: depth === 1 ? i % 8 : dir.colorIndex,
        })),
        ...dir.files.map((fl, i) => ({
          kind: 'file' as const,
          file: fl,
          size: fl.size,
          colorIndex: depth === 1 ? (dir.subdirs.length + i) % 8 : dir.colorIndex,
        })),
      ].sort((a, b) => b.size - a.size);

      const visible: ChildItem[] = [];
      let otherSize = 0;
      let otherCount = 0;
      for (const ch of rawChildren) {
        if (ch.size <= 0) continue;
        const span = (ch.size / dir.size) * totalSpan;
        if (span < 0.025 && visible.length >= 9) {
          otherSize += ch.size;
          otherCount += ch.kind === 'directory' ? ch.node.fileCount : 1;
        } else {
          visible.push(ch);
        }
      }
      if (otherSize > 0) {
        visible.push({ kind: 'other', size: otherSize, count: otherCount, colorIndex: 0 });
      }

      let cursor = startAng;
      for (let i = 0; i < visible.length; i++) {
        const ch = visible[i];
        const nextCursor =
          i === visible.length - 1 ? endAng : cursor + (ch.size / dir.size) * totalSpan;
        const pathD = describeAnnularSector(cx, cy, rIn, rOut, cursor, nextCursor);
        if (pathD) {
          const palette = DAISY_PALETTES[ch.colorIndex % DAISY_PALETTES.length];
          const fill =
            ch.kind === 'other'
              ? '#64748B'
              : palette[Math.min(palette.length - 1, depth - 1)];

          const arcId =
            ch.kind === 'directory'
              ? ch.node.id
              : ch.kind === 'file'
                ? ch.file.id
                : `other-${depth}-${dir.fullPath}`;
          const chain = [...ancestors, arcId];

          computed.push({
            id: arcId,
            kind: ch.kind,
            name:
              ch.kind === 'directory'
                ? ch.node.name
                : ch.kind === 'file'
                  ? ch.file.name
                  : t(`Прочие (${ch.count})`, `Other (${ch.count})`),
            fullPath:
              ch.kind === 'directory'
                ? ch.node.fullPath
                : ch.kind === 'file'
                  ? ch.file.relativePath
                  : dir.fullPath,
            parentPath: dir.fullPath,
            size: ch.size,
            fileCount:
              ch.kind === 'directory'
                ? ch.node.fileCount
                : ch.kind === 'file'
                  ? 1
                  : ch.count,
            colorIndex: ch.colorIndex,
            depth,
            pathD,
            fill,
            ancestorIds: chain,
            fileItem: ch.kind === 'file' ? ch.file : undefined,
          });

          if (ch.kind === 'directory' && depth < 4) {
            walkRing(
              { ...ch.node, colorIndex: ch.colorIndex },
              cursor,
              nextCursor,
              depth + 1,
              chain
            );
          }
        }
        cursor = nextCursor;
      }
    };

    walkRing({ ...activeDirNode, colorIndex: 0 }, -Math.PI / 2, Math.PI * 1.5, 1, []);
    return { arcs: computed };
  }, [activeDirNode, t]);

  // Staging helpers (prevent double-counting files inside staged parent directories)
  const pathIndex = useMemo(() => {
    const m = new Map<string, { path: string; isDir: boolean; size: number; name: string }>();
    for (const [fp, d] of dirMap.entries()) {
      if (fp) m.set(fp, { path: fp, isDir: true, size: d.size, name: d.name });
    }
    for (const f of files) {
      m.set(f.relativePath, { path: f.relativePath, isDir: false, size: f.size, name: f.name });
    }
    return m;
  }, [dirMap, files]);

  const effectiveStagedItems = useMemo(() => {
    const sorted = Array.from(stagedPaths).sort((a, b) => a.length - b.length);
    const nonOverlapping: string[] = [];
    for (const p of sorted) {
      const covered = nonOverlapping.some((parent) => p === parent || p.startsWith(`${parent}/`));
      if (!covered) nonOverlapping.push(p);
    }
    return nonOverlapping
      .map((p) => pathIndex.get(p))
      .filter((x): x is { path: string; isDir: boolean; size: number; name: string } => Boolean(x));
  }, [stagedPaths, pathIndex]);

  const totalStagedBytes = useMemo(
    () => effectiveStagedItems.reduce((acc, i) => acc + i.size, 0),
    [effectiveStagedItems]
  );

  const toggleStage = (path: string) => {
    if (!path) return;
    setStagedPaths((prev) => {
      const next = new Set(prev);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      return next;
    });
  };

  // Build TreeSize flat visible hierarchy rows
  type TreeRow =
    | { rowType: 'dir'; depth: number; dir: LocalDirNode; isExpanded: boolean }
    | { rowType: 'file'; depth: number; file: LocalFileItem };

  const treeRows = useMemo(() => {
    const q = localFilter.trim().toLowerCase();
    const out: TreeRow[] = [];

    const visit = (dir: LocalDirNode, depth: number) => {
      for (const sd of dir.subdirs) {
        if (q && !sd.name.toLowerCase().includes(q) && !sd.fullPath.toLowerCase().includes(q)) {
          const hasMatchingFile = files.some(
            (f) => f.relativePath.startsWith(`${sd.fullPath}/`) && f.relativePath.toLowerCase().includes(q)
          );
          if (!hasMatchingFile) continue;
        }
        const isExpanded = expandedFolders.has(sd.fullPath) || Boolean(q && depth < 2);
        out.push({ rowType: 'dir', depth, dir: sd, isExpanded });
        if (isExpanded && depth < 6) {
          visit(sd, depth + 1);
        }
      }
      for (const f of dir.files) {
        if (q && !f.nameNorm.includes(q) && !f.relativePath.toLowerCase().includes(q)) continue;
        out.push({ rowType: 'file', depth, file: f });
      }
    };

    visit(activeDirNode, 0);
    return out;
  }, [activeDirNode, expandedFolders, localFilter, files]);

  // Mode 2: Largest & Stale Files
  const largestStaleFiles = useMemo(() => {
    const now = Date.now();
    const minBytes = minSizeFilterMb * MB;
    const minAgeMs = minAgeDaysFilter * 86400 * 1000;
    const q = localFilter.trim().toLowerCase();

    return files
      .filter((f) => {
        if (f.size < minBytes) return false;
        if (minAgeMs > 0 && now - f.lastModifiedMs < minAgeMs) return false;
        if (q && !f.nameNorm.includes(q) && !f.relativePath.toLowerCase().includes(q)) return false;
        return true;
      })
      .sort((a, b) => b.size - a.size)
      .slice(0, 200);
  }, [files, minSizeFilterMb, minAgeDaysFilter, localFilter]);

  // Mode 3: Bloat & Caches
  const bloatBreakdown = useMemo(() => {
    const groups: Record<
      BloatCategory,
      { id: BloatCategory; bytes: number; items: LocalFileItem[]; folderTargets: Set<string> }
    > = {
      dev: { id: 'dev', bytes: 0, items: [], folderTargets: new Set() },
      cache: { id: 'cache', bytes: 0, items: [], folderTargets: new Set() },
      installers: { id: 'installers', bytes: 0, items: [], folderTargets: new Set() },
      archives: { id: 'archives', bytes: 0, items: [], folderTargets: new Set() },
    };

    for (const f of files) {
      const cat = classifyBloatItem(f);
      if (!cat) continue;
      groups[cat].bytes += f.size;
      groups[cat].items.push(f);
      if (cat === 'dev') {
        const segs = f.relativePath.split('/');
        const devIdx = segs.findIndex((s) =>
          ['node_modules', '.venv', '__pycache__', 'target', '.next', 'dist', 'build', 'deriveddata'].includes(
            s.toLowerCase()
          )
        );
        if (devIdx >= 0) groups[cat].folderTargets.add(segs.slice(0, devIdx + 1).join('/'));
      }
    }

    return Object.values(groups).sort((a, b) => b.bytes - a.bytes);
  }, [files]);

  // Mode 4: metadata candidate and comparison groups; no file content is read.
  const duplicateGroups = useMemo(() => {
    const q = localFilter.trim().toLowerCase();
    const map = new Map<string, LocalFileItem[]>();

    for (const f of files) {
      if (f.size <= 0) continue;
      if (q && !f.nameNorm.includes(q) && !f.relativePath.toLowerCase().includes(q)) continue;

      if (dupCriterion === 'size_renamed') {
        if (f.size < MB) continue; // avoid tiny config collisions
        const key = `sz::${f.size}::${f.ext}`;
        const list = map.get(key) ?? [];
        list.push(f);
        map.set(key, list);
      } else if (dupCriterion === 'same_name_diff_size') {
        const key = `nm::${f.nameNorm}`;
        const list = map.get(key) ?? [];
        list.push(f);
        map.set(key, list);
      } else {
        const key = `candidate::${metadataCandidateKey(f.name, f.size)}`;
        const list = map.get(key) ?? [];
        list.push(f);
        map.set(key, list);
      }
    }

    const groups: {
      key: string;
      title: string;
      size: number;
      candidateBytes: number;
      items: LocalFileItem[];
    }[] = [];

    for (const [key, list] of map.entries()) {
      if (list.length < 2) continue;

      if (dupCriterion === 'size_renamed') {
        const distinctNames = new Set(list.map((i) => i.nameNorm));
        if (distinctNames.size < 2) continue; // only groups where at least 1 copy was renamed
      } else if (dupCriterion === 'same_name_diff_size') {
        const distinctSizes = new Set(list.map((i) => i.size));
        if (distinctSizes.size < 2) continue; // only groups where byte sizes differ
      }

      const sorted = [...list].sort((a, b) => b.size - a.size || a.relativePath.length - b.relativePath.length);
      const candidateBytes = sorted.reduce((acc, item) => acc + item.size, 0);

      groups.push({
        key,
        title:
          dupCriterion === 'size_renamed'
            ? sorted.map((s) => s.name).slice(0, 2).join(' ≠ ')
            : sorted[0].name,
        size: sorted[0].size,
        candidateBytes,
        items: sorted,
      });
    }

    return groups.sort((a, b) => b.candidateBytes - a.candidateBytes);
  }, [files, dupCriterion, localFilter]);

  const totalCandidateBytes = useMemo(
    () => duplicateGroups.reduce((acc, g) => acc + g.candidateBytes, 0),
    [duplicateGroups]
  );

  // Mode 6: Baseline Diff ("What grew?")
  const handleSaveBaseline = () => {
    if (!rootLabel || files.length === 0) return;
    const folders: Record<string, number> = {};
    for (const [fp, node] of dirMap.entries()) {
      folders[fp || '__root__'] = node.size;
    }
    const payload: BaselineStore = {
      sourceSessionId: sourceSessionIdRef.current,
      savedAtMs: Date.now(),
      folders,
    };
    setBaseline(payload);
    onNotify(
      t(
        `База размеров «${rootLabel}» запомнена только до выбора другой папки или закрытия вкладки.`,
        `Folder-size baseline for "${rootLabel}" is kept for this source session only.`
      ),
      'success'
    );
  };

  const diffRows = useMemo(() => {
    if (!baseline || baseline.sourceSessionId !== sourceSessionIdRef.current) return [];
    const allKeys = new Set<string>([
      ...Object.keys(baseline.folders),
      ...Array.from(dirMap.keys()).map((k) => k || '__root__'),
    ]);
    const rows: { path: string; prevBytes: number; currBytes: number; deltaBytes: number }[] = [];
    for (const key of allKeys) {
      const prevBytes = baseline.folders[key] ?? 0;
      const currNode = dirMap.get(key === '__root__' ? '' : key);
      const currBytes = currNode?.size ?? 0;
      const deltaBytes = currBytes - prevBytes;
      rows.push({
        path: key === '__root__' ? `/${rootLabel}` : `/${key}`,
        prevBytes,
        currBytes,
        deltaBytes,
      });
    }
    return rows.sort((a, b) => Math.abs(b.deltaBytes) - Math.abs(a.deltaBytes));
  }, [baseline, dirMap, rootLabel]);

  const triggerDownload = (filename: string, content: string, mime = 'text/plain;charset=utf-8') => {
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = filename;
    document.body.appendChild(a);
    a.click();
    a.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
  };

  // Release-safety contract: this workbench NEVER emits executable cleanup scripts
  // and NEVER deletes the user's files. Metadata-only matches are not backups.
  const handleExportReviewCsv = () => {
    if (effectiveStagedItems.length === 0) return;
    const rows = effectiveStagedItems.map((item) =>
      [item.isDir ? 'DIR' : 'FILE', item.path, String(item.size), formatBytes(item.size)]
        .map(escapeReviewCsvCell).join(',')
    );
    triggerDownload('whereisit-review-candidates.csv',
      ['Kind,RelativePath,SizeBytes,SizeHuman', ...rows].join('\n'),
      'text/csv;charset=utf-8');
    onNotify(t('Список кандидатов сохранён для ручной проверки. CSV не выполняет действий с файлами.',
      'Review list exported for manual review. The CSV does not perform file operations.'), 'info');
  };

  const usbCandidateSummary = useMemo(() => {
    let candidateFiles = 0;
    let candidateBytes = 0;
    let noCandidateFiles = 0;
    let noCandidateBytes = 0;
    for (const f of files) {
      if ((usbMatches[f.relativePath]?.length ?? 0) > 0) {
        candidateFiles++;
        candidateBytes += f.size;
      } else {
        noCandidateFiles++;
        noCandidateBytes += f.size;
      }
    }
    return { candidateFiles, candidateBytes, noCandidateFiles, noCandidateBytes };
  }, [files, usbMatches]);

  const bloatLabels: Record<BloatCategory, { title: string; desc: string }> = {
    dev: {
      title: t('Зависимости и сборки (node_modules, .venv, dist, target)', 'Build & Dev Bloat (node_modules, .venv, dist, target)'),
      desc: t('Восстанавливаемые артефакты сборки и пакеты разработки', 'Reproducible package caches and build artifacts'),
    },
    cache: {
      title: t('Кэши, временные файлы и логи (.cache, .tmp, .log)', 'Caches, Temp & Logs (.cache, .tmp, .log)'),
      desc: t('Временные файлы программ, кэши Docker/IDE/браузеров', 'Transient caches, dumps, Docker/IDE scratch data'),
    },
    installers: {
      title: t('Установщики и образы (.dmg, .pkg, .exe, .iso)', 'Installers & Disk Images (.dmg, .pkg, .exe, .iso)'),
      desc: t('Инсталляторы, забытые в Загрузках после установки', 'Downloaded installers left behind after setup'),
    },
    archives: {
      title: t('Крупные архивы > 50 MB (.zip, .tar, .7z, .rar)', 'Large Archives > 50 MB (.zip, .tar, .7z, .rar)'),
      desc: t('Сжатые архивы, возможно уже распакованные рядом', 'Compressed bundles that may already be extracted'),
    },
  };

  const activeHoverDisplay = hoveredArc
    ? { name: hoveredArc.name, size: hoveredArc.size, count: hoveredArc.fileCount }
    : { name: activeDirNode.name, size: activeDirNode.size, count: activeDirNode.fileCount };
  const activeSharePct =
    activeDirNode.size > 0 ? Math.min(100, (activeHoverDisplay.size / activeDirNode.size) * 100) : 100;

  return (
    <div className="flex flex-col gap-2.5">
      {/* Top Bar: Clear Single Primary Action + Ephemeral Guarantee */}
      <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 px-3.5 py-2.5 flex flex-wrap items-center justify-between gap-2.5">
        <div className="flex flex-wrap items-center gap-2">
          <label className={`inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded-md bg-blue-600 text-white text-xs font-medium hover:bg-blue-500 transition-colors whitespace-nowrap shrink-0 ${scanning ? 'opacity-60 pointer-events-none' : 'cursor-pointer'}`}>
            <FolderUp className="w-3.5 h-3.5" />
            <span>
              {scanning
                ? t(`Чтение (${scanCount})…`, `Reading (${scanCount})…`)
                : t('Открыть папку / диск ПК', 'Open PC Folder / Drive')}
            </span>
            <input
              type="file"
              // @ts-expect-error webkitdirectory lets user pick any local folder or mounted drive
              webkitdirectory=""
              directory=""
              multiple
              disabled={scanning}
              onChange={handleWebkitDirectoryPick}
              className="hidden"
            />
          </label>

          {canUseDirectFsApi && (
            <button
              type="button"
              onClick={() => void handleFsApiPick()}
              disabled={scanning}
              className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-xs font-medium text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 cursor-pointer"
            >
              <FolderOpen className="w-3.5 h-3.5 text-blue-500" />
              {t('Прямой доступ: только чтение', 'Direct FS Access (Read-only)')}
            </button>
          )}
          {scanning && (
            <button type="button" onClick={cancelLocalScan} className="px-2.5 py-1.5 rounded-md border border-rose-400 text-rose-600 dark:text-rose-400 text-xs">
              {t(`Остановить чтение (${scanCount})`, `Stop reading (${scanCount})`)}
            </button>
          )}
        </div>

        <div className="flex flex-wrap items-center gap-3 text-xs">
          <span className="inline-flex items-center gap-1 text-emerald-600 dark:text-emerald-400 font-medium">
            <ShieldCheck className="w-3.5 h-3.5" />
            {t('В архив 60 USB не сохраняется (только ОЗУ)', 'Never saved to 60-USB archive (RAM only)')}
          </span>
          <span className="font-mono tabular-nums text-slate-700 dark:text-slate-300">
            {sourceKind === 'sample'
              ? t('Демо-макет', 'Demo layout')
              : rootLabel}{' '}
            · {formatBytes(treeRoot.size)} · {treeRoot.fileCount.toLocaleString(numLocale)}{' '}
            {t('файл.', 'files')}
          </span>
        </div>
      </div>

      {/* Honest Demo Banner — disappears immediately once user selects a real folder */}
      {sourceKind === 'sample' && (
        <div className="px-3.5 py-2 rounded-lg border border-amber-300/80 dark:border-amber-800/70 bg-amber-50/80 dark:bg-amber-950/30 flex flex-wrap items-center justify-between gap-2 text-xs text-amber-950 dark:text-amber-200">
          <div className="flex items-center gap-2">
            <Info className="w-3.5 h-3.5 text-amber-600 dark:text-amber-400 shrink-0" />
            <span>
              {t(
                'Показан демонстрационный пример для обзора дерева, карты и списка кандидатов. Нажмите «Открыть папку / диск ПК», чтобы просмотреть метаданные выбранной папки.',
                'Showing a demo layout for the tree, map, and candidate review. Choose "Open PC Folder / Drive" to inspect metadata from a folder you select.'
              )}
            </span>
          </div>
        </div>
      )}

      {/* Mode Switcher + Secondary Utilities */}
      <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 px-3 py-2 flex flex-wrap items-center justify-between gap-2 text-xs">
        <div className="inline-flex flex-wrap p-0.5 rounded-md bg-slate-100 dark:bg-slate-800 gap-0.5">
          {(
            [
              ['tree_map', t('Дерево + Карта', 'Tree + Sunburst')],
              ['bloat', t('Кэши и сборки', 'Caches & Builds')],
              ['largest', t('Крупные и старые', 'Heavy & Stale')],
              ['duplicates', `${t('Кандидаты по метаданным', 'Metadata Candidates')} (${duplicateGroups.length})`],
              ['backup', t('Кандидаты на USB', 'USB Candidates')],
              ['diff', t('Рост папок', 'Growth Diff')],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              onClick={() => setActiveMode(id)}
              className={`px-2.5 py-1 rounded font-medium whitespace-nowrap shrink-0 cursor-pointer transition-colors ${
                activeMode === id
                  ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white shadow-xs'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
              }`}
            >
              {label}
            </button>
          ))}
        </div>

        <div className="flex flex-wrap items-center gap-1.5">
          <input
            type="text"
            value={localFilter}
            onChange={(e) => setLocalFilter(e.target.value)}
            placeholder={t('Фильтр пути/имени…', 'Filter path or name…')}
            className="w-40 px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          <button
            type="button"
            onClick={() => void runUsbBackupMatch(files, false)}
            disabled={checkingUsb || files.length === 0}
            className="inline-flex items-center gap-1 px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-40 cursor-pointer whitespace-nowrap"
          >
            <HardDrive className="w-3.5 h-3.5 text-teal-500" />
            {checkingUsb
              ? t(`Кандидаты ${usbCheckProgress?.processed ?? 0}/${usbCheckProgress?.total ?? files.length}`, `Candidates ${usbCheckProgress?.processed ?? 0}/${usbCheckProgress?.total ?? files.length}`)
              : t('Искать на USB', 'Find on USB')}
          </button>
          {checkingUsb && (
            <button type="button" onClick={cancelUsbMatch} className="text-xs text-rose-600 dark:text-rose-400 hover:underline">
              {t('Отменить', 'Cancel')}
            </button>
          )}
          <button
            type="button"
            onClick={handleSaveBaseline}
            disabled={files.length === 0}
            className="inline-flex items-center gap-1 px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 disabled:opacity-40 cursor-pointer whitespace-nowrap"
          >
            <History className="w-3.5 h-3.5 text-blue-500" />
            {t('Запомнить базу', 'Save Baseline')}
          </button>
        </div>
      </div>

      {/* Mode 1: Combined DaisyDisk Sunburst + TreeSize Collapsible Tree */}
      {activeMode === 'tree_map' && (
        <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 overflow-hidden">
          <div className="px-3.5 py-2 border-b border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-900/50 flex flex-wrap items-center justify-between gap-2 text-xs">
            <div className="flex flex-wrap items-center gap-1">
              {breadcrumbs.map((b, i) => {
                const isLast = i === breadcrumbs.length - 1;
                return (
                  <React.Fragment key={b.path || 'root'}>
                    {i > 0 && <ChevronRight className="w-3.5 h-3.5 text-slate-400" />}
                    <button
                      type="button"
                      onClick={() => setCurrentFolderPath(b.path)}
                      className={`px-2 py-0.5 rounded cursor-pointer whitespace-nowrap ${
                        isLast
                          ? 'bg-slate-200/80 dark:bg-slate-800 font-semibold text-slate-900 dark:text-white'
                          : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                      }`}
                    >
                      {b.label}
                    </button>
                  </React.Fragment>
                );
              })}
            </div>
            {currentFolderPath && (
              <button
                type="button"
                onClick={() => {
                  const idx = currentFolderPath.lastIndexOf('/');
                  setCurrentFolderPath(idx >= 0 ? currentFolderPath.slice(0, idx) : '');
                }}
                className="inline-flex items-center gap-1 text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
              >
                <ArrowUpLeft className="w-3.5 h-3.5" />
                {t('Уровень выше', 'Up level')}
              </button>
            )}
          </div>

          <div className="grid grid-cols-1 xl:grid-cols-12 divide-y xl:divide-y-0 xl:divide-x divide-slate-200 dark:divide-slate-800">
            {/* Left 5 Cols: Interactive DaisyDisk Sunburst */}
            <div className="xl:col-span-5 p-3 flex flex-col items-center justify-center min-h-[420px]">
              <div className="relative w-full max-w-[420px] aspect-square flex items-center justify-center">
                <svg
                  viewBox="0 0 520 520"
                  className="w-full h-full select-none overflow-visible"
                  onMouseLeave={() => {
                    setHoveredArc(null);
                    setHoveredAncestors(new Set());
                  }}
                >
                  {RING_RADII.map(([rIn, rOut], idx) => (
                    <circle
                      key={idx}
                      cx={260}
                      cy={260}
                      r={(rIn + rOut) / 2}
                      fill="none"
                      stroke="currentColor"
                      strokeWidth={rOut - rIn}
                      className="text-slate-100/70 dark:text-slate-900/50"
                    />
                  ))}
                  {arcs.map((arc) => {
                    const isHover = hoveredAncestors.has(arc.id);
                    const isStaged = arc.fullPath ? stagedPaths.has(arc.fullPath) : false;
                    return (
                      <path
                        key={arc.id}
                        d={arc.pathD}
                        fill={arc.fill}
                        opacity={hoveredArc && !isHover ? 0.42 : 0.94}
                        stroke={isStaged ? '#F59E0B' : '#0F172A'}
                        strokeOpacity={isStaged ? 1 : 0.25}
                        strokeWidth={isStaged ? 2 : isHover ? 1.5 : 0.7}
                        className="transition-opacity duration-100 cursor-pointer"
                        onMouseEnter={() => {
                          setHoveredArc(arc);
                          setHoveredAncestors(new Set(arc.ancestorIds));
                        }}
                        onClick={() => {
                          if (arc.kind === 'directory') {
                            setCurrentFolderPath(arc.fullPath);
                          } else if (arc.kind === 'file') {
                            toggleStage(arc.fullPath);
                          }
                        }}
                      />
                    );
                  })}

                  <circle
                    cx={260}
                    cy={260}
                    r={76}
                    onClick={() => {
                      if (!currentFolderPath) return;
                      const idx = currentFolderPath.lastIndexOf('/');
                      setCurrentFolderPath(idx >= 0 ? currentFolderPath.slice(0, idx) : '');
                    }}
                    className={`fill-white dark:fill-slate-950 stroke-slate-200 dark:stroke-slate-800 ${
                      currentFolderPath ? 'cursor-pointer hover:fill-slate-50 dark:hover:fill-slate-900' : ''
                    }`}
                    strokeWidth={2}
                  />
                </svg>

                <div className="pointer-events-none absolute inset-0 flex flex-col items-center justify-center text-center px-16">
                  <div className="text-[11px] font-medium text-slate-500 dark:text-slate-400 truncate max-w-[130px]">
                    {activeHoverDisplay.name}
                  </div>
                  <div className="font-display text-lg font-bold font-mono tabular-nums text-slate-900 dark:text-white leading-tight">
                    {formatBytes(activeHoverDisplay.size)}
                  </div>
                  <div className="text-[11px] font-mono tabular-nums text-slate-500">
                    {activeSharePct.toFixed(1)}% · {activeHoverDisplay.count} {t('файл.', 'files')}
                  </div>
                  {currentFolderPath && !hoveredArc && (
                    <div className="mt-1 inline-flex items-center gap-1 text-[10px] font-medium text-blue-600 dark:text-blue-400">
                      <ArrowUpLeft className="w-3 h-3" />
                      {t('Клик — назад', 'Click — up')}
                    </div>
                  )}
                </div>
              </div>
              <div className="text-[11px] text-slate-500 text-center">
                {t(
                  'Клик по папке — войти внутрь · Клик по файлу — добавить в список проверки',
                  'Click a folder to open it · Click a file to add it to the review list'
                )}
              </div>
            </div>

            {/* Right 7 Cols: TreeSize-style Expandable Folder & File Size Table */}
            <div className="xl:col-span-7 overflow-auto max-h-[560px]">
              <table className="w-full text-left border-collapse text-xs">
                <thead className="sticky top-0 z-10 bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-400">
                  <tr>
                    <th className="py-1.5 px-2.5 font-medium">{t('Папка / Файл', 'Folder / File')}</th>
                    <th className="py-1.5 px-2 font-medium text-right whitespace-nowrap">{t('Доля', 'Share')}</th>
                    <th className="py-1.5 px-2.5 font-medium text-right whitespace-nowrap">{t('Объём', 'Size')}</th>
                    <th className="py-1.5 px-2 font-medium text-right whitespace-nowrap hidden sm:table-cell">{t('Файлов', 'Files')}</th>
                    <th className="py-1.5 px-2 font-medium whitespace-nowrap hidden md:table-cell">{t('Кандидаты на USB', 'USB Candidates')}</th>
                    <th className="py-1.5 px-2 font-medium text-right whitespace-nowrap">{t('В план', 'Stage')}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200/60 dark:divide-slate-800/60">
                  {treeRows.map((r) => {
                    if (r.rowType === 'dir') {
                      const d = r.dir;
                      const pct = activeDirNode.size > 0 ? (d.size / activeDirNode.size) * 100 : 0;
                      const isStaged = stagedPaths.has(d.fullPath);
                      const bStats = folderCandidateStats.get(d.fullPath);
                      const hasAnyCandidate = (bStats?.backedBytes ?? 0) > 0;
                      const allHaveCandidates = hasAnyCandidate && (bStats?.uniqueBytes ?? 0) === 0;

                      return (
                        <tr
                          key={d.id}
                          className={`hover:bg-slate-50 dark:hover:bg-slate-900/60 ${
                            isStaged ? 'bg-amber-50/70 dark:bg-amber-950/20' : ''
                          }`}
                        >
                          <td className="py-1.5 px-2.5 max-w-[260px]">
                            <div
                              className="flex items-center gap-1 min-w-0"
                              style={{ paddingLeft: `${r.depth * 14}px` }}
                            >
                              <button
                                type="button"
                                onClick={() =>
                                  setExpandedFolders((prev) => {
                                    const next = new Set(prev);
                                    if (next.has(d.fullPath)) next.delete(d.fullPath);
                                    else next.add(d.fullPath);
                                    return next;
                                  })
                                }
                                className="p-0.5 text-slate-400 hover:text-slate-800 dark:hover:text-white cursor-pointer shrink-0"
                              >
                                {r.isExpanded ? (
                                  <ChevronDown className="w-3.5 h-3.5" />
                                ) : (
                                  <ChevronRight className="w-3.5 h-3.5" />
                                )}
                              </button>
                              <button
                                type="button"
                                onClick={() => setCurrentFolderPath(d.fullPath)}
                                className="font-medium text-slate-900 dark:text-slate-100 hover:text-blue-600 dark:hover:text-blue-400 truncate text-left cursor-pointer"
                                title={d.fullPath}
                              >
                                {d.name}/
                              </button>
                            </div>
                          </td>

                          <td className="py-1.5 px-2 text-right font-mono tabular-nums whitespace-nowrap">
                            <div className="inline-flex items-center gap-1.5 justify-end">
                              <div className="w-12 h-1.5 rounded-full bg-slate-200 dark:bg-slate-800 overflow-hidden hidden sm:block">
                                <div
                                  className="h-full rounded-full bg-blue-600 dark:bg-blue-500"
                                  style={{ width: `${Math.max(3, pct)}%` }}
                                />
                              </div>
                              <span className="text-slate-500 w-10">{pct.toFixed(1)}%</span>
                            </div>
                          </td>

                          <td className="py-1.5 px-2.5 text-right font-mono font-semibold tabular-nums text-slate-900 dark:text-white whitespace-nowrap">
                            {formatBytes(d.size)}
                          </td>

                          <td className="py-1.5 px-2 text-right font-mono tabular-nums text-slate-500 hidden sm:table-cell">
                            {d.fileCount}
                          </td>

                          <td className="py-1.5 px-2 text-[11px] whitespace-nowrap hidden md:table-cell">
                            {!usbCheckedAt ? (
                              <span className="text-slate-400">{checkingUsb ? t('Поиск…', 'Searching…') : t('Не проверено', 'Not checked')}</span>
                            ) : allHaveCandidates ? (
                              <span className="text-emerald-600 dark:text-emerald-400">
                                {t('Есть кандидаты для всех файлов', 'Candidates for all files')}
                              </span>
                            ) : hasAnyCandidate ? (
                              <span className="text-blue-600 dark:text-blue-400">
                                {t('Частичные совпадения-кандидаты', 'Some metadata candidates')} ({formatBytes(bStats!.backedBytes)})
                              </span>
                            ) : (
                              <span className="text-slate-400">—</span>
                            )}
                          </td>

                          <td className="py-1.5 px-2 text-right whitespace-nowrap">
                            <button
                              type="button"
                              onClick={() => toggleStage(d.fullPath)}
                              className={`inline-flex items-center gap-1 px-2 py-0.5 rounded border text-[11px] cursor-pointer ${
                                isStaged
                                  ? 'border-amber-500 bg-amber-500 text-slate-950 font-semibold'
                                  : 'border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800'
                              }`}
                            >
                              {isStaged ? <Check className="w-3 h-3" /> : <Plus className="w-3 h-3" />}
                              {isStaged ? t('В плане', 'Staged') : t('Выбрать', 'Stage')}
                            </button>
                          </td>
                        </tr>
                      );
                    }

                    const f = r.file;
                    const pct = activeDirNode.size > 0 ? (f.size / activeDirNode.size) * 100 : 0;
                    const isStaged = stagedPaths.has(f.relativePath);
                    const matches = usbMatches[f.relativePath] ?? [];

                    return (
                      <tr
                        key={f.id}
                        className={`hover:bg-slate-50 dark:hover:bg-slate-900/60 ${
                          isStaged ? 'bg-amber-50/70 dark:bg-amber-950/20' : ''
                        }`}
                      >
                        <td className="py-1.5 px-2.5 max-w-[260px]">
                          <div
                            className="flex items-center gap-1.5 min-w-0"
                            style={{ paddingLeft: `${r.depth * 14 + 18}px` }}
                          >
                            <span className="text-slate-700 dark:text-slate-300 truncate" title={f.relativePath}>
                              {f.name}
                            </span>
                          </div>
                        </td>
                        <td className="py-1.5 px-2 text-right font-mono tabular-nums text-slate-500">
                          {pct.toFixed(1)}%
                        </td>
                        <td
                          className="py-1.5 px-2.5 text-right font-mono tabular-nums text-slate-900 dark:text-slate-100 whitespace-nowrap"
                          title={formatExactBytes(f.size, lang)}
                        >
                          {formatBytes(f.size)}
                        </td>
                        <td className="py-1.5 px-2 text-right font-mono text-slate-400 hidden sm:table-cell">1</td>
                        <td className="py-1.5 px-2 text-[11px] font-mono whitespace-nowrap hidden md:table-cell">
                          {matches.length > 0 ? (
                            <span className="text-emerald-600 dark:text-emerald-400">
                              {t('Кандидат:', 'Candidate:')} USB #{matches.map((m) => m.volumeId).join(', #')}
                            </span>
                          ) : (
                            <span className="text-amber-600 dark:text-amber-400">{!usbCheckedAt ? (checkingUsb ? t('Поиск…', 'Searching…') : t('Не проверено', 'Not checked')) : t('Кандидат не найден', 'No metadata candidate')}</span>
                          )}
                        </td>
                        <td className="py-1.5 px-2 text-right whitespace-nowrap">
                          <button
                            type="button"
                            onClick={() => toggleStage(f.relativePath)}
                            className={`inline-flex items-center gap-1 px-2 py-0.5 rounded border text-[11px] cursor-pointer ${
                              isStaged
                                ? 'border-amber-500 bg-amber-500 text-slate-950 font-semibold'
                                : 'border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800'
                            }`}
                          >
                            {isStaged ? <Check className="w-3 h-3" /> : <Plus className="w-3 h-3" />}
                            {isStaged ? t('В плане', 'Staged') : t('Выбрать', 'Stage')}
                          </button>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* Mode 2: Largest & Stale Files */}
      {activeMode === 'largest' && (
        <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 overflow-hidden">
          <div className="px-3.5 py-2 border-b border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-900/50 flex flex-wrap items-center justify-between gap-2 text-xs">
            <div className="flex flex-wrap items-center gap-2">
              <span className="text-slate-500">{t('Мин. размер:', 'Min size:')}</span>
              {[0, 10, 100, 1024].map((mb) => (
                <button
                  key={mb}
                  type="button"
                  onClick={() => setMinSizeFilterMb(mb)}
                  className={`px-2 py-0.5 rounded font-mono cursor-pointer ${
                    minSizeFilterMb === mb
                      ? 'bg-blue-600 text-white font-semibold'
                      : 'bg-slate-200/70 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                  }`}
                >
                  {mb === 0 ? t('Все', 'All') : mb >= 1024 ? '> 1 GB' : `> ${mb} MB`}
                </button>
              ))}
              <span className="text-slate-500 ml-2">{t('Не менялись:', 'Stale age:')}</span>
              {[0, 90, 180, 365].map((days) => (
                <button
                  key={days}
                  type="button"
                  onClick={() => setMinAgeDaysFilter(days)}
                  className={`px-2 py-0.5 rounded font-mono cursor-pointer ${
                    minAgeDaysFilter === days
                      ? 'bg-blue-600 text-white font-semibold'
                      : 'bg-slate-200/70 dark:bg-slate-800 text-slate-700 dark:text-slate-300'
                  }`}
                >
                  {days === 0 ? t('Любые', 'Any') : `> ${days}d`}
                </button>
              ))}
            </div>
            <span className="font-mono tabular-nums text-slate-500">
              {largestStaleFiles.length} {t('объектов', 'items')}
            </span>
          </div>

          <div className="overflow-auto max-h-[520px]">
            <table className="w-full text-left border-collapse text-xs">
              <thead className="sticky top-0 bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-400">
                <tr>
                  <th className="py-1.5 px-3 font-medium">{t('Файл и путь', 'File & Path')}</th>
                  <th className="py-1.5 px-2.5 font-medium text-right whitespace-nowrap">{t('Размер', 'Size')}</th>
                  <th className="py-1.5 px-2.5 font-medium text-right whitespace-nowrap">{t('Возраст', 'Age')}</th>
                  <th className="py-1.5 px-2.5 font-medium whitespace-nowrap">{t('Кандидаты на USB', 'USB Candidates')}</th>
                  <th className="py-1.5 px-3 font-medium text-right whitespace-nowrap">{t('В план', 'Stage')}</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-200/60 dark:divide-slate-800/60">
                {largestStaleFiles.map((f) => {
                  const daysOld = Math.max(0, Math.floor((Date.now() - f.lastModifiedMs) / (86400 * 1000)));
                  const isStaged = stagedPaths.has(f.relativePath);
                  const matches = usbMatches[f.relativePath] ?? [];
                  return (
                    <tr key={f.id} className="hover:bg-slate-50 dark:hover:bg-slate-900/50">
                      <td className="py-1.5 px-3 max-w-[380px]">
                        <div className="font-medium text-slate-900 dark:text-slate-100 truncate">{f.name}</div>
                        <div className="font-mono text-[11px] text-slate-500 truncate">{f.relativePath}</div>
                      </td>
                      <td className="py-1.5 px-2.5 text-right font-mono font-semibold tabular-nums text-slate-900 dark:text-white whitespace-nowrap">
                        {formatBytes(f.size)}
                      </td>
                      <td className="py-1.5 px-2.5 text-right font-mono tabular-nums text-slate-600 dark:text-slate-400 whitespace-nowrap">
                        {daysOld} {t('дн.', 'd')}
                      </td>
                      <td className="py-1.5 px-2.5 font-mono text-[11px] whitespace-nowrap">
                        {matches.length > 0 ? (
                          <span className="text-emerald-600 dark:text-emerald-400">
                            {t('Кандидат:', 'Candidate:')} USB #{matches.map((m) => m.volumeId).join(', #')}
                          </span>
                        ) : (
                          <span className="text-amber-600 dark:text-amber-400">{!usbCheckedAt ? (checkingUsb ? t('Поиск…', 'Searching…') : t('Не проверено', 'Not checked')) : t('Кандидат не найден', 'No metadata candidate')}</span>
                        )}
                      </td>
                      <td className="py-1.5 px-3 text-right whitespace-nowrap">
                        <button
                          type="button"
                          onClick={() => toggleStage(f.relativePath)}
                          className={`inline-flex items-center gap-1 px-2 py-0.5 rounded border text-[11px] cursor-pointer ${
                            isStaged
                              ? 'border-amber-500 bg-amber-500 text-slate-950 font-semibold'
                              : 'border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300'
                          }`}
                        >
                          {isStaged ? t('В плане', 'Staged') : t('Выбрать', 'Stage')}
                        </button>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Mode 3: Bloat, Build Caches & Installers */}
      {activeMode === 'bloat' && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {bloatBreakdown.map((grp) => {
            const meta = bloatLabels[grp.id];
            const stageTargets =
              grp.folderTargets.size > 0
                ? Array.from(grp.folderTargets)
                : grp.items.map((i) => i.relativePath);
            return (
              <section
                key={grp.id}
                className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 p-3.5 flex flex-col gap-2"
              >
                <div className="flex items-start justify-between gap-2">
                  <div>
                    <h3 className="text-xs sm:text-sm font-semibold text-slate-900 dark:text-white">{meta.title}</h3>
                    <p className="text-[11px] text-slate-500">{meta.desc}</p>
                  </div>
                  <div className="text-right font-mono tabular-nums shrink-0">
                    <div className="text-sm font-bold text-blue-600 dark:text-blue-400">{formatBytes(grp.bytes)}</div>
                    <div className="text-[11px] text-slate-500">
                      {grp.items.length} {t('файл.', 'files')}
                    </div>
                  </div>
                </div>

                <div className="divide-y divide-slate-200/60 dark:divide-slate-800/60 text-xs max-h-44 overflow-auto">
                  {grp.items
                    .slice()
                    .sort((a, b) => b.size - a.size)
                    .slice(0, 12)
                    .map((item) => {
                      const isStaged = stagedPaths.has(item.relativePath);
                      return (
                        <div key={item.id} className="py-1 flex items-center justify-between gap-2">
                          <span className="font-mono text-[11px] text-slate-700 dark:text-slate-300 truncate">
                            {item.relativePath}
                          </span>
                          <div className="flex items-center gap-2 shrink-0">
                            <span className="font-mono tabular-nums text-slate-900 dark:text-white">
                              {formatBytes(item.size)}
                            </span>
                            <button
                              type="button"
                              onClick={() => toggleStage(item.relativePath)}
                              className={`px-1.5 py-0.5 rounded border text-[11px] cursor-pointer ${
                                isStaged
                                  ? 'border-amber-500 bg-amber-500 text-slate-950 font-semibold'
                                  : 'border-slate-300 dark:border-slate-700 text-slate-600 dark:text-slate-400'
                              }`}
                            >
                              {isStaged ? '✓' : '+'}
                            </button>
                          </div>
                        </div>
                      );
                    })}
                  {grp.items.length === 0 && (
                    <div className="py-3 text-center text-slate-500">{t('Ничего не найдено', 'None detected')}</div>
                  )}
                </div>

                {stageTargets.length > 0 && (
                  <button
                    type="button"
                    onClick={() =>
                      setStagedPaths((prev) => {
                        const next = new Set(prev);
                        for (const tPath of stageTargets) next.add(tPath);
                        return next;
                      })
                    }
                    className="mt-auto px-3 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-slate-50 dark:bg-slate-900 hover:bg-slate-100 dark:hover:bg-slate-800 text-xs font-medium text-slate-800 dark:text-slate-200 cursor-pointer"
                  >
                    {t(`Добавить всю группу (${formatBytes(grp.bytes)})`, `Stage entire group (${formatBytes(grp.bytes)})`)}
                  </button>
                )}
              </section>
            );
          })}
        </div>
      )}

      {/* Mode 4: Local metadata candidates */}
      {activeMode === 'duplicates' && (
        <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 overflow-hidden">
          <div className="px-3.5 py-2.5 border-b border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-900/50 flex flex-col gap-2 text-xs">
            <div className="flex flex-wrap items-center justify-between gap-2">
              {/* Criterion Switcher */}
              <div className="inline-flex flex-wrap p-0.5 rounded bg-slate-200/70 dark:bg-slate-800 gap-0.5">
                {(
                  [
                    ['name_size', t('Имя + размер', 'Name + Size')],
                    ['size_renamed', t('Размер совпал, имена разные', 'Same size, different names')],
                    ['same_name_diff_size', t('Одно имя, размеры разные', 'Same name, different sizes')],
                  ] as const
                ).map(([crit, lbl]) => (
                  <button
                    key={crit}
                    type="button"
                    onClick={() => setDupCriterion(crit)}
                    className={`px-2.5 py-1 rounded font-medium cursor-pointer whitespace-nowrap ${
                      dupCriterion === crit
                        ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white shadow-xs'
                        : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
                    }`}
                  >
                    {lbl}
                  </button>
                ))}
              </div>

              <div className="flex flex-wrap items-center gap-2">
                <span className="font-mono tabular-nums text-blue-600 dark:text-blue-400 font-semibold">
                {duplicateGroups.length} {dupCriterion === 'name_size' ? t('групп-кандидатов', 'candidate groups') : t('групп для сравнения', 'comparison groups')} · {t('объём файлов в группах', 'bytes in groups')}{' '}
                  {formatBytes(totalCandidateBytes)}
                </span>
              </div>
            </div>

            <div className="text-[11px] text-slate-500">
              {dupCriterion === 'name_size'
                ? t('Совпадают нормализованное имя и точный размер в байтах. Это кандидаты по метаданным, не подтверждение одинакового содержимого.', 'Groups match NFC-normalized, trimmed, case-insensitive names and exact byte size. These are metadata candidates, not proof of equal content.')
                : dupCriterion === 'size_renamed'
                  ? t('Только одинаковый размер (≥ 1 МБ) и разные имена: слабый список для ручного просмотра, не копии.', 'Same size (≥ 1 MB) and different names only: a weak review list, not copy matches.')
                  : t('Одно нормализованное имя при разных размерах: сравнение вариантов, не копий.', 'Same normalized name with different sizes: version comparison, not copy matching.')}
            </div>
          </div>

          <div className="divide-y divide-slate-200 dark:divide-slate-800 max-h-[520px] overflow-auto">
            {duplicateGroups.length === 0 ? (
              <div className="py-12 text-center text-xs text-slate-500">
                {t('По выбранному правилу нет групп-кандидатов.', 'No candidate groups match this criterion.')}
              </div>
            ) : (
              duplicateGroups.map((grp) => {
                return (
                  <div key={grp.key} className="p-3 text-xs flex flex-col gap-1.5">
                    <div className="flex flex-wrap items-center justify-between gap-2">
                      <div className="flex flex-wrap items-center gap-2 min-w-0">
                        <Layers className="w-3.5 h-3.5 text-blue-500 shrink-0" />
                        <span className="font-semibold text-slate-900 dark:text-white truncate">{grp.title}</span>
                        <span className="font-mono text-slate-500">
                          · {grp.items.length} {t('файла', 'files')} · {formatExactBytes(grp.size, lang)} {dupCriterion === 'same_name_diff_size' ? t('(максимальный размер)', '(largest size)') : ''}
                        </span>
                      </div>

                      <div className="flex items-center gap-2 shrink-0">
                        <span className="font-mono font-semibold tabular-nums text-blue-600 dark:text-blue-400">
                          {formatBytes(grp.candidateBytes)}
                        </span>
                      </div>
                    </div>

                    <div className="pl-5 flex flex-col gap-1">
                      {grp.items.map((item, idx) => {
                        const isStaged = stagedPaths.has(item.relativePath);
                        return (
                          <div key={item.id} className="flex items-center justify-between gap-2">
                            <div className="flex items-center gap-2 min-w-0">
                              <span className="font-mono text-[11px] text-slate-600 dark:text-slate-400 truncate">
                                {item.relativePath}
                              </span>
                              {dupCriterion === 'same_name_diff_size' && (
                                <span className="font-mono text-[11px] text-slate-500 shrink-0">
                                  ({formatBytes(item.size)})
                                </span>
                              )}
                            </div>
                            <button
                              type="button"
                              onClick={() => toggleStage(item.relativePath)}
                              className={`px-2 py-0.5 rounded border text-[11px] cursor-pointer shrink-0 ${
                                isStaged
                                  ? 'border-amber-500 bg-amber-500 text-slate-950 font-semibold'
                                  : 'border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300'
                              }`}
                            >
                              {isStaged ? t('В плане', 'Staged') : t('Выбрать', 'Stage')}
                            </button>
                          </div>
                        );
                      })}
                    </div>
                  </div>
                );
              })
            )}
          </div>
        </div>
      )}

      {/* Mode 5: Read-Only USB metadata candidates */}
      {activeMode === 'backup' && (
        <div className="flex flex-col gap-3">
          <div className="grid grid-cols-1 sm:grid-cols-3 gap-3">
            <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 p-3">
              <div className="text-xs text-slate-500">{t('Файлы с кандидатом по имени и размеру', 'Files with name-and-size candidates')}</div>
              <div className="mt-1 font-display text-lg font-bold font-mono tabular-nums text-emerald-600 dark:text-emerald-400">
                {usbCheckedAt ? formatBytes(usbCandidateSummary.candidateBytes) : '—'}
              </div>
              <div className="text-[11px] font-mono text-slate-500">
                {usbCheckedAt
                  ? `${usbCandidateSummary.candidateFiles} ${t('файл.; равенство содержимого и резерв не установлены', 'files; content equality and backup are unverified')}`
                  : checkingUsb ? t('Поиск…', 'Searching…') : t('Не проверено', 'Not checked')}
              </div>
            </div>

            <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 p-3">
              <div className="text-xs text-slate-500">{t('Без найденного совпадения', 'No metadata match found')}</div>
              <div className="mt-1 font-display text-lg font-bold font-mono tabular-nums text-amber-600 dark:text-amber-400">
                {usbCheckedAt ? formatBytes(usbCandidateSummary.noCandidateBytes) : '—'}
              </div>
              <div className="text-[11px] font-mono text-slate-500">
                {usbCheckedAt
                  ? `${usbCandidateSummary.noCandidateFiles} ${t('файл. без найденного совпадения по метаданным', 'files with no metadata match found')}`
                  : checkingUsb ? t('Поиск…', 'Searching…') : t('Не проверено', 'Not checked')}
              </div>
            </div>

            <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 p-3 flex flex-col justify-between">
              <div className="text-xs text-slate-500">
                {t('Статус поиска (без записи в каталог)', 'Candidate lookup status (no catalog writes)')}
              </div>
              <div className="text-xs text-slate-800 dark:text-slate-200">
                {usbCheckedAt
                  ? t(`Диски с кандидатами: ${new Set(Object.values(usbMatches).flat().map((hit) => hit.volumeId)).size} из ${snapshot.volumes.length}`, `Disks with candidates: ${new Set(Object.values(usbMatches).flat().map((hit) => hit.volumeId)).size} of ${snapshot.volumes.length}`)
                  : checkingUsb
                    ? t(`Поиск: ${usbCheckProgress?.processed ?? 0}/${usbCheckProgress?.total ?? files.length}`, `Searching: ${usbCheckProgress?.processed ?? 0}/${usbCheckProgress?.total ?? files.length}`)
                    : t('Не проверено', 'Not checked')}
              </div>
              <button
                type="button"
                onClick={() => {
                  const candidatePaths = files
                    .filter((f) => (usbMatches[f.relativePath]?.length ?? 0) > 0)
                    .map((f) => f.relativePath);
                  setStagedPaths((prev) => {
                    const next = new Set(prev);
                    for (const p of candidatePaths) next.add(p);
                    return next;
                  });
                  onNotify(
                    t(`В список проверки добавлено кандидатов: ${candidatePaths.length}. Резерв не подтверждён.`, `Added ${candidatePaths.length} metadata candidates to the review list. Backup is unverified.`),
                    'success'
                  );
                }}
                className="text-left text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
              >
                {t('Добавить кандидатов в список проверки →', 'Add candidates to review list →')}
              </button>
            </div>
          </div>

          <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 overflow-hidden">
            <div className="px-3.5 py-2 border-b border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-900/50 flex flex-wrap items-center justify-between gap-2 text-xs">
              <div className="inline-flex p-0.5 rounded bg-slate-200/70 dark:bg-slate-800">
                {(
                  [
                    ['all', t('Все файлы', 'All Files')],
                    ['candidate', t('Есть кандидат', 'Has candidate')],
                    ['no_candidate', t('Нет кандидата', 'No candidate')],
                  ] as const
                ).map(([k, lbl]) => (
                  <button
                    key={k}
                    type="button"
                    onClick={() => setBackupFilter(k)}
                    disabled={k !== 'all' && (!usbCheckedAt || checkingUsb)}
                    className={`px-2.5 py-0.5 rounded cursor-pointer ${
                      backupFilter === k
                        ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white font-medium'
                        : 'text-slate-600 dark:text-slate-400'
                    }`}
                  >
                    {lbl}
                  </button>
                ))}
              </div>
              <span className="text-slate-500">{t('Ключ: имя NFC, без краёв и регистра + точный размер; содержимое не проверяется', 'Key: NFC name, trimmed and case-insensitive + exact byte size; content is not checked')}</span>
            </div>

            <div className="overflow-auto max-h-[430px]">
              <table className="w-full text-left border-collapse text-xs">
                <thead className="sticky top-0 bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-400">
                  <tr>
                    <th className="py-1.5 px-3 font-medium">{t('Локальный файл', 'Local File')}</th>
                    <th className="py-1.5 px-2.5 font-medium text-right whitespace-nowrap">{t('Размер', 'Size')}</th>
                    <th className="py-1.5 px-3 font-medium">{t('Совпавшие диски-кандидаты', 'Candidate disks')}</th>
                    <th className="py-1.5 px-3 font-medium text-right whitespace-nowrap">{t('В план', 'Stage')}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200/60 dark:divide-slate-800/60">
                  {files
                    .filter((f) => {
                      const has = (usbMatches[f.relativePath]?.length ?? 0) > 0;
                      if (backupFilter === 'candidate' && !has) return false;
                      if (backupFilter === 'no_candidate' && has) return false;
                      const q = localFilter.trim().toLowerCase();
                      if (q && !f.relativePath.toLowerCase().includes(q)) return false;
                      return true;
                    })
                    .sort((a, b) => b.size - a.size)
                    .slice(0, 150)
                    .map((f) => {
                      const hits = usbMatches[f.relativePath] ?? [];
                      const isStaged = stagedPaths.has(f.relativePath);
                      return (
                        <tr key={f.id} className="hover:bg-slate-50 dark:hover:bg-slate-900/50">
                          <td className="py-1.5 px-3 max-w-[360px]">
                            <div className="font-medium text-slate-900 dark:text-slate-100 truncate">{f.name}</div>
                            <div className="font-mono text-[11px] text-slate-500 truncate">{f.relativePath}</div>
                          </td>
                          <td className="py-1.5 px-2.5 text-right font-mono tabular-nums text-slate-900 dark:text-white whitespace-nowrap">
                            {formatBytes(f.size)}
                          </td>
                          <td className="py-1.5 px-3 font-mono text-[11px]">
                            {hits.length > 0 ? (
                              <span className="text-emerald-600 dark:text-emerald-400">
                                {hits.map((h) => `#${h.volumeId} (${h.volumeLabel})`).join(', ')}
                              </span>
                            ) : (
                              <span className="inline-flex items-center gap-1 text-amber-600 dark:text-amber-400">
                                <ShieldAlert className="w-3 h-3" />
                                {!usbCheckedAt ? (checkingUsb ? t('Поиск…', 'Searching…') : t('Не проверено', 'Not checked')) : t('Совпадение не найдено', 'No metadata match found')}
                              </span>
                            )}
                          </td>
                          <td className="py-1.5 px-3 text-right whitespace-nowrap">
                            <button
                              type="button"
                              onClick={() => toggleStage(f.relativePath)}
                              className={`px-2 py-0.5 rounded border text-[11px] cursor-pointer ${
                                isStaged
                                  ? 'border-amber-500 bg-amber-500 text-slate-950 font-semibold'
                                  : 'border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300'
                              }`}
                            >
                              {isStaged ? t('В плане', 'Staged') : t('Выбрать', 'Stage')}
                            </button>
                          </td>
                        </tr>
                      );
                    })}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      )}

      {/* Mode 6: Folder Size Growth Diff */}
      {activeMode === 'diff' && (
        <div className="border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 p-4 flex flex-col gap-3">
          <div className="flex flex-wrap items-center justify-between gap-2">
            <div>
              <h3 className="text-sm font-semibold text-slate-900 dark:text-white">
                {t('Сравнение роста папок («Что выросло?»)', 'Folder Size Growth Diff ("What Grew?")')}
              </h3>
              <p className="text-xs text-slate-500">
                {baseline
                  ? t(
                      `База замера от ${new Date(baseline.savedAtMs).toLocaleString(numLocale)}. Откройте эту же папку позже, чтобы увидеть изменения.`,
                      `Baseline captured ${new Date(baseline.savedAtMs).toLocaleString(numLocale)}. Re-open folder later to track growth.`
                    )
                  : t(
                      'Нажмите «Запомнить базу», чтобы сохранить размеры папок локально в браузере (без добавления в USB-архив).',
                      'Click "Save Baseline" to snapshot folder sizes locally in browser storage.'
                    )}
              </p>
            </div>
            <button
              type="button"
              onClick={handleSaveBaseline}
              className="px-3 py-1.5 rounded bg-blue-600 text-white text-xs font-medium hover:bg-blue-500 cursor-pointer"
            >
              {baseline ? t('Обновить базу сейчас', 'Update Baseline Now') : t('Запомнить базу', 'Save Baseline')}
            </button>
          </div>

          {baseline && (
            <div className="border border-slate-200 dark:border-slate-800 rounded overflow-auto max-h-[430px]">
              <table className="w-full text-left border-collapse text-xs">
                <thead className="sticky top-0 bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-400">
                  <tr>
                    <th className="py-1.5 px-3 font-medium">{t('Папка', 'Folder')}</th>
                    <th className="py-1.5 px-2.5 font-medium text-right whitespace-nowrap">{t('Было', 'Baseline')}</th>
                    <th className="py-1.5 px-2.5 font-medium text-right whitespace-nowrap">{t('Сейчас', 'Current')}</th>
                    <th className="py-1.5 px-3 font-medium text-right whitespace-nowrap">{t('Изменение Δ', 'Delta')}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-200/60 dark:divide-slate-800/60">
                  {diffRows.slice(0, 100).map((row) => (
                    <tr key={row.path} className="hover:bg-slate-50 dark:hover:bg-slate-900/50">
                      <td className="py-1.5 px-3 font-mono text-slate-800 dark:text-slate-200 truncate max-w-[360px]">
                        {row.path}
                      </td>
                      <td className="py-1.5 px-2.5 text-right font-mono tabular-nums text-slate-500">
                        {formatBytes(row.prevBytes)}
                      </td>
                      <td className="py-1.5 px-2.5 text-right font-mono tabular-nums text-slate-900 dark:text-white">
                        {formatBytes(row.currBytes)}
                      </td>
                      <td
                        className={`py-1.5 px-3 text-right font-mono font-semibold tabular-nums ${
                          row.deltaBytes > 0
                            ? 'text-amber-600 dark:text-amber-400'
                            : row.deltaBytes < 0
                              ? 'text-emerald-600 dark:text-emerald-400'
                              : 'text-slate-400'
                        }`}
                      >
                        {row.deltaBytes > 0
                          ? `+${formatBytes(row.deltaBytes)}`
                          : row.deltaBytes < 0
                            ? `-${formatBytes(Math.abs(row.deltaBytes))}`
                            : '0 B'}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {/* Review list collector: informational only, never a cleanup plan */}
      {effectiveStagedItems.length > 0 && (
        <div className="border border-amber-500/60 dark:border-amber-500/50 rounded-lg bg-amber-50/80 dark:bg-slate-900 px-3.5 py-2.5 flex flex-wrap items-center justify-between gap-3">
          <div className="flex flex-wrap items-center gap-3 text-xs">
            <span className="font-semibold text-slate-900 dark:text-white">
              {t('Список проверки:', 'Review list:')} {effectiveStagedItems.length}{' '}
              {t('объектов', 'items')}
            </span>
            <span className="font-mono font-bold tabular-nums text-amber-700 dark:text-amber-400">
              {t('Объём выбранных элементов:', 'Selected item size:')} {formatBytes(totalStagedBytes)}
            </span>
            <button
              type="button"
              onClick={() => setStagedPaths(new Set())}
              className="text-slate-500 hover:text-slate-900 dark:hover:text-white underline cursor-pointer"
            >
              {t('Сбросить', 'Clear')}
            </button>
          </div>

          <div className="flex flex-wrap items-center gap-1.5 text-xs">
            <span className="text-slate-500 hidden sm:inline">
              {t('Только список метаданных:', 'Metadata list only:')}
            </span>
            <button
              type="button"
              onClick={handleExportReviewCsv}
              className="inline-flex items-center gap-1 px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 cursor-pointer"
            >
              <Download className="w-3.5 h-3.5" />
              {t('Экспорт CSV (без удаления)', 'Export CSV (no deletion)')}
            </button>
          </div>
        </div>
      )}


    </div>
  );
};
