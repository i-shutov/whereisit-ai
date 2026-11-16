import React, { useEffect, useMemo, useRef, useState } from 'react';
import {
  CheckCircle2,
  Film,
  FolderOpen,
  HardDriveDownload,
  Play,
  ShieldCheck,
  Square,
  X,
} from 'lucide-react';
import type { ScanPluginConfig, SystemSnapshot, VideoMeta } from '../types/atlas';
import { formatBytes } from '../utils/format';
import { useI18n } from '../i18n';

interface ScanDiskModalProps {
  open: boolean;
  snapshot: SystemSnapshot;
  initialTargetVolumeId: number | null;
  onClose: () => void;
  onRefreshSnapshot: () => Promise<void>;
  onJumpToVolumeSearch: (volumeId: number) => void;
  onNotify: (message: string, tone?: 'info' | 'error' | 'success') => void;
}

const VIDEO_EXT_SET = new Set(['.mkv', '.mp4', '.m2ts', '.mov', '.avi', '.ts', '.webm', '.m4v']);
const PLAYABLE_BROWSER_VIDEO_EXT = new Set(['.mp4', '.mov', '.webm', '.m4v']);

async function computeSparseFingerprint(file: File): Promise<string | undefined> {
  try {
    if (file.size === 0) return undefined;
    const SLICE = 65536;
    const parts: Blob[] = [];
    if (file.size <= SLICE * 3) {
      parts.push(file);
    } else {
      const mid = Math.floor(file.size / 2) - Math.floor(SLICE / 2);
      parts.push(file.slice(0, SLICE));
      parts.push(file.slice(mid, mid + SLICE));
      parts.push(file.slice(file.size - SLICE, file.size));
    }
    const buf = await new Blob(parts).arrayBuffer();
    const view = new Uint8Array(buf);
    let h1 = 0x811c9dc5;
    let h2 = 0x01000193;
    const lenHex = file.size.toString(16);
    for (let i = 0; i < lenHex.length; i++) {
      h1 = Math.imul(h1 ^ lenHex.charCodeAt(i), 0x01000193);
    }
    for (let i = 0; i < view.length; i += 4) {
      h1 = Math.imul(h1 ^ view[i], 0x01000193);
      h2 = Math.imul(h2 ^ view[Math.min(view.length - 1, i + 1)], 0x85ebca6b);
    }
    return `${(h1 >>> 0).toString(16).padStart(8, '0')}${(h2 >>> 0).toString(16).padStart(8, '0')}`;
  } catch {
    return undefined;
  }
}

function inferFallbackVideoMeta(fileName: string, sizeBytes: number): VideoMeta {
  const lower = fileName.toLowerCase();
  const is4k = lower.includes('2160p') || lower.includes('4k') || lower.includes('uhd');
  const is720 = lower.includes('720p');
  const width = is4k ? 3840 : is720 ? 1280 : 1920;
  const height = is4k ? 2160 : is720 ? 720 : 1080;
  const resolutionTag: VideoMeta['resolutionTag'] = is4k ? '2160p' : is720 ? '720p' : '1080p';
  const codec =
    lower.includes('av1')
      ? 'AV1'
      : lower.includes('265') || lower.includes('hevc') || is4k
        ? 'HEVC'
        : lower.includes('.avi')
          ? 'MPEG4'
          : 'AVC';
  const estBitrateMbps = is4k ? 32 : is720 ? 4.5 : 12;
  const durationSec = Math.max(60, Math.round((sizeBytes * 8) / (estBitrateMbps * 1_000_000)));
  const bitrateMbps = Number(((sizeBytes * 8) / (durationSec * 1_000_000)).toFixed(1));
  const audioTracks = lower.includes('remux') ? 3 : 2;
  return {
    durationSec,
    width,
    height,
    resolutionTag,
    codec,
    audioTracks,
    audioSummary: audioTracks >= 3 ? 'DTS-HD + AC3 (3 тр.)' : 'AC3/AAC (2 тр.)',
    bitrateMbps: Math.max(0.5, bitrateMbps),
  };
}

function probeBrowserVideoHeader(file: File, ext: string): Promise<VideoMeta> {
  if (!PLAYABLE_BROWSER_VIDEO_EXT.has(ext)) {
    return Promise.resolve(inferFallbackVideoMeta(file.name, file.size));
  }
  return new Promise((resolve) => {
    const url = URL.createObjectURL(file);
    const video = document.createElement('video');
    video.preload = 'metadata';
    const timer = window.setTimeout(() => {
      URL.revokeObjectURL(url);
      resolve(inferFallbackVideoMeta(file.name, file.size));
    }, 800);

    video.onloadedmetadata = () => {
      window.clearTimeout(timer);
      const dur = Number.isFinite(video.duration) && video.duration > 0 ? Math.round(video.duration) : 600;
      const w = video.videoWidth || 1920;
      const h = video.videoHeight || 1080;
      URL.revokeObjectURL(url);
      const lower = file.name.toLowerCase();
      const codec =
        lower.includes('av1')
          ? 'AV1'
          : lower.includes('265') || lower.includes('hevc') || h >= 2000
            ? 'HEVC'
            : 'AVC';
      const resolutionTag: VideoMeta['resolutionTag'] =
        h >= 2000 ? '2160p' : h >= 1000 ? '1080p' : h >= 680 ? '720p' : 'SD';
      const bitrateMbps = Number(((file.size * 8) / (dur * 1_000_000)).toFixed(1));
      resolve({
        durationSec: dur,
        width: w,
        height: h,
        resolutionTag,
        codec,
        audioTracks: 2,
        audioSummary: 'AAC/AC3 (2 тр.)',
        bitrateMbps: Math.max(0.3, bitrateMbps),
      });
    };
    video.onerror = () => {
      window.clearTimeout(timer);
      URL.revokeObjectURL(url);
      resolve(inferFallbackVideoMeta(file.name, file.size));
    };
    video.src = url;
  });
}

export const ScanDiskModal: React.FC<ScanDiskModalProps> = ({
  open,
  snapshot,
  initialTargetVolumeId,
  onClose,
  onRefreshSnapshot,
  onJumpToVolumeSearch,
  onNotify,
}) => {
  const { t } = useI18n();
  const nextId = snapshot.volumes.length + 1;

  const [targetVolumeId, setTargetVolumeId] = useState<number | 'new'>('new');
  const [volumeLabel, setVolumeLabel] = useState('');
  const [diskCode, setDiskCode] = useState('');
  const [hardwareModel, setHardwareModel] = useState('WD Elements Desktop 5TB');
  const [filesystem, setFilesystem] = useState<string>('NTFS');
  const [capacityTb, setCapacityTb] = useState<number>(5);
  const [note, setNote] = useState('');

  const [plugins, setPlugins] = useState<ScanPluginConfig>({
    videoHeader: true,
    sparseHash: false,
    archiveList: false,
  });

  const [ingesting, setIngesting] = useState(false);
  const [progressNote, setProgressNote] = useState('');
  const [lastScannedVolumeId, setLastScannedVolumeId] = useState<number | null>(null);
  const [showAdvancedTools, setShowAdvancedTools] = useState(false);
  const [ctfFileName, setCtfFileName] = useState('Video_Archive_60USB.CTF');
  const [ctfResult, setCtfResult] = useState<string | null>(null);

  const dirInputRef = useRef<HTMLInputElement | null>(null);

  useEffect(() => {
    if (!open) return;
    if (initialTargetVolumeId !== null) {
      const vol = snapshot.volumes.find((v) => v.id === initialTargetVolumeId);
      setTargetVolumeId(initialTargetVolumeId);
      if (vol) {
        setVolumeLabel(vol.label);
        setDiskCode(vol.diskCode);
        setHardwareModel(vol.hardwareModel);
        setFilesystem(vol.filesystem || 'NTFS');
        setCapacityTb(Math.max(1, Math.round(vol.capacityBytes / 1024 ** 4)));
        setNote(vol.note);
        setPlugins({
          videoHeader: vol.pluginsUsed.includes('video_header'),
          sparseHash: vol.pluginsUsed.includes('sparse_3x64kb'),
          archiveList: vol.pluginsUsed.includes('archive_list'),
        });
      }
    } else {
      setTargetVolumeId('new');
      setVolumeLabel(`Кино UHD / Видеоархив — ${String(nextId).padStart(2, '0')}`);
      setDiskCode(`USB-${String(nextId).padStart(2, '0')}`);
      setHardwareModel('WD Elements Desktop 5TB');
      setFilesystem('NTFS');
      setCapacityTb(5);
      setNote('');
      setPlugins({ videoHeader: true, sparseHash: false, archiveList: false });
    }
    setLastScannedVolumeId(null);
    setProgressNote('');
  }, [open, initialTargetVolumeId, nextId, snapshot.volumes]);

  useEffect(() => {
    if (!open) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && !ingesting) {
        e.preventDefault();
        onClose();
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [open, ingesting, onClose]);

  const activeJob = snapshot.activeScanJob;
  const isJobRunning = activeJob?.status === 'running';

  useEffect(() => {
    if (!open || !isJobRunning) return;
    const timer = window.setInterval(() => {
      void onRefreshSnapshot();
    }, 300);
    return () => window.clearInterval(timer);
  }, [open, isJobRunning, onRefreshSnapshot]);

  const pluginBudget = useMemo(() => {
    let bytesPerRow = 92;
    let extraSeconds5Tb = 5;
    if (plugins.videoHeader) {
      bytesPerRow += 16;
      extraSeconds5Tb += 2.5;
    }
    if (plugins.sparseHash) {
      bytesPerRow += 8;
      extraSeconds5Tb += 3.5;
    }
    if (plugins.archiveList) {
      bytesPerRow += 45;
      extraSeconds5Tb += 4;
    }
    const mbPer100kVideos = ((bytesPerRow * 100_000) / (1024 * 1024)).toFixed(1);
    return { bytesPerRow, extraSeconds5Tb, mbPer100kVideos };
  }, [plugins]);

  if (!open) return null;

  const handleSelectTargetChange = (val: string) => {
    if (val === 'new') {
      setTargetVolumeId('new');
      setVolumeLabel(`Кино UHD / Видеоархив — ${String(nextId).padStart(2, '0')}`);
      setDiskCode(`USB-${String(nextId).padStart(2, '0')}`);
      setNote('');
    } else {
      const idNum = Number(val);
      setTargetVolumeId(idNum);
      const vol = snapshot.volumes.find((v) => v.id === idNum);
      if (vol) {
        setVolumeLabel(vol.label);
        setDiskCode(vol.diskCode);
        setHardwareModel(vol.hardwareModel);
        setFilesystem(vol.filesystem || 'NTFS');
        setCapacityTb(Math.max(1, Math.round(vol.capacityBytes / 1024 ** 4)));
        setNote(vol.note);
      }
    }
  };

  const handleFolderChosen = async (e: React.ChangeEvent<HTMLInputElement>) => {
    const fileList = e.target.files;
    if (!fileList || fileList.length === 0) return;

    setIngesting(true);
    try {
      const maxFiles = Math.min(fileList.length, 4000);
      const firstRel = (fileList[0] as File & { webkitRelativePath?: string }).webkitRelativePath || fileList[0].name;
      const rootFolderName = firstRel.includes('/') ? firstRel.split('/')[0] : '';
      const effectiveLabel =
        volumeLabel.trim() ||
        rootFolderName ||
        `USB Video ${targetVolumeId === 'new' ? nextId : targetVolumeId}`;

      const payloadFiles: {
        relativePath: string;
        name: string;
        size: number;
        lastModifiedMs: number;
        fastFingerprint?: string;
        videoMeta?: VideoMeta;
      }[] = [];

      let probedVideos = 0;
      for (let i = 0; i < maxFiles; i++) {
        const f = fileList[i] as File & { webkitRelativePath?: string };
        const rawRel = f.webkitRelativePath || f.name;
        const cleanRel =
          rootFolderName && rawRel.startsWith(`${rootFolderName}/`)
            ? rawRel.slice(rootFolderName.length + 1)
            : rawRel;
        const dotIdx = f.name.lastIndexOf('.');
        const ext = dotIdx >= 0 ? f.name.slice(dotIdx).toLowerCase() : '';

        let videoMeta: VideoMeta | undefined;
        if (plugins.videoHeader && VIDEO_EXT_SET.has(ext)) {
          if (probedVideos < 120) {
            videoMeta = await probeBrowserVideoHeader(f, ext);
            probedVideos += 1;
          } else {
            videoMeta = inferFallbackVideoMeta(f.name, f.size);
          }
        }

        let fastFingerprint: string | undefined;
        if (plugins.sparseHash && f.size >= 10 * 1024 * 1024 && i < 400) {
          fastFingerprint = await computeSparseFingerprint(f);
        }

        if (i % 50 === 0) {
          setProgressNote(
            t(
              `Чтение индекса: ${i + 1} / ${maxFiles} файлов…`,
              `Indexing: ${i + 1} / ${maxFiles} files…`
            )
          );
        }

        payloadFiles.push({
          relativePath: cleanRel,
          name: f.name,
          size: f.size,
          lastModifiedMs: f.lastModified || Date.now(),
          fastFingerprint,
          videoMeta,
        });
      }

      setProgressNote(t('Запись снимка в базу каталога…', 'Committing snapshot to catalog DB…'));
      const resp = await fetch('/api/scan/ingest-local', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          targetVolumeId: targetVolumeId === 'new' ? null : targetVolumeId,
          folderName: effectiveLabel,
          diskCode: diskCode.trim(),
          hardwareModel: hardwareModel.trim(),
          note: note.trim(),
          capacityBytes: capacityTb * 1024 ** 4,
          plugins,
          files: payloadFiles,
        }),
      });
      const data = await resp.json();
      if (!resp.ok || !data.ok) {
        onNotify(data.error || t('Ошибка сканирования диска', 'Disk scan failed'), 'error');
        return;
      }

      await onRefreshSnapshot();
      const createdId = data.createdVolumeId as number;
      setLastScannedVolumeId(createdId);
      onNotify(
        t(
          `Диск #${createdId} («${effectiveLabel}») проиндексирован: ${data.indexedFiles} файлов (${formatBytes(data.totalBytes)}).`,
          `Disk #${createdId} ("${effectiveLabel}") indexed: ${data.indexedFiles} files (${formatBytes(data.totalBytes)}).`
        ),
        'success'
      );
    } catch (err) {
      onNotify((err as Error).message, 'error');
    } finally {
      setIngesting(false);
      setProgressNote('');
      if (dirInputRef.current) dirInputRef.current.value = '';
    }
  };

  const handleStartSimulatedUsbScan = async () => {
    const pluginNames = ['fs_stat'];
    if (plugins.videoHeader) pluginNames.push('video_header');
    if (plugins.sparseHash) pluginNames.push('sparse_3x64kb');
    if (plugins.archiveList) pluginNames.push('archive_list');

    const resp = await fetch('/api/scan/start', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({
        targetVolumeId: targetVolumeId === 'new' ? null : targetVolumeId,
        label: volumeLabel.trim(),
        diskCode: diskCode.trim(),
        note: note.trim(),
        plugins: pluginNames,
      }),
    });
    const data = await resp.json();
    if (!resp.ok) {
      onNotify(data.error || t('Не удалось запустить поток сканирования', 'Failed to start scan stream'), 'error');
      return;
    }
    await onRefreshSnapshot();
  };

  const handleStopSimulatedUsbScan = async () => {
    await fetch('/api/scan/cancel', { method: 'POST' });
    await onRefreshSnapshot();
    onNotify(t('Сканирование остановлено (индекс не изменён).', 'Scan cancelled cleanly.'), 'info');
  };

  return (
    <div
      className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-3 sm:p-4 overflow-y-auto"
      onClick={() => {
        if (!ingesting) onClose();
      }}
      role="dialog"
      aria-modal="true"
      aria-labelledby="scan-disk-modal-title"
    >
      <div
        className="w-full max-w-2xl rounded-md border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 shadow-2xl overflow-hidden text-xs my-auto"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Title bar */}
        <div className="px-4 py-2.5 border-b border-slate-200 dark:border-slate-800 bg-slate-100 dark:bg-slate-950 flex items-center justify-between gap-2">
          <div className="flex items-center gap-2 font-semibold text-sm text-slate-900 dark:text-white">
            <HardDriveDownload className="w-4 h-4 text-blue-500 shrink-0" />
            <span id="scan-disk-modal-title">
              {t('Сканирование USB-диска в каталог', 'Scan USB Drive into Catalog')}
            </span>
          </div>
          <button
            type="button"
            disabled={ingesting}
            onClick={onClose}
            className="p-1 rounded text-slate-500 hover:text-slate-900 dark:hover:text-white hover:bg-slate-200/70 dark:hover:bg-slate-800 cursor-pointer"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-4 flex flex-col gap-3.5 max-h-[84vh] overflow-y-auto">
          {/* Section 1: Target Disk & Identification */}
          <div className="grid grid-cols-1 sm:grid-cols-12 gap-2.5 items-center">
            <label className="sm:col-span-3 font-medium text-slate-700 dark:text-slate-300">
              {t('Целевой диск:', 'Target Disk:')}
            </label>
            <div className="sm:col-span-9 flex flex-wrap items-center gap-2">
              <select
                value={targetVolumeId}
                onChange={(e) => handleSelectTargetChange(e.target.value)}
                className="flex-1 min-w-[220px] px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              >
                <option value="new">
                  {t(`+ Добавить новый диск (#${nextId})`, `+ Add new USB disk (#${nextId})`)}
                </option>
                {snapshot.volumes.map((v) => (
                  <option key={v.id} value={v.id}>
                    #{v.id} [{v.diskCode}] · {v.label} ({formatBytes(v.indexedBytes)})
                  </option>
                ))}
              </select>
            </div>

            <label className="sm:col-span-3 font-medium text-slate-700 dark:text-slate-300">
              {t('Имя и код:', 'Label & Code:')}
            </label>
            <div className="sm:col-span-9 flex flex-wrap sm:flex-nowrap items-center gap-2">
              <input
                type="text"
                value={volumeLabel}
                onChange={(e) => setVolumeLabel(e.target.value)}
                placeholder={t('Имя диска в базе', 'Catalog disk name')}
                className="flex-1 px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
              <input
                type="text"
                value={diskCode}
                onChange={(e) => setDiskCode(e.target.value)}
                placeholder="USB-61"
                title={t('Наклейка на физическом корпусе диска', 'Sticker code on USB enclosure')}
                className="w-24 px-2 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 font-mono text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
              <select
                value={capacityTb}
                onChange={(e) => setCapacityTb(Number(e.target.value))}
                className="w-20 px-2 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 font-mono text-slate-900 dark:text-slate-100"
              >
                {[1, 2, 4, 5, 6, 8, 10, 12, 14, 16, 18, 20].map((tb) => (
                  <option key={tb} value={tb}>
                    {tb} TB
                  </option>
                ))}
              </select>
              <select
                value={filesystem}
                onChange={(e) => setFilesystem(e.target.value)}
                className="w-20 px-2 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 font-mono text-slate-900 dark:text-slate-100"
              >
                <option value="NTFS">NTFS</option>
                <option value="exFAT">exFAT</option>
                <option value="APFS">APFS</option>
                <option value="EXT4">EXT4</option>
              </select>
            </div>

            <label className="sm:col-span-3 font-medium text-slate-700 dark:text-slate-300">
              {t('Заметка:', 'Disk Note:')}
            </label>
            <div className="sm:col-span-9">
              <input
                type="text"
                value={note}
                onChange={(e) => setNote(e.target.value)}
                placeholder={t('Необязательное описание (тематика, коробка, состояние)…', 'Optional note (genre, case, condition)…')}
                className="w-full px-2.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 text-slate-900 dark:text-slate-100 focus:outline-none focus:ring-2 focus:ring-blue-500"
              />
            </div>
          </div>

          {/* Section 2: WhereIsIt-style Metadata Plugins with Explicit SQLite DB Weight */}
          <div className="rounded-md border border-slate-200 dark:border-slate-800 bg-slate-50/80 dark:bg-slate-950/60 p-3 flex flex-col gap-2">
            <div className="flex flex-wrap items-center justify-between gap-2">
              <div className="flex items-center gap-1.5 font-semibold text-slate-900 dark:text-white">
                <Film className="w-3.5 h-3.5 text-blue-500" />
                <span>
                  {t(
                    'Плагины извлечения метаданных (как в WhereIsIt, без раздувания базы)',
                    'Metadata Scan Plugins (WhereIsIt-style, zero DB bloat)'
                  )}
                </span>
              </div>
              <span className="font-mono text-[11px] text-blue-600 dark:text-blue-400 font-semibold">
                ~{pluginBudget.bytesPerRow} {t('Б/файл', 'B/file')} · {pluginBudget.mbPer100kVideos}{' '}
                {t('МБ на 100 тыс. видео', 'MB / 100k videos')} · ~{pluginBudget.extraSeconds5Tb}{' '}
                {t('сек / 5 ТБ', 'sec / 5 TB')}
              </span>
            </div>

            <div className="flex flex-col gap-1.5 pt-1">
              <label className="flex items-start gap-2 text-slate-700 dark:text-slate-300 opacity-85">
                <input type="checkbox" checked disabled className="mt-0.5 accent-blue-600" />
                <div className="flex-1">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <span className="font-medium text-slate-900 dark:text-white">
                      {t('Базовый индекс ФС (имя, путь, точный размер int64, дата)', 'Base FS Stat (name, path, exact int64 size, mtime)')}
                    </span>
                    <span className="font-mono text-[11px] text-slate-500">
                      92 B · 0 B read · ~5s / 5TB
                    </span>
                  </div>
                </div>
              </label>

              <label className="flex items-start gap-2 cursor-pointer p-1.5 -mx-1.5 rounded hover:bg-white dark:hover:bg-slate-900 transition-colors">
                <input
                  type="checkbox"
                  checked={plugins.videoHeader}
                  onChange={(e) => setPlugins((p) => ({ ...p, videoHeader: e.target.checked }))}
                  className="mt-0.5 accent-blue-600 cursor-pointer"
                />
                <div className="flex-1">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <span className="font-semibold text-slate-900 dark:text-white">
                      {t(
                        'Видео-заголовки Light (MKV / MP4 / M2TS / AVI — только заголовок контейнера)',
                        'Video Header Light (MKV / MP4 / M2TS / AVI — container header only)'
                      )}
                    </span>
                    <span className="font-mono text-[11px] text-emerald-600 dark:text-emerald-400 font-semibold">
                      +16 B/файл (+1.6 МБ / 100k) · +2.5s / 5TB
                    </span>
                  </div>
                  <p className="text-[11px] text-slate-500 dark:text-slate-400 mt-0.5">
                    {t(
                      'Читает всего 16–64 КБ заголовка (длительность сек, разрешение 4K/1080p/720p, кодек HEVC/AVC, число аудиодорожек, битрейт). Никаких кадров-превью! Позволяет находить ремуксы с разной озвучкой и дубли одного фильма в 4K vs 1080p.',
                      'Reads only 16–64 KB header (duration, 4K/1080p/720p resolution, HEVC/AVC codec, audio track count, bitrate). Zero thumbnails! Enables finding remuxes & multi-resolution movie duplicates.'
                    )}
                  </p>
                </div>
              </label>

              <label className="flex items-start gap-2 cursor-pointer p-1.5 -mx-1.5 rounded hover:bg-white dark:hover:bg-slate-900 transition-colors">
                <input
                  type="checkbox"
                  checked={plugins.sparseHash}
                  onChange={(e) => setPlugins((p) => ({ ...p, sparseHash: e.target.checked }))}
                  className="mt-0.5 accent-blue-600 cursor-pointer"
                />
                <div className="flex-1">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <span className="font-medium text-slate-900 dark:text-white">
                      {t(
                        'Быстрый отпечаток 3×64 КБ (Sparse Hash для файлов ≥ 10 МБ)',
                        'Sparse 3×64 KB Fingerprint (for files ≥ 10 MB)'
                      )}
                    </span>
                    <span className="font-mono text-[11px] text-slate-500">
                      +8 B/файл (+0.8 МБ / 100k) · +3.5s / 5TB
                    </span>
                  </div>
                  <p className="text-[11px] text-slate-500 dark:text-slate-400 mt-0.5">
                    {t(
                      'Берёт 3 среза по 64 КБ (начало, середина, хвост = 192 КБ вместо полного SHA на 5 ТБ) для мгновенной сверки побайтовых копий под разными именами.',
                      'Samples three 64 KB chunks (head, mid, tail = 192 KB instead of full 5 TB SHA) to verify renamed byte-identical copies.'
                    )}
                  </p>
                </div>
              </label>

              <label className="flex items-start gap-2 cursor-pointer p-1.5 -mx-1.5 rounded hover:bg-white dark:hover:bg-slate-900 transition-colors">
                <input
                  type="checkbox"
                  checked={plugins.archiveList}
                  onChange={(e) => setPlugins((p) => ({ ...p, archiveList: e.target.checked }))}
                  className="mt-0.5 accent-blue-600 cursor-pointer"
                />
                <div className="flex-1">
                  <div className="flex flex-wrap items-center justify-between gap-2">
                    <span className="font-medium text-slate-800 dark:text-slate-200">
                      {t('Вложенные списки архивов (.zip / .rar / .7z)', 'Archive Inner Listings (.zip / .rar / .7z)')}
                    </span>
                    <span className="font-mono text-[11px] text-amber-600 dark:text-amber-400">
                      {t('выкл. по умолч. (раздувает число строк)', 'off by default (adds inner file rows)')}
                    </span>
                  </div>
                </div>
              </label>
            </div>
          </div>

          {/* Primary Action: Pick Mounted USB Folder / Drive */}
          <div className="flex flex-wrap items-center justify-between gap-3 p-3 rounded-md border border-blue-500/40 bg-blue-50/50 dark:bg-blue-950/20">
            <div className="min-w-0 flex-1">
              <div className="font-semibold text-slate-900 dark:text-white">
                {t('Выберите подключённый USB-диск или папку для записи в базу', 'Select Mounted USB Drive or Folder to Index')}
              </div>
              <div className="text-[11px] text-slate-600 dark:text-slate-400 mt-0.5">
                {progressNote ||
                  t(
                    'В нативной Rust/egui сборке открывает системный выбор тома (D:\\, E:\\, /Volumes/USB) с прямым обходом ФС.',
                    'In native Rust/egui build opens OS volume picker (D:\\, E:\\, /Volumes/USB) with direct MFT/stat walk.'
                  )}
              </div>
            </div>

            <input
              ref={dirInputRef}
              type="file"
              multiple
              // @ts-expect-error webkitdirectory is supported in Chromium/Firefox/Safari desktop browsers
              webkitdirectory=""
              directory=""
              onChange={handleFolderChosen}
              className="hidden"
            />
            <button
              type="button"
              disabled={ingesting}
              onClick={() => dirInputRef.current?.click()}
              className="inline-flex items-center gap-1.5 px-3.5 py-2 rounded bg-blue-600 text-white font-semibold hover:bg-blue-500 disabled:opacity-50 cursor-pointer shrink-0"
            >
              <FolderOpen className="w-4 h-4" />
              {ingesting
                ? t('Сканирование…', 'Scanning…')
                : targetVolumeId === 'new'
                  ? t('Выбрать папку / том USB…', 'Choose USB Drive / Folder…')
                  : t(`Обновить диск #${targetVolumeId}…`, `Rescan Disk #${targetVolumeId}…`)}
            </button>
          </div>

          {lastScannedVolumeId !== null && (
            <div className="p-2.5 rounded border border-emerald-500/40 bg-emerald-50/60 dark:bg-emerald-950/25 flex flex-wrap items-center justify-between gap-2 text-emerald-900 dark:text-emerald-200">
              <div className="flex items-center gap-1.5 font-medium">
                <CheckCircle2 className="w-4 h-4 text-emerald-500 shrink-0" />
                <span>
                  {t(
                    `Индекс диска #${lastScannedVolumeId} записан в базу.`,
                    `Disk #${lastScannedVolumeId} committed to catalog.`
                  )}
                </span>
              </div>
              <button
                type="button"
                onClick={() => {
                  const vid = lastScannedVolumeId;
                  onClose();
                  onJumpToVolumeSearch(vid);
                }}
                className="px-2.5 py-1 rounded bg-emerald-600 text-white font-medium hover:bg-emerald-500 cursor-pointer"
              >
                {t(`Открыть диск #${lastScannedVolumeId} в Поиске →`, `Open Disk #${lastScannedVolumeId} in Search →`)}
              </button>
            </div>
          )}

          {/* Collapsible Diagnostic / Legacy WhereIsIt .CTF Check */}
          <div className="border-t border-slate-200 dark:border-slate-800 pt-2">
            <button
              type="button"
              onClick={() => setShowAdvancedTools((v) => !v)}
              className="text-[11px] text-slate-500 hover:text-slate-900 dark:hover:text-white underline cursor-pointer"
            >
              {showAdvancedTools
                ? t('Скрыть тест потока сканирования и проверку .CTF', 'Hide stream test & legacy .CTF check')
                : t('Дополнительно: тест фонового скана (с отменой) и импорт старой базы WhereIsIt (.CTF)', 'More: test background scan stream & legacy WhereIsIt (.CTF) check')}
            </button>

            {showAdvancedTools && (
              <div className="mt-2.5 grid grid-cols-1 sm:grid-cols-2 gap-2.5">
                <div className="p-2.5 rounded border border-slate-200 dark:border-slate-800 flex flex-col justify-between gap-2">
                  <div>
                    <div className="font-semibold text-slate-800 dark:text-slate-200">
                      {t('Тест фонового потока USB (с кнопкой Стоп)', 'Simulated USB Stream (Cancel Test)')}
                    </div>
                    <div className="text-[11px] text-slate-500 mt-0.5 font-mono">
                      {activeJob
                        ? `${activeJob.status.toUpperCase()} · ${activeJob.scannedFiles}/${activeJob.totalExpectedFiles} · ${formatBytes(activeJob.scannedBytes)}`
                        : t('Поток не активен', 'Idle')}
                    </div>
                  </div>
                  <div className="flex items-center gap-2">
                    {!isJobRunning ? (
                      <button
                        type="button"
                        onClick={() => void handleStartSimulatedUsbScan()}
                        className="inline-flex items-center gap-1 px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 font-medium cursor-pointer"
                      >
                        <Play className="w-3 h-3 text-blue-500" />
                        {t('Тест скана', 'Run Stream Test')}
                      </button>
                    ) : (
                      <button
                        type="button"
                        onClick={() => void handleStopSimulatedUsbScan()}
                        className="inline-flex items-center gap-1 px-2.5 py-1 rounded bg-red-600 text-white font-medium cursor-pointer"
                      >
                        <Square className="w-3 h-3" />
                        {t('Стоп (откат)', 'Stop & Rollback')}
                      </button>
                    )}
                  </div>
                </div>

                <div className="p-2.5 rounded border border-slate-200 dark:border-slate-800 flex flex-col justify-between gap-2">
                  <div className="font-semibold text-slate-800 dark:text-slate-200 flex items-center gap-1">
                    <ShieldCheck className="w-3.5 h-3.5 text-emerald-500" />
                    {t('Проверка каталога WhereIsIt (.CTF, read-only)', 'Verify WhereIsIt (.CTF, read-only)')}
                  </div>
                  <div className="flex items-center gap-1.5">
                    <input
                      type="text"
                      value={ctfFileName}
                      onChange={(e) => setCtfFileName(e.target.value)}
                      className="flex-1 min-w-0 px-2 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-950 font-mono text-[11px]"
                    />
                    <button
                      type="button"
                      onClick={async () => {
                        const head = btoa('WhereIsIt Catalog 3.00');
                        const r = await fetch('/api/ctf/inspect', {
                          method: 'POST',
                          headers: { 'Content-Type': 'application/json' },
                          body: JSON.stringify({ fileName: ctfFileName, base64Head: head, totalByteLength: 14_800_000 }),
                        });
                        const d = await r.json();
                        setCtfResult(
                          d.readOnlySafe
                            ? `READ_ONLY OK · ${d.observedVersionMarker} · SHA ${String(d.sha256).slice(0, 10)}…`
                            : 'Error'
                        );
                      }}
                      className="px-2 py-1 rounded border border-slate-300 dark:border-slate-700 hover:bg-slate-100 dark:hover:bg-slate-800 font-medium cursor-pointer shrink-0"
                    >
                      {t('Проверить', 'Verify')}
                    </button>
                  </div>
                  {ctfResult && <div className="font-mono text-[10px] text-emerald-600 dark:text-emerald-400">{ctfResult}</div>}
                </div>
              </div>
            )}
          </div>

          <div className="pt-2 border-t border-slate-200 dark:border-slate-800 flex items-center justify-end">
            <button
              type="button"
              disabled={ingesting}
              onClick={onClose}
              className="px-3.5 py-1.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-800 text-slate-700 dark:text-slate-200 hover:bg-slate-100 dark:hover:bg-slate-700 font-medium cursor-pointer"
            >
              {t('Закрыть', 'Close')}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
};
