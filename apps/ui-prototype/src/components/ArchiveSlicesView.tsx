import React, { useCallback, useEffect, useState } from 'react';
import {
  Copy,
  Film,
  HardDrive,
  PieChart,
  Search,
  ShieldAlert,
} from 'lucide-react';
import type { ArchiveSlicesResponse, SystemSnapshot } from '../types/atlas';
import { formatBytes } from '../utils/format';
import { useI18n } from '../i18n';

interface ArchiveSlicesViewProps {
  snapshot: SystemSnapshot;
  initialVolumeId: number | null;
  onJumpToSearch: (params: { volumeId: number | null; folderPrefix: string | null; query?: string }) => void;
  onJumpToCopyCandidates: (entryId: number) => void;
  onOpenSunburst: (volumeId: number | null, folderPath: string | null, highlightQuery?: string) => void;
  onNotify: (message: string, tone?: 'info' | 'error' | 'success') => void;
}

const SLICE_COLORS = [
  '#2563EB', // blue
  '#0D9488', // teal
  '#D97706', // amber
  '#7C3AED', // violet
  '#E11D48', // rose
  '#0284C7', // sky
  '#059669', // emerald
  '#4F46E5', // indigo
];

type SliceDimension = 'overview' | 'duplicates' | 'volumes';

export const ArchiveSlicesView: React.FC<ArchiveSlicesViewProps> = ({
  snapshot,
  initialVolumeId,
  onJumpToSearch,
  onJumpToCopyCandidates,
  onOpenSunburst,
  onNotify,
}) => {
  const { lang, t } = useI18n();
  const [scopedVolumeId, setScopedVolumeId] = useState<number | null>(initialVolumeId);
  const [selectedCategory, setSelectedCategory] = useState<string | null>(null);
  const [activeDim, setActiveDim] = useState<SliceDimension>('overview');
  const [data, setData] = useState<ArchiveSlicesResponse | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    setScopedVolumeId(initialVolumeId);
  }, [initialVolumeId]);

  const loadSlices = useCallback(async () => {
    setLoading(true);
    try {
      const resp = await fetch('/api/catalog/slices', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          volumeId: scopedVolumeId,
          onlyConnected: false,
          category: selectedCategory,
        }),
      });
      const json = await resp.json();
      if (!resp.ok) {
        onNotify(json.error || t('Не удалось построить разрезы архива', 'Failed to compute archive slices'), 'error');
        return;
      }
      setData(json as ArchiveSlicesResponse);
    } catch (err) {
      onNotify((err as Error).message, 'error');
    } finally {
      setLoading(false);
    }
  }, [scopedVolumeId, selectedCategory, onNotify, t]);

  useEffect(() => {
    void loadSlices();
  }, [loadSlices, snapshot.totalCatalogEntries, snapshot.pinnedGenerationSha]);

  const dupProtectionPct =
    data && data.totalIndexedBytes > 0
      ? Math.round((data.multiCopyProtectedBytes / data.totalIndexedBytes) * 100)
      : 0;

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

  const translateBucket = (id: string, fallback: string) => {
    if (lang === 'ru') return fallback;
    const map: Record<string, string> = {
      gt50: '> 50 GB (UHD BD-Remux / BDMV)',
      '10to50': '10 – 50 GB (1080p Remux & 4K Rips)',
      '1to10': '1 – 10 GB (1080p / 720p Rips & Episodes)',
      '100mto1g': '100 MB – 1 GB (Short clips & extras)',
      lt100m: '< 100 MB (Subtitles .srt & metadata)',
    };
    return map[id] ?? fallback;
  };

  const translateResolution = (resolution: string) => {
    if (lang === 'ru') {
      const ruMap: Record<string, string> = {
        '2160p': '4K UHD (2160p)',
        '1080p': 'Full HD (1080p)',
        '720p': 'HD (720p)',
        SD: 'SD (<720p)',
      };
      return ruMap[resolution] ?? resolution;
    }
    const enMap: Record<string, string> = {
      '2160p': '4K UHD (2160p)',
      '1080p': 'Full HD (1080p)',
      '720p': 'HD (720p)',
      SD: 'SD (<720p)',
    };
    return enMap[resolution] ?? resolution;
  };

  const translateTier = (tier: 'single' | 'paired' | 'excessive') => {
    if (tier === 'single') {
      return {
        label: t('1 копия (без резерва)', '1 copy (no backup)'),
        desc: t(
          'Фильмы и видео без дублей на других дисках — приоритет для резервного копирования.',
          'Videos present on a single USB drive only — highest priority for backup.'
        ),
      };
    }
    if (tier === 'paired') {
      return {
        label: t('2 копии (оптимальный резерв)', '2 copies (protected)'),
        desc: t(
          'Основной экземпляр + 1 резервная копия на другом носителе.',
          'Primary file plus 1 redundant copy on another USB drive.'
        ),
      };
    }
    return {
      label: t('3+ копии (избыточные дубли)', '3+ copies (excess duplicates)'),
      desc: t(
        'Видеофайлы, продублированные трижды и более — кандидаты на очистку места.',
        'Video files duplicated 3 or more times — top candidates to reclaim space.'
      ),
    };
  };

  const numLocale = lang === 'en' ? 'en-US' : 'ru-RU';

  const fsBreakdown = Array.from(
    snapshot.volumes.reduce((map, v) => {
      const slot = map.get(v.filesystem) ?? { fs: v.filesystem, count: 0, usedBytes: 0, capacityBytes: 0 };
      slot.count += 1;
      slot.usedBytes += v.usedBytes;
      slot.capacityBytes += v.capacityBytes;
      map.set(v.filesystem, slot);
      return map;
    }, new Map<string, { fs: string; count: number; usedBytes: number; capacityBytes: number }>())
  )
    .map(([, v]) => v)
    .sort((a, b) => b.capacityBytes - a.capacityBytes);

  return (
    <div className="flex flex-col gap-3.5">
      {/* Top Scope & Dimension Switcher */}
      <div className="border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 px-3.5 py-2.5 flex flex-wrap items-center justify-between gap-2.5">
        <div className="flex flex-wrap items-center gap-2 text-xs">
          <button
            type="button"
            onClick={() => setScopedVolumeId(null)}
            className={`px-2.5 py-1 rounded font-medium border whitespace-nowrap shrink-0 cursor-pointer ${
              scopedVolumeId === null
                ? 'border-blue-600 bg-blue-600 text-white'
                : 'border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-300'
            }`}
          >
            {t('Весь архив', 'All Archive')} ({snapshot.volumes.length})
          </button>

          <select
            value={scopedVolumeId ?? ''}
            onChange={(e) => {
              const val = e.target.value;
              setScopedVolumeId(val ? Number(val) : null);
            }}
            aria-label={t('Выбор конкретного диска для разреза', 'Select disk scope')}
            className="px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-xs focus:outline-none focus:ring-2 focus:ring-blue-500 max-w-[240px]"
          >
            <option value="">{t('Конкретный USB-диск…', 'Specific USB disk…')}</option>
            {snapshot.volumes.map((v) => (
              <option key={v.id} value={v.id}>
                #{v.id} [{v.diskCode}] · {v.label}
              </option>
            ))}
          </select>

          <select
            value={selectedCategory ?? ''}
            onChange={(e) => setSelectedCategory(e.target.value || null)}
            aria-label={t('Фильтр раздела для разрезов', 'Filter by archive section')}
            className="px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 text-xs focus:outline-none focus:ring-2 focus:ring-blue-500 max-w-[210px]"
          >
            <option value="">{t('Все разделы архива', 'All archive sections')}</option>
            {snapshot.categories.map((c) => (
              <option key={c} value={c}>
                {translateCategory(c)}
              </option>
            ))}
          </select>
        </div>

        <div className="flex flex-wrap items-center gap-2 text-xs">
          <div className="inline-flex p-0.5 rounded bg-slate-100 dark:bg-slate-800">
            {(
              [
                ['overview', t('Видео-разрезы', 'Video Slices')],
                ['duplicates', t('Дубликаты и резерв', 'Duplicates & Backup')],
                ['volumes', t('Диски и ФС', 'USB Drives & FS')],
              ] as const
            ).map(([k, lbl]) => (
              <button
                key={k}
                type="button"
                onClick={() => setActiveDim(k)}
                className={`px-2.5 py-1 rounded font-medium whitespace-nowrap shrink-0 cursor-pointer ${
                  activeDim === k
                    ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white shadow-xs'
                    : 'text-slate-600 dark:text-slate-400'
                }`}
              >
                {lbl}
              </button>
            ))}
          </div>

          <button
            type="button"
            onClick={() => onOpenSunburst(scopedVolumeId, null)}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-800 font-medium whitespace-nowrap shrink-0 cursor-pointer"
          >
            <PieChart className="w-3.5 h-3.5 text-blue-500" />
            {t('На карте объёма', 'Volume Map')}
          </button>
        </div>
      </div>

      {/* 4 Key Structural Slice Metrics */}
      {data && (
        <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-3">
          <div className="border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5">
            <div className="text-xs text-slate-500 dark:text-slate-400">
              {t('Объём выборки', 'Scoped Volume')} ·{' '}
              {scopedVolumeId ? `USB #${scopedVolumeId}` : t('Все 60 USB', 'All 60 USBs')}
            </div>
            <div className="mt-1 font-display text-xl font-bold font-mono tabular-nums text-slate-900 dark:text-white">
              {formatBytes(data.totalIndexedBytes)}
            </div>
            <div className="mt-0.5 text-[11px] font-mono tabular-nums text-slate-500">
              {data.totalFiles.toLocaleString(numLocale)} {t('файл.', 'files')} · {data.volumeCount}{' '}
              {t('дисков', 'disks')}
            </div>
          </div>

          <div className="border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5">
            <div className="text-xs text-slate-500 dark:text-slate-400">
              {t('Уникальный логический объём', 'Unique Logical Size')}
            </div>
            <div className="mt-1 font-display text-xl font-bold font-mono tabular-nums text-slate-900 dark:text-white">
              {formatBytes(data.uniqueLogicalBytes)}
            </div>
            <div className="mt-0.5 text-[11px] font-mono tabular-nums text-slate-500">
              {t('Защищено копиями:', 'Protected by copies:')} {dupProtectionPct}%
            </div>
          </div>

          <div className="border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col justify-between">
            <div>
              <div className="text-xs text-slate-500 dark:text-slate-400">
                {t('Побайтовые дубликаты файлов', 'Exact Byte Duplicate Excess')}
              </div>
              <div className="mt-1 font-display text-xl font-bold font-mono tabular-nums text-blue-600 dark:text-blue-400">
                {formatBytes(data.duplicateExcessBytes)}
              </div>
            </div>
            <button
              type="button"
              onClick={() => onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: 'dup:>1' })}
              className="mt-0.5 text-left text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
            >
              {t('Открыть дубликаты в Поиске →', 'Open duplicates in Search →')}
            </button>
          </div>

          <div className="border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col justify-between">
            <div>
              <div className="text-xs text-slate-500 dark:text-slate-400">
                {t('В единственной копии (без резерва)', 'Single Copy (No Backup)')}
              </div>
              <div className="mt-1 font-display text-xl font-bold font-mono tabular-nums text-amber-600 dark:text-amber-400">
                {formatBytes(data.singleCopyBytes)}
              </div>
            </div>
            <button
              type="button"
              onClick={() => onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: 'dup:1' })}
              className="mt-0.5 text-left text-xs text-blue-600 dark:text-blue-400 hover:underline cursor-pointer"
            >
              {t('Показать видео без резерва →', 'Show unbacked videos →')}
            </button>
          </div>
        </div>
      )}

      {loading && !data ? (
        <div className="h-64 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 flex items-center justify-center text-xs text-slate-500">
          {t('Вычисление разрезов видеоархива…', 'Computing video archive slices…')}
        </div>
      ) : data && activeDim === 'overview' ? (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-3.5">
          {/* Slice 1: Video Resolution & Video Codec (6 cols — powered by Video Header Light plugin!) */}
          <section className="lg:col-span-6 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col gap-3">
            <div>
              <div className="flex items-center justify-between mb-2">
                <h2 className="text-sm font-semibold text-slate-900 dark:text-white flex items-center gap-1.5">
                  <Film className="w-4 h-4 text-blue-500" />
                  {t('Разрешение видео (из заголовков контейнеров)', 'Video Resolution (Container Headers)')}
                </h2>
                <span className="text-xs text-slate-500">{t('Клик — фильтр в Поиске', 'Click to filter')}</span>
              </div>

              <div className="flex flex-col gap-1.5 text-xs">
                {(data.byResolution ?? []).map((r) => (
                  <button
                    key={r.resolution}
                    type="button"
                    onClick={() =>
                      onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: r.searchQuery })
                    }
                    className="w-full text-left px-2.5 py-2 rounded border border-slate-200/80 dark:border-slate-800/80 hover:border-blue-500/60 hover:bg-slate-50 dark:hover:bg-slate-900/50 transition-colors cursor-pointer"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-semibold text-slate-900 dark:text-slate-100">
                        {translateResolution(r.resolution)}
                      </span>
                      <span className="font-mono tabular-nums font-semibold text-slate-900 dark:text-white">
                        {formatBytes(r.bytes)} ({r.sharePct.toFixed(1)}%)
                      </span>
                    </div>
                    <div className="mt-1 flex items-center gap-2">
                      <div className="flex-1 h-1.5 rounded-full bg-slate-200 dark:bg-slate-800 overflow-hidden">
                        <div
                          className="h-full rounded-full bg-blue-600 dark:bg-blue-500"
                          style={{ width: `${Math.max(r.bytes > 0 ? 2 : 0, r.sharePct)}%` }}
                        />
                      </div>
                      <span className="font-mono text-[11px] tabular-nums text-slate-500 w-24 text-right">
                        {r.fileCount} {t('видео', 'videos')}
                      </span>
                    </div>
                  </button>
                ))}
              </div>
            </div>

            {data.byVideoCodec && data.byVideoCodec.length > 0 && (
              <div className="pt-2 border-t border-slate-200 dark:border-slate-800">
                <div className="text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                  {t('Видеокодеки архива (HEVC / AVC / AV1)', 'Video Codecs (HEVC / AVC / AV1)')}
                </div>
                <div className="grid grid-cols-1 sm:grid-cols-3 gap-2 text-xs">
                  {data.byVideoCodec.map((c) => (
                    <button
                      key={c.codec}
                      type="button"
                      onClick={() =>
                        onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: c.searchQuery })
                      }
                      className="p-2 rounded border border-slate-200 dark:border-slate-800 hover:border-blue-500 text-left cursor-pointer"
                    >
                      <div className="font-mono font-bold text-slate-900 dark:text-white">{c.codec}</div>
                      <div className="font-mono text-[11px] text-slate-500 mt-0.5">
                        {formatBytes(c.bytes)} · {c.fileCount}×
                      </div>
                    </button>
                  ))}
                </div>
              </div>
            )}
          </section>

          {/* Slice 2: By Archive Section / Genre (6 cols) */}
          <section className="lg:col-span-6 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col gap-2.5">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-white">
                {t('Разрез по разделам видеоархива', 'By Video Archive Section')}
              </h2>
              <span className="text-xs text-slate-500">{t('Клик — фильтр', 'Click to filter')}</span>
            </div>

            <div className="h-2.5 w-full rounded-full overflow-hidden flex bg-slate-100 dark:bg-slate-900">
              {data.byCategory.map((row) => (
                <button
                  key={row.category}
                  type="button"
                  title={`${translateCategory(row.category)}: ${formatBytes(row.bytes)} (${row.sharePct.toFixed(1)}%)`}
                  onClick={() =>
                    onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: row.searchQuery })
                  }
                  className="h-full transition-opacity hover:opacity-85 cursor-pointer"
                  style={{
                    width: `${Math.max(1.5, row.sharePct)}%`,
                    backgroundColor: SLICE_COLORS[row.colorIndex % SLICE_COLORS.length],
                  }}
                />
              ))}
            </div>

            <div className="divide-y divide-slate-200/70 dark:divide-slate-800/70 text-xs">
              {data.byCategory.map((row) => {
                const color = SLICE_COLORS[row.colorIndex % SLICE_COLORS.length];
                return (
                  <div
                    key={row.category}
                    className="py-1.5 flex items-center justify-between gap-2 hover:bg-slate-50 dark:hover:bg-slate-900/50 px-1 rounded"
                  >
                    <button
                      type="button"
                      onClick={() =>
                        onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: row.searchQuery })
                      }
                      className="flex items-center gap-2 min-w-0 text-left flex-1 cursor-pointer"
                    >
                      <span className="w-2.5 h-2.5 rounded-xs shrink-0" style={{ backgroundColor: color }} />
                      <span className="font-medium text-slate-900 dark:text-slate-100 truncate">
                        {translateCategory(row.category)}
                      </span>
                      <span className="font-mono text-[11px] text-slate-500 shrink-0">
                        · {row.fileCount} {t('ф.', 'f')} · {row.volumeCount} {t('д.', 'd')}
                      </span>
                    </button>

                    <div className="flex items-center gap-2 shrink-0 font-mono tabular-nums">
                      <span className="text-slate-500 w-11 text-right">{row.sharePct.toFixed(1)}%</span>
                      <span className="font-semibold text-slate-900 dark:text-white w-20 text-right">
                        {formatBytes(row.bytes)}
                      </span>
                      <button
                        type="button"
                        title={t('Подсветить на карте объёма', 'Highlight on Volume Map')}
                        onClick={() => onOpenSunburst(scopedVolumeId, null, row.searchQuery)}
                        className="p-0.5 text-slate-400 hover:text-blue-500 cursor-pointer"
                      >
                        <PieChart className="w-3.5 h-3.5" />
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>

          {/* Slice 3: By Containers / Extensions (6 cols) */}
          <section className="lg:col-span-6 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col gap-2.5">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-white">
                {t('Разрез по контейнерам (.mkv, .m2ts, .mp4, .avi)', 'By Video Container')}
              </h2>
              <span className="text-xs text-slate-500">{t('По объёму', 'By size')}</span>
            </div>

            <div className="divide-y divide-slate-200/70 dark:divide-slate-800/70 text-xs">
              {data.byExtension.map((row) => (
                <div
                  key={row.ext}
                  onClick={() =>
                    row.searchQuery &&
                    onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: row.searchQuery })
                  }
                  className="py-1.5 flex items-center justify-between gap-2.5 hover:bg-slate-50 dark:hover:bg-slate-900/50 px-1 rounded cursor-pointer"
                >
                  <div className="flex items-center gap-2 min-w-0 flex-1">
                    <span className="font-mono font-semibold text-slate-900 dark:text-white w-14 shrink-0">
                      {row.ext === 'прочие' ? t('прочие', 'other') : row.ext}
                    </span>
                    <div className="flex-1 h-1.5 rounded-full bg-slate-200/80 dark:bg-slate-800 overflow-hidden">
                      <div
                        className="h-full rounded-full bg-teal-600 dark:bg-teal-400"
                        style={{ width: `${Math.max(2, row.sharePct)}%` }}
                      />
                    </div>
                  </div>
                  <div className="flex items-center gap-2.5 shrink-0 font-mono tabular-nums">
                    <span className="text-slate-500 text-[11px]">
                      {t('ср.', 'avg')} {formatBytes(row.avgBytes)}
                    </span>
                    <span className="text-slate-500 w-9 text-right">{row.fileCount}</span>
                    <span className="font-semibold text-slate-900 dark:text-white w-20 text-right">
                      {formatBytes(row.bytes)}
                    </span>
                  </div>
                </div>
              ))}
            </div>
          </section>

          {/* Slice 4: By File Size Buckets (6 cols) */}
          <section className="lg:col-span-6 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col gap-2.5">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-white">
                {t('Разрез по размерам файлов', 'By File Size Bucket')}
              </h2>
              <span className="text-xs text-slate-500">{t('От BD-Remux >50 GB до субтитров', 'From >50 GB masters to .srt')}</span>
            </div>

            <div className="flex flex-col gap-1.5 text-xs">
              {data.bySizeBucket.map((b) => (
                <button
                  key={b.id}
                  type="button"
                  onClick={() =>
                    onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: b.searchQuery })
                  }
                  className="w-full text-left px-2.5 py-2 rounded border border-slate-200/80 dark:border-slate-800/80 hover:border-blue-500/60 hover:bg-slate-50 dark:hover:bg-slate-900/50 transition-colors cursor-pointer"
                >
                  <div className="flex items-center justify-between gap-2">
                    <span className="font-medium text-slate-900 dark:text-slate-100">
                      {translateBucket(b.id, b.label)}
                    </span>
                    <span className="font-mono tabular-nums font-semibold text-slate-900 dark:text-white">
                      {formatBytes(b.bytes)} ({b.sharePct.toFixed(1)}%)
                    </span>
                  </div>
                  <div className="mt-1 flex items-center gap-2">
                    <div className="flex-1 h-1.5 rounded-full bg-slate-200 dark:bg-slate-800 overflow-hidden">
                      <div
                        className="h-full rounded-full bg-indigo-600 dark:bg-indigo-400"
                        style={{ width: `${Math.max(b.bytes > 0 ? 2 : 0, b.sharePct)}%` }}
                      />
                    </div>
                    <span className="font-mono text-[11px] tabular-nums text-slate-500 w-20 text-right">
                      {b.fileCount} {t('файл.', 'files')}
                    </span>
                  </div>
                </button>
              ))}
            </div>
          </section>
        </div>
      ) : data && activeDim === 'duplicates' ? (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-3.5">
          <section className="lg:col-span-5 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col gap-2.5">
            <h2 className="text-sm font-semibold text-slate-900 dark:text-white">
              {t('Профиль резервирования видеоархива', 'Archive Redundancy Profile')}
            </h2>
            <div className="flex flex-col gap-2">
              {data.byDuplicationTier.map((tier) => {
                const tr = translateTier(tier.tier);
                return (
                  <div
                    key={tier.tier}
                    className="p-3 rounded border border-slate-200 dark:border-slate-800 flex flex-col gap-1.5 text-xs"
                  >
                    <div className="flex items-center justify-between gap-2">
                      <span className="font-semibold text-slate-900 dark:text-white flex items-center gap-1.5">
                        {tier.tier === 'single' && <ShieldAlert className="w-3.5 h-3.5 text-amber-500" />}
                        {tr.label}
                      </span>
                      <span className="font-mono font-semibold tabular-nums text-slate-900 dark:text-white">
                        {formatBytes(tier.occupiedBytes)}
                      </span>
                    </div>
                    <p className="text-slate-600 dark:text-slate-400">{tr.desc}</p>
                    <div className="flex items-center justify-between pt-1 border-t border-slate-200/60 dark:border-slate-800/60 font-mono text-[11px] tabular-nums text-slate-500">
                      <span>
                        {tier.distinctClasses} {t('уник.', 'uniq')} · {tier.fileInstances} {t('экз.', 'inst')}
                        {tier.reclaimableBytes > 0
                          ? ` · ${t('избыток', 'excess')} ${formatBytes(tier.reclaimableBytes)}`
                          : ''}
                      </span>
                      <button
                        type="button"
                        onClick={() =>
                          onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: tier.searchQuery })
                        }
                        className="inline-flex items-center gap-1 text-blue-600 dark:text-blue-400 hover:underline font-sans font-medium cursor-pointer"
                      >
                        <Search className="w-3 h-3" />
                        {t('Показать', 'Show')}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>

          <section className="lg:col-span-7 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col gap-2.5">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-white">
                {t('Крупнейшие междисковые копии по байтам', 'Largest Cross-Disk Byte Copies')}
              </h2>
              <span className="text-xs text-slate-500">{t('Клик — все диски с копией', 'Click for candidate disks')}</span>
            </div>

            <div className="divide-y divide-slate-200/70 dark:divide-slate-800/70 text-xs">
              {data.topDuplicateGroups.map((g) => (
                <div
                  key={`${g.sampleEntryId}-${g.name}`}
                  className="py-1.5 flex flex-wrap items-center justify-between gap-2.5"
                >
                  <div className="min-w-0 flex-1">
                    <div className="font-medium text-slate-900 dark:text-slate-100 truncate" title={g.name}>
                      {g.name}
                    </div>
                    <div className="font-mono text-[11px] text-slate-500 truncate">
                      {g.copies} {t('копий', 'copies')} · USB #{g.diskIds.join(', #')} ·{' '}
                      {t('экз.', 'each')} {formatBytes(g.size)}
                    </div>
                  </div>
                  <div className="flex items-center gap-2.5 shrink-0">
                    <div className="text-right font-mono tabular-nums">
                      <div className="font-semibold text-blue-600 dark:text-blue-400">
                        +{formatBytes(g.excessBytes)}
                      </div>
                      <div className="text-[10px] text-slate-500">{t('избыток', 'excess')}</div>
                    </div>
                    <button
                      type="button"
                      onClick={() => onJumpToCopyCandidates(g.sampleEntryId)}
                      className="inline-flex items-center gap-1 px-2 py-1 rounded border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-800 dark:text-slate-200 font-medium cursor-pointer"
                    >
                      <Copy className="w-3 h-3" />
                      {t('Копии', 'Copies')}
                    </button>
                  </div>
                </div>
              ))}
            </div>
          </section>
        </div>
      ) : data ? (
        <div className="grid grid-cols-1 lg:grid-cols-12 gap-3.5">
          <section className="lg:col-span-4 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col gap-3">
            <div>
              <h2 className="text-sm font-semibold text-slate-900 dark:text-white">
                {t('Файловые системы 60 USB-дисков', 'USB Drive Filesystems')}
              </h2>
              <div className="mt-2 divide-y divide-slate-200/70 dark:divide-slate-800/70 text-xs">
                {fsBreakdown.map((f) => (
                  <div key={f.fs} className="py-1.5 flex items-center justify-between gap-2">
                    <span className="font-mono font-semibold text-slate-900 dark:text-white">{f.fs}</span>
                    <span className="font-mono tabular-nums text-slate-600 dark:text-slate-400">
                      {f.count} {t('д.', 'disks')} · {formatBytes(f.capacityBytes)}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            <div className="pt-2 border-t border-slate-200 dark:border-slate-800">
              <h3 className="text-xs font-semibold text-slate-700 dark:text-slate-300 mb-1.5">
                {t('Годы архивации', 'Archive Years')}
              </h3>
              <div className="divide-y divide-slate-200/70 dark:divide-slate-800/70 text-xs">
                {data.byYear.map((yr) => (
                  <button
                    key={yr.year}
                    type="button"
                    onClick={() =>
                      onJumpToSearch({ volumeId: scopedVolumeId, folderPrefix: null, query: yr.searchQuery })
                    }
                    className="w-full py-1.5 flex items-center justify-between gap-2 hover:text-blue-600 cursor-pointer"
                  >
                    <span className="font-mono font-semibold">{yr.year}</span>
                    <span className="font-mono tabular-nums text-slate-600 dark:text-slate-400">
                      {yr.fileCount}× · {formatBytes(yr.bytes)}
                    </span>
                  </button>
                ))}
              </div>
            </div>
          </section>

          <section className="lg:col-span-8 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 p-3.5 flex flex-col gap-2.5">
            <div className="flex items-center justify-between">
              <h2 className="text-sm font-semibold text-slate-900 dark:text-white">
                {t('Крупнейшие USB-носители по объёму индекса', 'Top USB Drives by Indexed Volume')}
              </h2>
              <span className="text-xs text-slate-500">{t('Переход в Поиск или Карту', 'Open in Search or Map')}</span>
            </div>

            <div className="divide-y divide-slate-200/70 dark:divide-slate-800/70 text-xs">
              {data.topVolumes.map((v) => {
                const usedPct =
                  v.capacityBytes > 0 ? Math.min(100, Math.round((v.usedBytes / v.capacityBytes) * 100)) : 0;
                return (
                  <div key={v.volumeId} className="py-1.5 flex items-center justify-between gap-2.5">
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center justify-between gap-2">
                        <div className="flex items-center gap-1.5 min-w-0">
                          <HardDrive className="w-3.5 h-3.5 text-slate-400 shrink-0" />
                          <span className="font-mono font-bold tabular-nums text-slate-900 dark:text-white shrink-0">
                            #{v.volumeId}
                          </span>
                          <span className="font-medium text-slate-900 dark:text-slate-100 truncate">
                            {v.label}
                          </span>
                        </div>
                        <span className="font-mono text-[11px] font-semibold tabular-nums text-slate-900 dark:text-white shrink-0">
                          {formatBytes(v.indexedBytes)} ({v.fileCount} {t('ф.', 'f')}) · {usedPct}%
                        </span>
                      </div>
                      <div className="mt-1 h-1 w-full rounded-full bg-slate-200 dark:bg-slate-800 overflow-hidden">
                        <div
                          className="h-full rounded-full bg-blue-600 dark:bg-blue-500"
                          style={{ width: `${Math.max(3, usedPct)}%` }}
                        />
                      </div>
                    </div>

                    <div className="flex items-center gap-1 shrink-0">
                      <button
                        type="button"
                        onClick={() => onJumpToSearch({ volumeId: v.volumeId, folderPrefix: null })}
                        className="px-2 py-1 rounded border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 text-slate-700 dark:text-slate-300 cursor-pointer"
                      >
                        {t('Поиск', 'Search')}
                      </button>
                      <button
                        type="button"
                        onClick={() => onOpenSunburst(v.volumeId, null)}
                        className="inline-flex items-center gap-1 px-2 py-1 rounded bg-blue-600 text-white hover:bg-blue-500 font-medium cursor-pointer"
                      >
                        <PieChart className="w-3 h-3" />
                        {t('Карта', 'Map')}
                      </button>
                    </div>
                  </div>
                );
              })}
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
};
