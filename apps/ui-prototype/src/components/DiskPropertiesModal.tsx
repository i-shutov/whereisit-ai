import React, { useEffect, useState } from 'react';
import { HardDrive, RefreshCw, Save, X } from 'lucide-react';
import type { CatalogVolume } from '../types/atlas';
import { formatBytes, formatUnixDate } from '../utils/format';
import { useI18n } from '../i18n';

interface DiskPropertiesModalProps {
  volume: CatalogVolume | null;
  onClose: () => void;
  onSaved: () => Promise<void>;
  onOpenRescan: (volumeId: number) => void;
  onNotify: (message: string, tone?: 'info' | 'error' | 'success') => void;
}

const CAPACITY_OPTIONS_TB = [1, 2, 4, 5, 6, 8, 10, 12, 14, 16, 18, 20];
const TB_BYTES = 1024 ** 4;

export const DiskPropertiesModal: React.FC<DiskPropertiesModalProps> = ({
  volume,
  onClose,
  onSaved,
  onOpenRescan,
  onNotify,
}) => {
  const { t } = useI18n();
  const [label, setLabel] = useState('');
  const [diskCode, setDiskCode] = useState('');
  const [hardwareModel, setHardwareModel] = useState('');
  const [filesystem, setFilesystem] = useState<string>('NTFS');
  const [capacityTb, setCapacityTb] = useState<number>(5);
  const [note, setNote] = useState('');
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!volume) return;
    setLabel(volume.label);
    setDiskCode(volume.diskCode || `USB-${String(volume.id).padStart(2, '0')}`);
    setHardwareModel(volume.hardwareModel || '');
    setFilesystem(volume.filesystem || 'NTFS');
    const approxTb = Math.max(1, Math.round(volume.capacityBytes / TB_BYTES));
    setCapacityTb(CAPACITY_OPTIONS_TB.includes(approxTb) ? approxTb : 5);
    setNote(volume.note || '');
  }, [volume]);

  useEffect(() => {
    if (!volume) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [volume, onClose]);

  if (!volume) return null;

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    const trimmedLabel = label.trim();
    if (!trimmedLabel) {
      onNotify(t('Имя диска не может быть пустым.', 'Disk name cannot be empty.'), 'error');
      return;
    }

    setSaving(true);
    try {
      const resp = await fetch('/api/volumes/update', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          volumeId: volume.id,
          label: trimmedLabel,
          diskCode: diskCode.trim() || `USB-${String(volume.id).padStart(2, '0')}`,
          hardwareModel: hardwareModel.trim(),
          filesystem,
          capacityBytes: Math.max(volume.usedBytes, capacityTb * TB_BYTES),
          note: note.trim(),
        }),
      });
      const data = await resp.json();
      if (!resp.ok || !data.ok) {
        onNotify(data.error || t('Ошибка сохранения свойств диска', 'Failed to save disk properties'), 'error');
        return;
      }
      await onSaved();
      onNotify(
        t(
          `Свойства диска #${volume.id} («${trimmedLabel}») сохранены.`,
          `Properties for disk #${volume.id} ("${trimmedLabel}") saved.`
        ),
        'success'
      );
      onClose();
    } catch (err) {
      onNotify((err as Error).message, 'error');
    } finally {
      setSaving(false);
    }
  };

  const usedPct =
    volume.capacityBytes > 0 ? Math.min(100, Math.round((volume.usedBytes / volume.capacityBytes) * 100)) : 0;

  return (
    <div
      className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4"
      onClick={onClose}
      role="dialog"
      aria-modal="true"
      aria-labelledby="disk-props-title"
    >
      {/* Classic desktop egui::Window dialog frame */}
      <div
        className="w-full max-w-lg rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 shadow-2xl overflow-hidden text-xs"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Window Title Bar */}
        <div className="px-3.5 py-2.5 border-b border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950 flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 font-semibold text-sm text-slate-900 dark:text-white">
            <HardDrive className="w-4 h-4 text-blue-500 shrink-0" />
            <span id="disk-props-title">
              {t(`Свойства USB-диска #${volume.id}`, `USB Disk #${volume.id} Properties`)}
            </span>
            <span className="font-mono text-xs text-slate-500">[{volume.diskCode}]</span>
          </div>
          <button
            type="button"
            onClick={onClose}
            aria-label={t('Закрыть окно', 'Close window')}
            className="p-1 rounded text-slate-500 hover:text-slate-900 dark:hover:text-white hover:bg-slate-200/70 dark:hover:bg-slate-800 cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-4 flex flex-col gap-3">
          {/* Editable Fields (1:1 mapping to egui Grid + TextEdit + ComboBox) */}
          <div className="grid grid-cols-1 sm:grid-cols-12 gap-2.5 items-center">
            <label className="sm:col-span-4 font-medium text-slate-700 dark:text-slate-300">
              {t('Имя диска в базе:', 'Catalog Label:')}
            </label>
            <div className="sm:col-span-8">
              <input
                type="text"
                autoFocus
                value={label}
                onChange={(e) => setLabel(e.target.value)}
                placeholder={t('Например: Кино UHD / BD-Remux — 04', 'e.g. UHD Movies / Remux — 04')}
                className="w-full px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>

            <label className="sm:col-span-4 font-medium text-slate-700 dark:text-slate-300">
              {t('Код / наклейка:', 'Sticker / Code:')}
            </label>
            <div className="sm:col-span-8 flex items-center gap-2">
              <input
                type="text"
                value={diskCode}
                onChange={(e) => setDiskCode(e.target.value)}
                placeholder="USB-04"
                className="w-32 px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 font-mono text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
              <span className="text-[11px] text-slate-500">
                {t('физическая маркировка на корпусе / кабеле', 'physical label on drive enclosure')}
              </span>
            </div>

            <label className="sm:col-span-4 font-medium text-slate-700 dark:text-slate-300">
              {t('Модель USB-диска:', 'Drive Model:')}
            </label>
            <div className="sm:col-span-8">
              <input
                type="text"
                value={hardwareModel}
                onChange={(e) => setHardwareModel(e.target.value)}
                placeholder={t('WD Elements 5TB / Seagate Expansion…', 'WD Elements 5TB / Seagate…')}
                className="w-full px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>

            <label className="sm:col-span-4 font-medium text-slate-700 dark:text-slate-300">
              {t('Ёмкость и ФС:', 'Capacity & FS:')}
            </label>
            <div className="sm:col-span-8 flex items-center gap-2">
              <select
                value={capacityTb}
                onChange={(e) => setCapacityTb(Number(e.target.value))}
                className="px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 font-mono text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              >
                {CAPACITY_OPTIONS_TB.map((tb) => (
                  <option key={tb} value={tb}>
                    {tb} TB
                  </option>
                ))}
              </select>

              <select
                value={filesystem}
                onChange={(e) => setFilesystem(e.target.value)}
                className="px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 font-mono text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              >
                <option value="NTFS">NTFS</option>
                <option value="exFAT">exFAT</option>
                <option value="APFS">APFS</option>
                <option value="EXT4">EXT4</option>
              </select>
            </div>

            <label className="sm:col-span-4 font-medium text-slate-700 dark:text-slate-300 self-start pt-1.5">
              {t('Описание / заметка:', 'Description / Note:')}
            </label>
            <div className="sm:col-span-8">
              <textarea
                rows={3}
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t(
                  'Комментарий к диску (как Disk Description в WhereIsIt): тематика, чьи рипы, состояние SMART…',
                  'Disk description (like WhereIsIt): content summary, rip notes, SMART health…'
                )}
                className="w-full px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500 resize-y"
              />
            </div>
          </div>

          {/* Read-Only Index Telemetry Block */}
          <div className="p-2.5 rounded border border-slate-200 dark:border-slate-800 bg-slate-50 dark:bg-slate-950/70 flex flex-col gap-1.5 font-mono text-[11px] text-slate-600 dark:text-slate-400">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span>
                {t('Занято / Ёмкость:', 'Used / Capacity:')}{' '}
                <strong className="text-slate-900 dark:text-white">
                  {formatBytes(volume.usedBytes)} / {formatBytes(volume.capacityBytes)} ({usedPct}%)
                </strong>
              </span>
              <span>
                {t('В индексе:', 'Indexed:')}{' '}
                <strong className="text-slate-900 dark:text-white">{formatBytes(volume.indexedBytes)}</strong>
              </span>
            </div>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span>
                {t('Объектов / Видео:', 'Entries / Video:')}{' '}
                <strong className="text-slate-900 dark:text-white">
                  {volume.entryCount} / {volume.videoFileCount}
                </strong>
              </span>
              <span>
                {t('Дата скана:', 'Scanned:')} {formatUnixDate(volume.cataloguedAtUnix)}
              </span>
            </div>
            <div className="flex flex-wrap items-center justify-between gap-2">
              <span>
                {t('Серийный № ФС:', 'Volume S/N:')} {volume.serialNumber}
              </span>
              <span>
                {t('Плагины индекса:', 'Scan plugins:')}{' '}
                <span className="text-blue-600 dark:text-blue-400">
                  {volume.pluginsUsed.join(', ')}
                </span>
              </span>
            </div>
          </div>

          {/* Bottom Window Buttons */}
          <div className="pt-2 border-t border-slate-200 dark:border-slate-800 flex flex-wrap items-center justify-between gap-2">
            <button
              type="button"
              onClick={() => {
                const vid = volume.id;
                onClose();
                onOpenRescan(vid);
              }}
              className="inline-flex items-center gap-1.5 px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-800 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 font-medium cursor-pointer"
            >
              <RefreshCw className="w-3.5 h-3.5 text-blue-500" />
              {t('Пересканировать этот диск…', 'Rescan this disk…')}
            </button>

            <div className="flex items-center gap-2">
              <button
                type="button"
                onClick={onClose}
                className="px-3 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-700 dark:text-slate-300 hover:bg-slate-100 dark:hover:bg-slate-700 font-medium cursor-pointer"
              >
                {t('Отмена', 'Cancel')}
              </button>
              <button
                type="submit"
                disabled={saving}
                className="inline-flex items-center gap-1.5 px-3.5 py-1.5 rounded bg-blue-600 text-white font-medium hover:bg-blue-500 disabled:opacity-50 cursor-pointer"
              >
                <Save className="w-3.5 h-3.5" />
                {saving ? t('Сохранение…', 'Saving…') : t('Сохранить', 'Save')}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  );
};
