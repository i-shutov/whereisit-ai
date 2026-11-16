import type { CatalogEntry, CatalogVolume, PhysicalStatus } from '../types/atlas';
import type { Lang } from '../i18n';

export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B';
  const units = ['B', 'KB', 'MB', 'GB', 'TB'];
  let val = bytes;
  let idx = 0;
  while (val >= 1024 && idx < units.length - 1) {
    val /= 1024;
    idx++;
  }
  return `${val >= 100 || idx === 0 ? val.toFixed(0) : val.toFixed(2)} ${units[idx]}`;
}

export function formatExactBytes(bytes: number, lang: Lang = 'ru'): string {
  return lang === 'en'
    ? `${bytes.toLocaleString('en-US')} bytes`
    : `${bytes.toLocaleString('ru-RU')} байт`;
}

export function formatUnixDate(unix: number): string {
  if (!unix) return '—';
  const d = new Date(unix * 1000);
  return d.toISOString().slice(0, 10);
}

export function formatDuration(seconds: number | undefined | null): string {
  if (!seconds || !Number.isFinite(seconds) || seconds <= 0) return '—';
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const rem = s % 60;
  if (h > 0) {
    return `${String(h).padStart(2, '0')}:${String(m).padStart(2, '0')}:${String(rem).padStart(2, '0')}`;
  }
  return `${String(m).padStart(2, '0')}:${String(rem).padStart(2, '0')}`;
}

export function physicalStatusMeta(
  status: PhysicalStatus,
  lang: Lang = 'ru'
): {
  shortLabel: string;
  detailLabel: string;
  textClass: string;
} {
  switch (status) {
    case 'connected':
      return {
        shortLabel: lang === 'en' ? 'Mounted' : 'Подключён',
        detailLabel: lang === 'en' ? 'Mounted in OS' : 'Подключён в ОС',
        textClass: 'text-emerald-600 dark:text-emerald-400',
      };
    case 'offline':
      return {
        shortLabel: lang === 'en' ? 'Offline' : 'В архиве',
        detailLabel: lang === 'en' ? 'Offline catalog volume' : 'Офлайн-том в каталоге',
        textClass: 'text-slate-500 dark:text-slate-400',
      };
    case 'conflict':
      return {
        shortLabel: lang === 'en' ? 'Conflict' : 'Конфликт',
        detailLabel: lang === 'en' ? 'Volume conflict' : 'Конфликт тома',
        textClass: 'text-red-600 dark:text-red-400 font-semibold',
      };
    default:
      return {
        shortLabel: lang === 'en' ? 'Cataloged' : 'В каталоге',
        detailLabel: lang === 'en' ? 'Offline indexed USB disk' : 'Офлайн USB-диск в каталоге',
        textClass: 'text-slate-500 dark:text-slate-400',
      };
  }
}

export function buildPickupCardText(
  volume: CatalogVolume,
  entry: CatalogEntry | null,
  _verifiedSha: string | null,
  lang: Lang = 'ru'
): string {
  if (lang === 'en') {
    const lines = [
      `USB DISK #${volume.id} · ${volume.diskCode} (${volume.label})`,
      `Hardware / FS: ${volume.hardwareModel} · ${volume.filesystem} · ${formatBytes(volume.usedBytes)} / ${formatBytes(volume.capacityBytes)}`,
    ];
    if (volume.note) lines.push(`Note: ${volume.note}`);
    if (entry) {
      lines.push(
        `File: ${entry.name}`,
        `Path: ${entry.relativePath}`,
        `Size: ${formatBytes(entry.size)} (${formatExactBytes(entry.size, 'en')})`
      );
      if (entry.videoMeta) {
        lines.push(
          `Video: ${formatDuration(entry.videoMeta.durationSec)} · ${entry.videoMeta.width}x${entry.videoMeta.height} (${entry.videoMeta.resolutionTag}) · ${entry.videoMeta.codec} · ${entry.videoMeta.bitrateMbps.toFixed(1)} Mbps`
        );
      }
    }
    return lines.join('\n');
  }

  const lines = [
    `USB ДИСК #${volume.id} · ${volume.diskCode} (${volume.label})`,
    `Модель / ФС: ${volume.hardwareModel} · ${volume.filesystem} · ${formatBytes(volume.usedBytes)} / ${formatBytes(volume.capacityBytes)}`,
  ];
  if (volume.note) lines.push(`Заметка: ${volume.note}`);
  if (entry) {
    lines.push(
      `Файл: ${entry.name}`,
      `Путь: ${entry.relativePath}`,
      `Размер: ${formatBytes(entry.size)} (${formatExactBytes(entry.size, 'ru')})`
    );
    if (entry.videoMeta) {
      lines.push(
        `Видео: ${formatDuration(entry.videoMeta.durationSec)} · ${entry.videoMeta.width}x${entry.videoMeta.height} (${entry.videoMeta.resolutionTag}) · ${entry.videoMeta.codec} · ${entry.videoMeta.bitrateMbps.toFixed(1)} Мбит/с`
      );
    }
  }
  return lines.join('\n');
}

export function buildFullDiskRegisterText(
  volumes: CatalogVolume[],
  _verifiedSha: string | null,
  lang: Lang = 'ru'
): string {
  if (lang === 'en') {
    const header = [
      `USB ARCHIVE REGISTER (${volumes.length} disks)`,
      `ID\tCode\tLabel\tModel\tFS\tUsed / Capacity\tNote`,
    ];
    const rows = volumes.map(
      (v) =>
        `#${v.id}\t${v.diskCode}\t${v.label}\t${v.hardwareModel}\t${v.filesystem}\t${formatBytes(v.usedBytes)} / ${formatBytes(v.capacityBytes)}\t${v.note || '—'}`
    );
    return [...header, ...rows].join('\n');
  }

  const header = [
    `РЕЕСТР USB-ДИСКОВ АРХИВА (${volumes.length} шт.)`,
    `ID\tКод\tИмя диска\tМодель\tФС\tЗанято / Ёмкость\tЗаметка`,
  ];
  const rows = volumes.map(
    (v) =>
      `#${v.id}\t${v.diskCode}\t${v.label}\t${v.hardwareModel}\t${v.filesystem}\t${formatBytes(v.usedBytes)} / ${formatBytes(v.capacityBytes)}\t${v.note || '—'}`
  );
  return [...header, ...rows].join('\n');
}
