import React, { useMemo, useRef, useState } from 'react';
import {
  Check,
  HardDrive,
  HardDriveDownload,
  Pencil,
  PieChart,
  RefreshCw,
  Search,
  Settings,
  X,
} from 'lucide-react';
import type { CatalogVolume, SystemSnapshot } from '../types/atlas';
import { formatBytes, formatUnixDate } from '../utils/format';
import { useI18n } from '../i18n';

interface DisksInventoryViewProps {
  snapshot: SystemSnapshot;
  onRefreshSnapshot: () => Promise<void>;
  onOpenSunburst: (volumeId: number) => void;
  onOpenSlices: (volumeId: number) => void;
  onOpenSearch: (volumeId: number) => void;
  onOpenDiskProperties: (volumeId: number) => void;
  onOpenScanModal: (volumeId: number | null) => void;
  onNotify: (message: string, tone?: 'info' | 'error' | 'success') => void;
}

export const DisksInventoryView: React.FC<DisksInventoryViewProps> = ({
  snapshot,
  onRefreshSnapshot,
  onOpenSunburst,
  onOpenSlices,
  onOpenSearch,
  onOpenDiskProperties,
  onOpenScanModal,
  onNotify,
}) => {
  const { t } = useI18n();
  const [filter, setFilter] = useState('');
  const [sortBy, setSortBy] = useState<'id' | 'used' | 'label'>('id');

  // Inline fast rename in the 60-disk table
  const [renamingId, setRenamingId] = useState<number | null>(null);
  const [renameDraft, setRenameDraft] = useState('');
  const renameInputRef = useRef<HTMLInputElement | null>(null);

  const startRename = (vol: CatalogVolume) => {
    setRenamingId(vol.id);
    setRenameDraft(vol.label);
    window.setTimeout(() => {
      renameInputRef.current?.focus();
      renameInputRef.current?.select();
    }, 20);
  };

  const commitRename = async (volId: number) => {
    const clean = renameDraft.trim();
    if (!clean) {
      setRenamingId(null);
      return;
    }
    const resp = await fetch('/api/volumes/update', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ volumeId: volId, label: clean }),
    });
    const data = await resp.json();
    if (!resp.ok || !data.ok) {
      onNotify(data.error || t('Ошибка переименования диска', 'Failed to rename disk'), 'error');
      return;
    }
    setRenamingId(null);
    await onRefreshSnapshot();
    onNotify(t(`Диск #${volId} переименован в «${clean}».`, `Disk #${volId} renamed to "${clean}".`), 'success');
  };

  const filteredVolumes = useMemo(() => {
    const q = filter.trim().toLowerCase();
    const list = snapshot.volumes.filter((v) => {
      if (!q) return true;
      return (
        `#${v.id}`.includes(q) ||
        String(v.id) === q ||
        v.diskCode.toLowerCase().includes(q) ||
        v.label.toLowerCase().includes(q) ||
        v.hardwareModel.toLowerCase().includes(q) ||
        v.note.toLowerCase().includes(q)
      );
    });
    list.sort((a, b) => {
      if (sortBy === 'used') return b.usedBytes - a.usedBytes || a.id - b.id;
      if (sortBy === 'label') return a.label.localeCompare(b.label) || a.id - b.id;
      return a.id - b.id;
    });
    return list;
  }, [snapshot.volumes, filter, sortBy]);

  return (
    <div className="flex flex-col gap-3">
      {/* Compact Header with Filter, Sort & Scan New Disk Button */}
      <div className="border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 px-3.5 py-2.5 flex flex-wrap items-center justify-between gap-3">
        <div className="flex flex-wrap items-center gap-3 text-xs font-mono tabular-nums">
          <span>
            <span className="text-slate-500">{t('USB-дисков:', 'USB Drives:')} </span>
            <span className="font-semibold text-slate-900 dark:text-white">
              {filteredVolumes.length}/{snapshot.volumes.length}
            </span>
          </span>
          <span aria-hidden="true">·</span>
          <span>
            <span className="text-slate-500">{t('Занято / Ёмкость:', 'Used / Cap:')} </span>
            <span className="font-semibold text-slate-900 dark:text-white">
              {formatBytes(snapshot.totalUsedBytes)} / {formatBytes(snapshot.totalCapacityBytes)}
            </span>
          </span>
          <span aria-hidden="true">·</span>
          <span className="font-sans text-slate-500">
            {t('Двойной клик по имени — быстрое переименование', 'Double-click label for fast rename')}
          </span>
        </div>

        <div className="flex flex-wrap items-center gap-2 text-xs">
          <input
            type="text"
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder={t('Фильтр #ID, USB-04, имя, заметка…', 'Filter #ID, USB-04, label, note…')}
            className="w-56 px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
          />
          <div className="inline-flex p-0.5 rounded bg-slate-100 dark:bg-slate-800">
            {(
              [
                ['id', '#ID'],
                ['used', t('Объём ↓', 'Size ↓')],
                ['label', t('Имя', 'Label')],
              ] as const
            ).map(([k, lbl]) => (
              <button
                key={k}
                type="button"
                onClick={() => setSortBy(k)}
                className={`px-2 py-1 rounded cursor-pointer whitespace-nowrap ${
                  sortBy === k
                    ? 'bg-white dark:bg-slate-950 text-slate-900 dark:text-white font-medium shadow-xs'
                    : 'text-slate-600 dark:text-slate-400'
                }`}
              >
                {lbl}
              </button>
            ))}
          </div>

          <button
            type="button"
            onClick={() => onOpenScanModal(null)}
            className="inline-flex items-center gap-1.5 px-3 py-1 rounded bg-blue-600 text-white font-medium hover:bg-blue-500 cursor-pointer whitespace-nowrap"
          >
            <HardDriveDownload className="w-3.5 h-3.5" />
            {t('+ Сканировать диск…', '+ Scan Disk…')}
          </button>
        </div>
      </div>

      {/* High-Density 60-USB Register Table */}
      <div className="border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-950 overflow-hidden">
        <div className="overflow-x-auto max-h-[calc(100vh-205px)]">
          <table className="w-full text-left border-collapse text-xs">
            <thead className="sticky top-0 z-10 bg-slate-100 dark:bg-slate-900 border-b border-slate-200 dark:border-slate-800 text-slate-600 dark:text-slate-400">
              <tr>
                <th className="py-1.5 px-2.5 font-medium w-24">{t('# · Наклейка', '# · Code')}</th>
                <th className="py-1.5 px-2.5 font-medium">{t('Имя диска в каталоге', 'Catalog Disk Label')}</th>
                <th className="py-1.5 px-2.5 font-medium hidden lg:table-cell">
                  {t('Модель · Заметка', 'Model · Note')}
                </th>
                <th className="py-1.5 px-2.5 font-medium whitespace-nowrap">
                  {t('ФС · Скан', 'FS · Scanned')}
                </th>
                <th className="py-1.5 px-2.5 font-medium text-right whitespace-nowrap">
                  {t('Занято / Ёмкость', 'Used / Capacity')}
                </th>
                <th className="py-1.5 px-2.5 font-medium text-right whitespace-nowrap">
                  {t('Видео · Индекс', 'Videos · Indexed')}
                </th>
                <th className="py-1.5 px-2.5 font-medium text-right whitespace-nowrap">
                  {t('Управление и переход', 'Manage & Open')}
                </th>
              </tr>
            </thead>
            <tbody className="divide-y divide-slate-200/70 dark:divide-slate-800/70">
              {filteredVolumes.map((vol) => {
                const pct =
                  vol.capacityBytes > 0 ? Math.min(100, Math.round((vol.usedBytes / vol.capacityBytes) * 100)) : 0;
                const isRenaming = renamingId === vol.id;

                return (
                  <tr
                    key={vol.id}
                    className="hover:bg-slate-50 dark:hover:bg-slate-900/60 transition-colors"
                  >
                    <td className="py-1.5 px-2.5 font-mono tabular-nums whitespace-nowrap">
                      <span className="font-semibold text-slate-900 dark:text-white">#{vol.id}</span>
                      <span className="ml-1.5 text-[11px] text-slate-500">[{vol.diskCode}]</span>
                    </td>

                    <td className="py-1.5 px-2.5 max-w-[270px]">
                      {isRenaming ? (
                        <div className="flex items-center gap-1">
                          <input
                            ref={renameInputRef}
                            type="text"
                            value={renameDraft}
                            onChange={(e) => setRenameDraft(e.target.value)}
                            onKeyDown={(e) => {
                              if (e.key === 'Enter') {
                                e.preventDefault();
                                void commitRename(vol.id);
                              } else if (e.key === 'Escape') {
                                e.preventDefault();
                                setRenamingId(null);
                              }
                            }}
                            className="flex-1 min-w-0 px-2 py-0.5 rounded border border-blue-500 bg-white dark:bg-slate-950 text-slate-900 dark:text-white focus:outline-none"
                          />
                          <button
                            type="button"
                            onClick={() => void commitRename(vol.id)}
                            className="p-1 rounded bg-blue-600 text-white cursor-pointer"
                          >
                            <Check className="w-3 h-3" />
                          </button>
                          <button
                            type="button"
                            onClick={() => setRenamingId(null)}
                            className="p-1 rounded text-slate-500 hover:text-slate-900 dark:hover:text-white cursor-pointer"
                          >
                            <X className="w-3 h-3" />
                          </button>
                        </div>
                      ) : (
                        <div
                          className="flex items-center gap-1.5 min-w-0 group cursor-pointer"
                          onDoubleClick={() => startRename(vol)}
                          title={t('Двойной клик для быстрого переименования', 'Double-click to rename')}
                        >
                          <HardDrive className="w-3.5 h-3.5 text-slate-400 shrink-0" />
                          <span className="font-medium text-slate-900 dark:text-slate-100 truncate">
                            {vol.label}
                          </span>
                          <button
                            type="button"
                            onClick={() => startRename(vol)}
                            title={t('Переименовать', 'Rename')}
                            className="opacity-0 group-hover:opacity-100 p-0.5 rounded text-slate-400 hover:text-blue-600 dark:hover:text-blue-400 cursor-pointer"
                          >
                            <Pencil className="w-3 h-3" />
                          </button>
                        </div>
                      )}
                    </td>

                    <td className="py-1.5 px-2.5 hidden lg:table-cell max-w-[260px] text-[11px] text-slate-500 dark:text-slate-400 truncate">
                      <span className="text-slate-700 dark:text-slate-300">{vol.hardwareModel}</span>
                      {vol.note ? ` · ${vol.note}` : ''}
                    </td>

                    <td className="py-1.5 px-2.5 font-mono text-[11px] text-slate-500 whitespace-nowrap">
                      <span className="text-slate-700 dark:text-slate-300">{vol.filesystem}</span>
                      <span className="mx-1.5">·</span>
                      <span>{formatUnixDate(vol.cataloguedAtUnix)}</span>
                    </td>

                    <td className="py-1.5 px-2.5 text-right font-mono tabular-nums whitespace-nowrap">
                      <div className="inline-flex items-center justify-end gap-2">
                        <span className="text-slate-800 dark:text-slate-200">
                          {formatBytes(vol.usedBytes)} / {formatBytes(vol.capacityBytes)}
                        </span>
                        <div className="w-14 h-1.5 rounded-full bg-slate-200 dark:bg-slate-800 overflow-hidden shrink-0">
                          <div
                            className="h-full rounded-full bg-blue-600 dark:bg-blue-500"
                            style={{ width: `${Math.max(4, pct)}%` }}
                          />
                        </div>
                        <span className="text-slate-500 w-8 text-right">{pct}%</span>
                      </div>
                    </td>

                    <td className="py-1.5 px-2.5 text-right font-mono tabular-nums text-slate-600 dark:text-slate-300 whitespace-nowrap">
                      {vol.videoFileCount} · {formatBytes(vol.indexedBytes)}
                    </td>

                    <td className="py-1.5 px-2.5 text-right whitespace-nowrap">
                      <div className="inline-flex items-center gap-1">
                        <button
                          type="button"
                          onClick={() => onOpenDiskProperties(vol.id)}
                          title={t('Редактировать свойства и заметку диска', 'Edit disk properties & note')}
                          className="inline-flex items-center gap-1 px-2 py-0.5 rounded border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 cursor-pointer"
                        >
                          <Settings className="w-3 h-3" />
                          {t('Свойства', 'Props')}
                        </button>
                        <button
                          type="button"
                          onClick={() => onOpenScanModal(vol.id)}
                          title={t(`Пересканировать диск #${vol.id}`, `Rescan disk #${vol.id}`)}
                          className="inline-flex items-center gap-1 px-2 py-0.5 rounded border border-slate-300 dark:border-slate-700 text-blue-600 dark:text-blue-400 hover:bg-slate-100 dark:hover:bg-slate-800 cursor-pointer"
                        >
                          <RefreshCw className="w-3 h-3" />
                          {t('Скан', 'Scan')}
                        </button>
                        <button
                          type="button"
                          onClick={() => onOpenSearch(vol.id)}
                          className="inline-flex items-center gap-1 px-2 py-0.5 rounded border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 cursor-pointer"
                        >
                          <Search className="w-3 h-3" />
                          {t('Поиск', 'Search')}
                        </button>
                        <button
                          type="button"
                          onClick={() => onOpenSlices(vol.id)}
                          className="px-2 py-0.5 rounded border border-slate-300 dark:border-slate-700 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-800 cursor-pointer"
                        >
                          {t('Разрезы', 'Slices')}
                        </button>
                        <button
                          type="button"
                          onClick={() => onOpenSunburst(vol.id)}
                          className="inline-flex items-center gap-1 px-2 py-0.5 rounded bg-blue-600 text-white font-medium hover:bg-blue-500 cursor-pointer"
                        >
                          <PieChart className="w-3 h-3" />
                          {t('Карта', 'Map')}
                        </button>
                      </div>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  );
};
