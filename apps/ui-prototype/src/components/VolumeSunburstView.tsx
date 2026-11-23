import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  ArrowUpLeft,
  ChevronRight,
  Copy,
  Folder,
  HardDrive,
  Search,
  Sparkles,
  X,
} from 'lucide-react';
import type { SunburstNode, SunburstResponse, SystemSnapshot } from '../types/atlas';
import { formatBytes, formatExactBytes } from '../utils/format';
import { useI18n } from '../i18n';
import { ScopedRequestGate, type ScopedRequestTicket } from '../utils/scopedRequest';

interface VolumeSunburstViewProps {
  isActive: boolean;
  snapshot: SystemSnapshot;
  selectedVolumeId: number | null;
  selectedFolderPath: string | null;
  initialHighlightQuery?: string;
  onNavigateScope: (volumeId: number | null, folderPath: string | null) => void;
  onJumpToSearch: (params: { volumeId: number | null; folderPrefix: string | null; query?: string }) => void;
  onJumpToCopyCandidates: (entryId: number) => void;
  onOpenSlices: (volumeId: number | null) => void;
  onNotify: (message: string, tone?: 'info' | 'error' | 'success') => void;
}

interface ComputedArc {
  id: string;
  node: SunburstNode;
  depth: number;
  startAngle: number;
  endAngle: number;
  pathD: string;
  fill: string;
  ancestorIds: string[];
}

// Curated DaisyDisk-inspired 8-hue palette across 4 concentric depth rings
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
  [88, 134],  // Depth 1
  [137, 179], // Depth 2
  [182, 219], // Depth 3
  [222, 254], // Depth 4
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

  // Handle near-full 360° circle cleanly with two half-arcs
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

export const VolumeSunburstView: React.FC<VolumeSunburstViewProps> = ({
  isActive,
  snapshot,
  selectedVolumeId,
  selectedFolderPath,
  initialHighlightQuery = '',
  onNavigateScope,
  onJumpToSearch,
  onJumpToCopyCandidates,
  onOpenSlices,
  onNotify,
}) => {
  const { lang, t } = useI18n();
  const [groupBy, setGroupBy] = useState<'volumes' | 'categories' | 'extensions'>('volumes');
  const [highlightInput, setHighlightInput] = useState(initialHighlightQuery);
  const [appliedHighlight, setAppliedHighlight] = useState(initialHighlightQuery);
  const [sunburstData, setSunburstData] = useState<SunburstResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [hoveredNode, setHoveredNode] = useState<SunburstNode | null>(null);
  const [hoveredAncestors, setHoveredAncestors] = useState<Set<string>>(new Set());
  const requestGateRef = useRef<ScopedRequestGate | null>(null);
  if (!requestGateRef.current) requestGateRef.current = new ScopedRequestGate();

  useEffect(() => {
    setHighlightInput(initialHighlightQuery);
    setAppliedHighlight(initialHighlightQuery);
  }, [initialHighlightQuery]);

  const loadSunburst = useCallback(async (ticket: ScopedRequestTicket) => {
    const gate = requestGateRef.current!;
    setLoading(true);
    try {
      const resp = await fetch('/api/catalog/sunburst', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          volumeId: selectedVolumeId,
          folderPath: selectedFolderPath,
          groupBy,
          highlightQuery: appliedHighlight,
        }),
      });
      const data = await resp.json();
      if (!gate.isCurrent(ticket)) return;
      if (!resp.ok) {
        gate.fail(ticket);
        onNotify(data.error || t('Не удалось построить карту объёма', 'Failed to build volume map'), 'error');
        return;
      }
      if (!gate.succeed(ticket)) return;
      setSunburstData(data as SunburstResponse);
      setHoveredNode(null);
      setHoveredAncestors(new Set());
    } catch (err) {
      if (gate.fail(ticket)) onNotify((err as Error).message, 'error');
    } finally {
      if (gate.isCurrent(ticket)) setLoading(false);
    }
  }, [selectedVolumeId, selectedFolderPath, groupBy, appliedHighlight, onNotify]);

  useEffect(() => {
    if (!isActive) {
      setLoading(false);
      return;
    }
    const loadKey = JSON.stringify([selectedVolumeId, selectedFolderPath, groupBy, appliedHighlight, snapshot.totalCatalogEntries, snapshot.pinnedGenerationSha]);
    const gate = requestGateRef.current!;
    const ticket = gate.begin(loadKey);
    if (!ticket) {
      setLoading(false);
      return;
    }
    void loadSunburst(ticket);
    return () => gate.invalidate(ticket);
  }, [isActive, loadSunburst, selectedVolumeId, selectedFolderPath, groupBy, appliedHighlight, snapshot.totalCatalogEntries, snapshot.pinnedGenerationSha]);

  // Compute all visible Sunburst Arcs up to depth 4 (consolidating micro-slices < 0.9° cleanly)
  const { arcs, topChildren } = useMemo(() => {
    const root = sunburstData?.root;
    if (!root || root.size <= 0 || !root.children) {
      return { arcs: [] as ComputedArc[], topChildren: [] as SunburstNode[] };
    }

    const cx = 270;
    const cy = 270;
    const computed: ComputedArc[] = [];

    const buildLevel = (
      children: SunburstNode[],
      parentSize: number,
      startAng: number,
      endAng: number,
      depth: number,
      ancestors: string[]
    ) => {
      if (depth > 4 || children.length === 0 || parentSize <= 0) return;
      const totalSpan = endAng - startAng;
      if (totalSpan <= 0.012) return;

      const [rIn, rOut] = RING_RADII[depth - 1];
      let cursor = startAng;

      const visible: SunburstNode[] = [];
      let remainderSize = 0;
      let remainderMatched = 0;
      let remainderItems = 0;

      for (const child of children) {
        if (child.size <= 0) continue;
        const childSpan = (child.size / parentSize) * totalSpan;
        if (childSpan < 0.022 && visible.length >= 10) {
          remainderSize += child.size;
          remainderMatched += child.matchedBytes;
          remainderItems += child.itemCount;
        } else {
          visible.push(child);
        }
      }

      const effectiveList = [...visible];
      if (remainderSize > 0) {
        effectiveList.push({
          id: `other-${depth}-${ancestors.join('-')}`,
          name: t(`Прочие мелкие объекты (${remainderItems})`, `Other small items (${remainderItems})`),
          size: remainderSize,
          matchedBytes: remainderMatched,
          itemCount: remainderItems,
          kind: 'other_group',
          volumeId: root.volumeId,
          folderPath: root.folderPath,
          entryId: null,
          colorIndex: 0,
        });
      }

      for (let idx = 0; idx < effectiveList.length; idx++) {
        const node = effectiveList[idx];
        const frac = node.size / parentSize;
        const nextCursor = idx === effectiveList.length - 1 ? endAng : cursor + frac * totalSpan;
        const pathD = describeAnnularSector(cx, cy, rIn, rOut, cursor, nextCursor);
        if (pathD) {
          const palette = DAISY_PALETTES[node.colorIndex % DAISY_PALETTES.length];
          const fill =
            node.kind === 'other_group'
              ? '#64748B'
              : palette[Math.min(palette.length - 1, depth - 1)];

          const chain = [...ancestors, node.id];
          computed.push({
            id: node.id,
            node,
            depth,
            startAngle: cursor,
            endAngle: nextCursor,
            pathD,
            fill,
            ancestorIds: chain,
          });

          if (node.children && node.children.length > 0 && depth < 4) {
            buildLevel(node.children, node.size, cursor, nextCursor, depth + 1, chain);
          }
        }
        cursor = nextCursor;
      }
    };

    // Start at -90 degrees (12 o'clock)
    buildLevel(root.children, root.size, -Math.PI / 2, Math.PI * 1.5, 1, []);
    return { arcs: computed, topChildren: root.children };
  }, [sunburstData, t]);

  const handleNodeClick = (node: SunburstNode) => {
    if (node.kind === 'volume' && node.volumeId !== null) {
      onNavigateScope(node.volumeId, null);
    } else if (node.kind === 'directory' && node.volumeId !== null) {
      onNavigateScope(node.volumeId, node.folderPath);
    } else if (node.kind === 'category') {
      onJumpToSearch({ volumeId: null, folderPrefix: null, query: node.searchQuery || node.name });
    } else if (node.kind === 'file') {
      if ((node.duplicateCount ?? 1) > 1 && node.entryId) {
        onJumpToCopyCandidates(node.entryId);
      } else {
        onJumpToSearch({
          volumeId: node.volumeId,
          folderPrefix: node.folderPath,
          query: `name:"${node.name}"`,
        });
      }
    }
  };

  const handleZoomUp = () => {
    if (!sunburstData || sunburstData.breadcrumbs.length <= 1) return;
    const parentCrumb = sunburstData.breadcrumbs[sunburstData.breadcrumbs.length - 2];
    onNavigateScope(parentCrumb.volumeId, parentCrumb.folderPath);
  };

  const activeDisplayNode = hoveredNode ?? sunburstData?.root ?? null;
  const rootSize = sunburstData?.root.size ?? 0;
  const activeSharePct =
    activeDisplayNode && rootSize > 0
      ? Math.min(100, (activeDisplayNode.size / rootSize) * 100)
      : 100;

  return (
    <div className="flex flex-col lg:flex-row gap-3 items-stretch">
      {/* Narrow Compact Left Rail: 60-Disk Friendly, Single-Line + Volume Right, No Shelf Row */}
      <aside className="lg:w-[224px] xl:w-[236px] shrink-0 border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 flex flex-col max-h-[650px] overflow-hidden">
        <div className="px-2.5 py-2 border-b border-slate-200 dark:border-slate-800 flex flex-col gap-1.5 bg-slate-50/70 dark:bg-slate-900/50">
          <div className="flex items-center justify-between text-[11px]">
            <span className="font-semibold text-slate-900 dark:text-white">{t('Носители', 'Volumes')}</span>
            <span className="font-mono tabular-nums text-slate-500">{snapshot.volumes.length}</span>
          </div>
          <button
            type="button"
            onClick={() => onNavigateScope(null, null)}
            className={`w-full px-2 py-1.5 rounded text-left text-xs font-medium flex items-center justify-between transition-colors cursor-pointer ${
              selectedVolumeId === null
                ? 'bg-blue-600 text-white'
                : 'border border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-900 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800'
            }`}
          >
            <span>{t('Весь архив', 'All disks')}</span>
            <span className="font-mono tabular-nums">{formatBytes(snapshot.totalIndexedBytes)}</span>
          </button>
        </div>

        <div className="flex-1 overflow-y-auto divide-y divide-slate-200/50 dark:divide-slate-800/50">
          {snapshot.volumes.map((vol) => {
            const active = selectedVolumeId === vol.id;
            const usedPct = vol.capacityBytes > 0 ? Math.min(100, Math.round((vol.usedBytes / vol.capacityBytes) * 100)) : 0;

            return (
              <button
                key={vol.id}
                type="button"
                onClick={() => onNavigateScope(vol.id, null)}
                className={`w-full px-2 py-1.5 text-left text-[11px] transition-colors cursor-pointer ${
                  active
                    ? 'bg-blue-50 dark:bg-blue-950/45 text-slate-950 dark:text-white'
                    : 'hover:bg-slate-50 dark:hover:bg-slate-900/60 text-slate-700 dark:text-slate-300'
                }`}
              >
                <div className="flex items-center justify-between gap-1.5">
                  <span className="flex items-center gap-1 min-w-0 font-medium">
                    <span className="font-mono font-semibold tabular-nums shrink-0 text-slate-500 dark:text-slate-400">
                      #{vol.id}
                    </span>
                    <span className="truncate" title={vol.label}>{vol.label}</span>
                  </span>
                  <span className="font-mono font-semibold tabular-nums text-slate-900 dark:text-slate-100 shrink-0">
                    {formatBytes(vol.indexedBytes)}
                  </span>
                </div>
                <div className="mt-1 h-[2px] w-full rounded-full bg-slate-200/80 dark:bg-slate-800 overflow-hidden">
                  <div
                    className="h-full rounded-full bg-blue-600/75 dark:bg-blue-500/75"
                    style={{ width: `${Math.max(4, usedPct)}%` }}
                  />
                </div>
              </button>
            );
          })}
        </div>
      </aside>

      {/* Main DaisyDisk Canvas + Ranked Volume Breakdown */}
      <section className="flex-1 min-w-0 border border-slate-200 dark:border-slate-800 rounded-lg bg-white dark:bg-slate-950 flex flex-col overflow-hidden">
        {/* Top Breadcrumb & Highlight Filter Bar */}
        <div className="px-3.5 py-2 border-b border-slate-200 dark:border-slate-800 bg-slate-50/70 dark:bg-slate-900/50 flex flex-wrap items-center justify-between gap-2">
          {/* Interactive Breadcrumbs */}
          <div className="flex flex-wrap items-center gap-1 text-xs">
            {(sunburstData?.breadcrumbs ?? []).map((crumb, idx) => {
              const isLast = idx === (sunburstData?.breadcrumbs.length ?? 1) - 1;
              return (
                <React.Fragment key={`${crumb.label}-${idx}`}>
                  {idx > 0 && <ChevronRight className="w-3.5 h-3.5 text-slate-400 shrink-0" />}
                  <button
                    type="button"
                    onClick={() => onNavigateScope(crumb.volumeId, crumb.folderPath)}
                    className={`px-2 py-0.5 rounded transition-colors cursor-pointer whitespace-nowrap ${
                      isLast
                        ? 'font-semibold text-slate-900 dark:text-white bg-slate-200/70 dark:bg-slate-800'
                        : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white hover:bg-slate-200/40 dark:hover:bg-slate-800/50'
                    }`}
                  >
                    {idx === 0 && crumb.volumeId === null
                      ? t(`Весь архив (${snapshot.volumes.length})`, `All archive (${snapshot.volumes.length})`)
                      : crumb.label}
                  </button>
                </React.Fragment>
              );
            })}
          </div>

          {/* Archive Grouping + Search Highlight inside Sunburst */}
          <div className="flex flex-wrap items-center gap-1.5">
            {selectedVolumeId === null && (
              <div className="inline-flex p-0.5 rounded bg-slate-200/80 dark:bg-slate-800 text-xs">
                <button
                  type="button"
                  onClick={() => setGroupBy('volumes')}
                  className={`px-2 py-1 rounded transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                    groupBy === 'volumes'
                      ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white font-medium shadow-xs'
                      : 'text-slate-600 dark:text-slate-400'
                  }`}
                >
                  {t('По дискам', 'Disks')}
                </button>
                <button
                  type="button"
                  onClick={() => setGroupBy('categories')}
                  className={`px-2 py-1 rounded transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                    groupBy === 'categories'
                      ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white font-medium shadow-xs'
                      : 'text-slate-600 dark:text-slate-400'
                  }`}
                >
                  {t('По типам', 'Types')}
                </button>
                <button
                  type="button"
                  onClick={() => setGroupBy('extensions')}
                  className={`px-2 py-1 rounded transition-colors whitespace-nowrap shrink-0 cursor-pointer ${
                    groupBy === 'extensions'
                      ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white font-medium shadow-xs'
                      : 'text-slate-600 dark:text-slate-400'
                  }`}
                >
                  {t('По форматам', 'Extensions')}
                </button>
              </div>
            )}

            <form
              onSubmit={(e) => {
                e.preventDefault();
                setAppliedHighlight(highlightInput.trim());
              }}
              className="relative flex items-center"
            >
              <Sparkles className="w-3.5 h-3.5 text-blue-500 absolute left-2 pointer-events-none" />
              <input
                type="text"
                value={highlightInput}
                onChange={(e) => {
                  setHighlightInput(e.target.value);
                  if (!e.target.value.trim()) setAppliedHighlight('');
                }}
                placeholder={t('Подсветить (ext:mkv)…', 'Highlight (ext:mkv)…')}
                className="w-44 pl-7 pr-6 py-1 text-xs rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
              {highlightInput && (
                <button
                  type="button"
                  onClick={() => {
                    setHighlightInput('');
                    setAppliedHighlight('');
                  }}
                  className="absolute right-1.5 text-slate-400 hover:text-slate-700 dark:hover:text-slate-200 cursor-pointer"
                >
                  <X className="w-3.5 h-3.5" />
                </button>
              )}
            </form>

            <button
              type="button"
              onClick={() => onOpenSlices(selectedVolumeId)}
              className="px-2.5 py-1 text-xs font-medium rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 cursor-pointer"
            >
              {t('Разрезы', 'Slices')}
            </button>

            <button
              type="button"
              onClick={() =>
                onJumpToSearch({
                  volumeId: selectedVolumeId,
                  folderPrefix: selectedFolderPath,
                  query: appliedHighlight,
                })
              }
              className="inline-flex items-center gap-1 px-2.5 py-1 text-xs font-medium rounded bg-blue-600 text-white hover:bg-blue-500 transition-colors whitespace-nowrap shrink-0 cursor-pointer"
            >
              <Search className="w-3.5 h-3.5" />
              {t('Искать здесь', 'Search here')}
            </button>
          </div>
        </div>

        {/* Split DaisyDisk Viewport: Left Radial Sunburst SVG + Right Ranked Size Breakdown */}
        <div className="flex-1 grid grid-cols-1 xl:grid-cols-12 divide-y xl:divide-y-0 xl:divide-x divide-slate-200 dark:divide-slate-800">
          {/* Interactive Multi-Ring Sunburst Chart */}
          <div className="xl:col-span-7 p-4 flex flex-col items-center justify-center relative min-h-[470px]">
            {loading && !sunburstData ? (
              <div className="text-xs text-slate-500">{t('Построение карты объёмов…', 'Building volume map…')}</div>
            ) : (
              <div className="relative w-full max-w-[490px] aspect-square flex items-center justify-center">
                <svg
                  viewBox="0 0 540 540"
                  className="w-full h-full select-none overflow-visible"
                  onMouseLeave={() => {
                    setHoveredNode(null);
                    setHoveredAncestors(new Set());
                  }}
                >
                  {/* Subtle background guide rings */}
                  {RING_RADII.map(([rIn, rOut], idx) => (
                    <circle
                      key={idx}
                      cx={270}
                      cy={270}
                      r={(rIn + rOut) / 2}
                      fill="none"
                      stroke="currentColor"
                      strokeWidth={rOut - rIn}
                      className="text-slate-100/70 dark:text-slate-900/50"
                    />
                  ))}

                  {/* Sunburst Sectors */}
                  {arcs.map((arc) => {
                    const isHoveredChain = hoveredAncestors.has(arc.id);
                    const hasHighlightFilter = appliedHighlight.trim().length > 0;
                    const matchesFilter = arc.node.matchedBytes > 0;

                    let opacity = 0.92;
                    if (hasHighlightFilter) {
                      opacity = matchesFilter ? 1 : 0.16;
                    } else if (hoveredNode) {
                      opacity = isHoveredChain ? 1 : 0.35;
                    }

                    return (
                      <path
                        key={arc.id}
                        d={arc.pathD}
                        fill={arc.fill}
                        fillOpacity={opacity}
                        strokeWidth={1.4}
                        className="stroke-white dark:stroke-slate-950 transition-opacity duration-150 cursor-pointer"
                        onMouseEnter={() => {
                          setHoveredNode(arc.node);
                          setHoveredAncestors(new Set(arc.ancestorIds));
                        }}
                        onClick={() => handleNodeClick(arc.node)}
                      />
                    );
                  })}

                  {/* Interactive Center Hub */}
                  <circle
                    cx={270}
                    cy={270}
                    r={82}
                    onClick={handleZoomUp}
                    className={`fill-slate-50 dark:fill-slate-900 stroke-slate-200 dark:stroke-slate-800 stroke-2 transition-colors ${
                      (sunburstData?.breadcrumbs.length ?? 0) > 1
                        ? 'cursor-pointer hover:fill-slate-100 dark:hover:fill-slate-800'
                        : ''
                    }`}
                  />
                </svg>

                {/* HTML Overlay inside Center Hub for Crisp Typography */}
                <div
                  onClick={handleZoomUp}
                  className={`absolute w-[148px] h-[148px] rounded-full flex flex-col items-center justify-center text-center px-2 pointer-events-none ${
                    (sunburstData?.breadcrumbs.length ?? 0) > 1 ? 'cursor-pointer pointer-events-auto' : ''
                  }`}
                >
                  {(sunburstData?.breadcrumbs.length ?? 0) > 1 && !hoveredNode && (
                    <span className="inline-flex items-center gap-0.5 text-[10px] font-medium text-blue-600 dark:text-blue-400 mb-0.5">
                      <ArrowUpLeft className="w-3 h-3" />
                      {t('Наверх', 'Up')}
                    </span>
                  )}
                  <div className="font-display text-lg font-bold tracking-tight font-mono tabular-nums text-slate-900 dark:text-white leading-tight">
                    {formatBytes(activeDisplayNode?.size ?? 0)}
                  </div>
                  <div className="mt-1 text-[11px] font-medium text-slate-700 dark:text-slate-300 line-clamp-2 leading-snug max-w-[132px]">
                    {activeDisplayNode?.name ?? t('Нет данных', 'No data')}
                  </div>
                  <div className="mt-0.5 font-mono text-[10px] tabular-nums text-slate-500 dark:text-slate-400">
                    {hoveredNode ? `${activeSharePct.toFixed(1)}% · ` : ''}
                    {activeDisplayNode?.itemCount ?? 0} {t('файл.', 'files')}
                  </div>
                </div>
              </div>
            )}

            {/* Bottom Capacity Legend */}
            {sunburstData && (
              <div className="mt-2 w-full flex flex-wrap items-center justify-center gap-x-4 gap-y-1 text-xs text-slate-500 dark:text-slate-400 font-mono tabular-nums">
                <span>{t('Индекс:', 'Indexed:')} {formatBytes(sunburstData.root.size)}</span>
                <span aria-hidden="true">·</span>
                <span>{t('Ёмкость:', 'Capacity:')} {formatBytes(sunburstData.totalCapacityBytes)}</span>
                {appliedHighlight && (
                  <>
                    <span aria-hidden="true">·</span>
                    <span className="text-blue-600 dark:text-blue-400 font-semibold">
                      {t('Совпало:', 'Matched:')} {formatBytes(sunburstData.root.matchedBytes)}
                    </span>
                  </>
                )}
              </div>
            )}
          </div>

          {/* Right Column: Compact DaisyDisk Ranked Breakdown List */}
          <div className="xl:col-span-5 flex flex-col max-h-[580px]">
            <div className="px-3 py-2 border-b border-slate-200 dark:border-slate-800 flex items-center justify-between text-xs text-slate-500 dark:text-slate-400">
              <span className="font-medium text-slate-800 dark:text-slate-200">
                {t('По убыванию объёма', 'By size descending')} ({topChildren.length})
              </span>
              <span>{t('Клик — глубже', 'Click to drill')}</span>
            </div>

            <div className="flex-1 overflow-y-auto divide-y divide-slate-200/60 dark:divide-slate-800/60">
              {topChildren.map((item) => {
                const pct = rootSize > 0 ? Math.min(100, (item.size / rootSize) * 100) : 0;
                const swatch = DAISY_PALETTES[item.colorIndex % DAISY_PALETTES.length][0];
                const isHovered = hoveredNode?.id === item.id;

                return (
                  <div
                    key={item.id}
                    onMouseEnter={() => {
                      setHoveredNode(item);
                      setHoveredAncestors(new Set([item.id]));
                    }}
                    onMouseLeave={() => {
                      setHoveredNode(null);
                      setHoveredAncestors(new Set());
                    }}
                    onClick={() => handleNodeClick(item)}
                    className={`px-3 py-1.5 text-xs transition-colors cursor-pointer ${
                      isHovered
                        ? 'bg-slate-100 dark:bg-slate-900'
                        : 'hover:bg-slate-50 dark:hover:bg-slate-900/50'
                    }`}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <div className="flex items-center gap-1.5 min-w-0">
                        <span
                          className="w-2.5 h-2.5 rounded-xs shrink-0"
                          style={{ backgroundColor: swatch }}
                        />
                        {item.kind === 'directory' && <Folder className="w-3.5 h-3.5 text-slate-400 shrink-0" />}
                        {item.kind === 'volume' && <HardDrive className="w-3.5 h-3.5 text-slate-400 shrink-0" />}
                        <span className="font-medium text-slate-900 dark:text-slate-100 truncate" title={item.name}>
                          {item.name}
                        </span>
                      </div>
                      <div className="flex items-center gap-2 shrink-0 font-mono tabular-nums">
                        <span className="text-[11px] text-slate-500">{pct.toFixed(1)}%</span>
                        <span
                          className="font-semibold text-slate-900 dark:text-white"
                          title={formatExactBytes(item.size, lang)}
                        >
                          {formatBytes(item.size)}
                        </span>
                      </div>
                    </div>

                    <div className="mt-1 flex items-center justify-between gap-2">
                      <div className="flex-1 h-1 rounded-full bg-slate-200/80 dark:bg-slate-800 overflow-hidden">
                        <div
                          className="h-full rounded-full transition-all duration-150"
                          style={{ width: `${Math.max(2, pct)}%`, backgroundColor: swatch }}
                        />
                      </div>
                      <div className="flex items-center gap-2 text-[11px] shrink-0" onClick={(e) => e.stopPropagation()}>
                        {item.kind === 'file' && (item.duplicateCount ?? 1) > 1 && item.entryId && (
                          <button
                            type="button"
                            onClick={() => onJumpToCopyCandidates(item.entryId!)}
                            className="inline-flex items-center gap-0.5 text-blue-600 dark:text-blue-400 hover:underline font-medium cursor-pointer"
                          >
                            <Copy className="w-3 h-3" />
                            {item.duplicateCount}×
                          </button>
                        )}
                        {(item.kind === 'volume' || item.kind === 'directory') && (
                          <button
                            type="button"
                            onClick={() =>
                              onJumpToSearch({
                                volumeId: item.volumeId,
                                folderPrefix: item.folderPath,
                              })
                            }
                            className="text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
                          >
                            {t('Поиск →', 'Search →')}
                          </button>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </div>
      </section>
    </div>
  );
};
