import express from 'express';
import type { Request, Response } from 'express';
import crypto from 'node:crypto';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer as createViteServer } from 'vite';
import {
  buildMetadataCandidateIndex,
  countCandidateVolumes,
  metadataCandidateKey,
  normalizeMetadataName,
} from './src/utils/metadataCandidates';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const VIDEO_EXTENSIONS = new Set([
  'mkv', 'mp4', 'avi', 'mov', 'm4v', 'webm',
  'mpg', 'mpeg', 'wmv', 'ts', 'm2ts', 'vob', 'flv', 'iso',
]);

const AUDIO_EXTENSIONS = new Set(['flac', 'wav', 'dsf', 'ape', 'mp3', 'm4a', 'ogg', 'aiff', 'ac3', 'dts']);
const SUB_EXTENSIONS = new Set(['srt', 'ass', 'ssa', 'sub', 'idx', 'sup']);
const ARCHIVE_EXTENSIONS = new Set(['zip', 'tar', '7z', 'rar', 'gz', 'zst', 'dmg', 'img']);

type PlatformPreset = 'windows' | 'macos';
type PhysicalStatus = 'connected' | 'offline' | 'unverified' | 'conflict';

interface VideoMeta {
  durationSec: number;
  width: number;
  height: number;
  resolutionTag: '2160p' | '1080p' | '720p' | 'SD';
  codec: string;
  audioTracks: number;
  audioSummary: string;
  bitrateMbps: number;
  workKey?: string;
}

interface RawVolume {
  id: number;
  label: string;
  diskCode: string;
  hardwareModel: string;
  serialNumber: string;
  note: string;
  recordedRoot: string;
  filesystem: string;
  capacityBytes: number;
  usedBytes: number;
  pluginsUsed: string[];
  cataloguedAtUnix: number;
  sourceKind: 'archive_ctf' | 'scanned_usb' | 'local_folder';
}

interface RawEntry {
  id: number;
  volumeId: number;
  parentId: number | null;
  name: string;
  nameNorm: string;
  relativePath: string;
  ext: string;
  kind: 'file' | 'directory';
  size: number;
  modifiedUnix: number;
  category: string;
  description: string;
  tags: string;
  fastFingerprint?: string | null;
  videoMeta?: VideoMeta | null;
  opaqueFlag0x10: boolean;
}

interface LiveOsMount {
  mountPoint: string;
  physicalId: string;
  serial: string;
  label: string;
  filesystem: string;
  capacityBytes: number;
  usedBytes: number;
  isSystemReserved: boolean;
  isCloneSimulation?: boolean;
}

interface AnnotationRecord {
  id: number;
  oldEntryId: number;
  recordedDiskId: number;
  recordedPath: string;
  noteText: string;
  revision: number;
  candidateEntryId: number | null;
  confirmedEntryId: number | null;
  activeOverlay: boolean;
  updatedUnix: number;
}

interface ScanJobInternal {
  jobId: string;
  targetLabel: string;
  mountPoint: string;
  physicalId: string;
  filesystem: string;
  capacityBytes: number;
  shelfTag: string;
  status: 'running' | 'completed' | 'cancelled' | 'failed';
  aborted: boolean;
  scannedFiles: number;
  totalExpectedFiles: number;
  scannedBytes: number;
  totalExpectedBytes: number;
  currentPath: string;
  filesPerSec: number;
  bytesPerSec: number;
  duplicatesDetected: number;
  createdVolumeId: number | null;
  startedAtMs: number;
  elapsedMs: number;
  pluginsUsed?: string[];
  topFolders?: { path: string; bytes: number; files: number }[];
  error?: string;
}

interface SunburstNode {
  id: string;
  name: string;
  size: number;
  matchedBytes: number;
  itemCount: number;
  kind: 'root' | 'category' | 'volume' | 'directory' | 'file' | 'other_group';
  volumeId: number | null;
  folderPath: string | null;
  entryId: number | null;
  folderEntryId?: number | null;
  shelfTag?: string;
  physicalStatus?: PhysicalStatus;
  duplicateCount?: number;
  searchQuery?: string;
  colorIndex: number;
  children?: SunburstNode[];
}

const TB = 1024 ** 4;
const GB = 1024 ** 3;
const MB = 1024 ** 2;

let nextGlobalEntryId = 1000;
const folderCache = new Map<string, number>();

function categorizeByExt(ext: string, folderHint = ''): string {
  const fh = folderHint.toLowerCase();
  if (fh.includes('series') || fh.includes('сериал')) return 'Сериалы UHD/1080p';
  if (fh.includes('doc') || fh.includes('документ')) return 'Документалистика 4K/HD';
  if (fh.includes('concert') || fh.includes('концерт')) return 'Концерты и Музыкальное видео';
  if (fh.includes('mosfilm') || fh.includes('ussr') || fh.includes('ссср') || fh.includes('tarkovsky')) {
    return 'Классика и Кино СССР';
  }
  if (VIDEO_EXTENSIONS.has(ext)) return 'Кино UHD / BD-Remux';
  if (SUB_EXTENSIONS.has(ext) || AUDIO_EXTENSIONS.has(ext)) return 'Дорожки и Субтитры';
  if (ARCHIVE_EXTENSIONS.has(ext)) return 'Архивы и Прочее';
  return 'Кино UHD / BD-Remux';
}

function inferVideoMetaFromNameAndSize(name: string, sizeBytes: number): VideoMeta | null {
  const dotIdx = name.lastIndexOf('.');
  const ext = dotIdx > 0 ? name.slice(dotIdx + 1).toLowerCase() : '';
  if (!VIDEO_EXTENSIONS.has(ext)) return null;

  const lower = name.toLowerCase();
  let resolutionTag: '2160p' | '1080p' | '720p' | 'SD' = '1080p';
  let width = 1920;
  let height = 1080;

  if (lower.includes('2160p') || lower.includes('4k') || lower.includes('uhd') || sizeBytes >= 36 * GB) {
    resolutionTag = '2160p';
    width = 3840;
    height = 2160;
  } else if (lower.includes('720p') || (sizeBytes > 0 && sizeBytes < 5.5 * GB && !lower.includes('1080p'))) {
    resolutionTag = '720p';
    width = 1280;
    height = 720;
  } else if (lower.includes('dvd') || lower.includes('576p') || lower.includes('480p')) {
    resolutionTag = 'SD';
    width = 720;
    height = 576;
  }

  const codec =
    lower.includes('av1')
      ? 'AV1'
      : lower.includes('hevc') || lower.includes('x265') || lower.includes('h265') || resolutionTag === '2160p'
        ? 'HEVC'
        : lower.includes('vc-1') || lower.includes('vc1')
          ? 'VC-1'
          : lower.includes('mpeg2') || ext === 'vob'
            ? 'MPEG-2'
            : 'AVC';

  // Deterministic realistic movie duration if not read from live container
  const hashNum = crypto.createHash('md5').update(lower).digest()[0];
  const durationSec = 5400 + (hashNum % 42) * 120 + (hashNum % 59); // ~90m..174m
  const bitrateMbps = durationSec > 0 ? Number(((sizeBytes * 8) / durationSec / 1_000_000).toFixed(1)) : 12.0;

  const yearMatch = name.match(/\b(19\d{2}|20\d{2})\b/);
  const workKey = yearMatch
    ? `${name.slice(0, yearMatch.index).replace(/[._-]+/g, ' ').trim().toLowerCase()}::${yearMatch[1]}`
    : undefined;

  return {
    durationSec,
    width,
    height,
    resolutionTag,
    codec,
    audioTracks: resolutionTag === '2160p' ? 3 : 2,
    audioSummary: resolutionTag === '2160p' ? 'RUS+ENG DTS-HD' : 'RUS+ENG AC3',
    bitrateMbps: Math.max(0.8, bitrateMbps),
    workKey,
  };
}

function buildInitialCatalog() {
  const volumes: RawVolume[] = [];
  const entries: RawEntry[] = [];

  // Realistic 60 loose USB drives for a 99% Video Collection
  const diskProfiles: { label: string; model: string; note: string }[] = [
    { label: 'SciFi_90s_UHD_01', model: 'WD Elements 5TB', note: 'Фантастика 90-х: Терминатор 2, Матрица, Пятый элемент (4K Remux)' },
    { label: 'SciFi_2000s_UHD_02', model: 'Seagate Expansion 5TB', note: 'Фантастика 2000-х UHD BD-Remux, русские дорожки DTS-HD' },
    { label: 'Classic_Hollywood_03', model: 'Toshiba Canvio Basics 4TB', note: 'Классика Голливуда 1950–1980, Criterion 1080p/2160p' },
    { label: 'Mosfilm_Tarkovsky_4K', model: 'WD My Passport 5TB', note: 'Тарковский 4K реставрация Мосфильм + эталонные UHD ремуксы' },
    { label: 'IMAX_Blockbusters_05', model: 'Seagate Backup Plus 8TB', note: 'IMAX издания: Дюна, Интерстеллар, Оппенгеймер, Бегущий по лезвию' },
    { label: 'Compact_Rips_Road_06', model: 'WD Elements 2TB', note: 'Дорожные рипы 720p/1080p для ноутбука и планшета' },
    { label: 'Series_HBO_Vault_01', model: 'WD Elements 5TB', note: 'Сериалы HBO: Клан Сопрано, Прослушка, Чернобыль (1080p/2160p)' },
    { label: 'Series_HBO_Vault_02', model: 'WD Elements 5TB', note: 'Сериалы HBO: Игра престолов 4K Dolby Vision, Настоящий детектив' },
    { label: 'Action_90s_1080p_09', model: 'Toshiba Canvio 4TB', note: 'Боевики 80–90-х 1080p BDRip с авторскими переводами' },
    { label: 'European_Arthouse_10', model: 'Seagate Expansion 5TB', note: 'Бергман, Феллини, Антониони, Вендерс (BFI / Criterion)' },
    { label: 'UHD_Master_Backup_11', model: 'WD My Book 8TB', note: 'Холодный дубль избранных 4K ремуксов (Солярис, Blade Runner)' },
    { label: 'BBC_Nature_4K_12', model: 'WD Elements 5TB', note: 'Документалистика BBC Earth: Планета Земля II/III, Голубая планета 4K' },
    { label: 'BluRay_ISO_Full_13', model: 'Seagate IronWolf USB 8TB', note: 'Полные образы Blu-ray BDMV / ISO с меню' },
    { label: 'New_Releases_2024_14', model: 'WD My Passport 4TB', note: 'Новинки кинопроката 2023–2025 (1080p / 2160p)' },
    { label: 'Tracker_Downloads_15', model: 'Toshiba Canvio 4TB', note: 'Неразобранные раздачи с трекеров (есть переименованные дубли)' },
    { label: 'SciFi_1080p_Archive_16', model: 'WD Elements 4TB', note: 'Старый архив фантастики 1080p до перехода на 4K' },
    { label: 'Soviet_Comedies_17', model: 'Seagate Expansion 4TB', note: 'Комедии СССР: Гайдай, Рязанов, Данелия (реставрация)' },
    { label: 'Criterion_Collection_18', model: 'WD Elements 5TB', note: 'Собрание Criterion Collection #1–#400 (1080p AVC)' },
    { label: 'Nolan_Villeneuve_4K_19', model: 'WD My Book 8TB', note: 'Нолан и Вильнёв: полная фильмография UHD Remux' },
    { label: 'Anime_Ghibli_BD_20', model: 'Toshiba Canvio 4TB', note: 'Полная коллекция Studio Ghibli Blu-ray Remux RUS+JPN' },
    { label: 'Series_SciFi_Complete_21', model: 'WD Elements 5TB', note: 'Пространство (The Expanse), Вавилон 5, Звёздный крейсер Галактика' },
    { label: 'Historical_Epics_4K_22', model: 'Seagate Expansion 8TB', note: 'Историческое кино 4K: Андрей Рублёв, Гладиатор, Лоуренс Аравийский' },
    { label: 'DualAudio_Remux_23', model: 'WD Elements 5TB', note: 'Авторские сборки ремуксов с несколькими звуковыми дорожками' },
    { label: 'Space_IMAX_Vault_24', model: 'WD My Book 8TB', note: 'Космос и физика: IMAX оригиналы и документальные хроники' },
    { label: 'Live_Concerts_BD_25', model: 'Seagate Expansion 5TB', note: 'Концерты Blu-ray: Pink Floyd, Hans Zimmer, Berliner Philharmoniker' },
    { label: 'Noir_1940s_1950s_26', model: 'Toshiba Canvio 2TB', note: 'Классический фильм-нуар 1941–1958 (BDRip 1080p)' },
    { label: 'Old_HDD_Migration_27', model: 'WD Elements 2TB', note: 'Перенос со старых дисков 2015 г. (много 720p версий)' },
    { label: 'Russian_Cinema_Restored_28', model: 'WD Elements 5TB', note: 'Ленфильм и Мосфильм: реставрации в 1080p и 4K' },
    { label: 'BDMV_Raw_Streams_29', model: 'Seagate Expansion 8TB', note: 'Несжатые потоки .m2ts напрямую с UHD Blu-ray дисков' },
    { label: 'Cold_Mirror_SciFi_30', model: 'WD My Book 8TB', note: 'Резервное зеркало фантастики UHD' },
    { label: 'Festival_Winners_Cannes_31', model: 'WD Elements 5TB', note: 'Лауреаты Каннского и Венецианского фестивалей' },
    { label: 'Series_British_Drama_32', model: 'Toshiba Canvio 4TB', note: 'Британские сериалы BBC / ITV 1080p' },
    { label: 'NASA_65mm_Chronicles_33', model: 'Seagate Expansion 5TB', note: 'Архивные 65мм кинохроники Apollo и IMAX экспедиции' },
    { label: 'Custom_Dub_Vault_34', model: 'WD Elements 5TB', note: 'Ремуксы с редкими дубляжами и комментариями режиссёров' },
    { label: 'Asian_Cinema_4K_35', model: 'WD My Passport 5TB', note: 'Куросава, Вонг Карвай, Пак Чхан-ук (4K UHD)' },
    { label: 'Kubrick_Complete_UHD_36', model: 'Seagate Expansion 5TB', note: 'Стэнли Кубрик: 2001, Сияние, Барри Линдон, Цельнометаллическая оболочка' },
    { label: 'Scorsese_Coppola_4K_37', model: 'WD Elements 5TB', note: 'Крёстный отец трилогия 4K, Славные парни, Таксист, Казино' },
    { label: 'BluRay_Disc_Images_38', model: 'Seagate IronWolf USB 8TB', note: 'Зеркало ISO-образов коллекционных изданий' },
    { label: 'Criterion_Remux_Mirror_39', model: 'WD Elements 5TB', note: 'Ремуксы Criterion с добавленными русскими субтитрами и звуком' },
    { label: 'Documentary_History_40', model: 'Toshiba Canvio 4TB', note: 'Историческая документалистика: Вторая мировая, Холодная война' },
    { label: 'Archive_720p_Legacy_41', model: 'WD Elements 2TB', note: 'Старые рипы 720p на ревизию и освобождение места' },
    { label: 'Westerns_Leone_4K_42', model: 'WD My Passport 4TB', note: 'Вестерны Серджо Леоне и Джона Форда (4K UHD)' },
    { label: 'Animation_Pixar_UHD_43', model: 'Seagate Expansion 5TB', note: 'Полнометражная анимация 2160p HDR10' },
    { label: 'Opera_Ballet_BluRay_44', model: 'WD Elements 4TB', note: 'Постановки Royal Opera House и Большого театра (1080i/2160p)' },
    { label: 'Thriller_Fincher_4K_45', model: 'WD Elements 5TB', note: 'Финчер, Вильнёв, Манн: триллеры и детективы UHD' },
    { label: 'Series_AppleTV_UHD_46', model: 'Seagate Expansion 5TB', note: 'Разделение, Укрытие, Основание, Ради всего человечества (2160p)' },
    { label: 'Series_FX_AMC_1080p_47', model: 'WD Elements 5TB', note: 'Во все тяжкие, Лучше звоните Солу, Фарго (1080p BluRay)' },
    { label: 'War_Cinema_UHD_48', model: 'WD My Book 8TB', note: 'Военная драма: Иди и смотри, Апокалипсис сегодня, 1917, Дюнкерк' },
    { label: 'French_NewWave_49', model: 'Toshiba Canvio 4TB', note: 'Французская новая волна: Годар, Трюффо, Мельвиль' },
    { label: 'German_Expressionism_50', model: 'WD Elements 4TB', note: 'Метрополис, Носферату, М — реставрации Masters of Cinema' },
    { label: 'Horror_Classics_4K_51', model: 'Seagate Expansion 5TB', note: 'Нечто (1982), Чужой (1979), Экзорцист — 4K UHD Remux' },
    { label: 'Adventure_80s_UHD_52', model: 'WD Elements 5TB', note: 'Индиана Джонс 1–4 UHD, Назад в будущее трилогия 4K' },
    { label: 'VHS_LaserDisc_Digitized_53', model: 'Toshiba Canvio 2TB', note: 'Оцифровки редких кассет и LaserDisc (FFV1 / ProRes)' },
    { label: 'Sport_Motorsports_Archive_54', model: 'WD Elements 4TB', note: 'Сезоны Формулы-1 и Ле-Ман в HD/4K' },
    { label: 'Travel_Expeditions_4K_55', model: 'Seagate Expansion 5TB', note: 'Экспедиционные фильмы: Арктика, Гималаи, Памир 4K' },
    { label: 'Silent_Era_Restored_56', model: 'WD Elements 2TB', note: 'Чаплин, Китон, Эйзенштейн — реставрации с оркестром' },
    { label: 'Series_European_Noir_57', model: 'Toshiba Canvio 4TB', note: 'Скандинавский нуар: Мост, Убийство (1080p)' },
    { label: 'Music_Documentaries_58', model: 'WD Elements 4TB', note: 'The Beatles: Get Back, документальные фильмы о рок-музыке' },
    { label: 'Theater_Capture_HD_59', model: 'Seagate Expansion 4TB', note: 'National Theatre Live и театральные телеверсии' },
    { label: 'Incoming_Staging_USB_60', model: 'WD My Passport 5TB', note: 'Свежий диск для новых поступлений 2026' },
  ];

  for (let i = 0; i < 60; i++) {
    const id = i + 1;
    const prof = diskProfiles[i];
    const mMatch = prof.model.match(/(\d+)\s*TB/i);
    const capTb = mMatch ? Number(mMatch[1]) : 5;
    const capacityBytes = capTb * TB;
    const usedRatio = 0.62 + ((id * 7) % 33) / 100;
    const codeStr = `USB-${String(id).padStart(2, '0')}`;

    volumes.push({
      id,
      label: prof.label,
      diskCode: codeStr,
      hardwareModel: prof.model,
      serialNumber: `SN-${String(1000 + id * 37)}-${codeStr}`,
      note: prof.note,
      recordedRoot: 'F:\\',
      filesystem: id % 4 === 0 ? 'exFAT' : 'NTFS',
      capacityBytes,
      usedBytes: Math.floor(capacityBytes * usedRatio),
      pluginsUsed: ['fs_stat', 'video_header'],
      cataloguedAtUnix: 1714000000 + id * 86400,
      sourceKind: 'archive_ctf',
    });
  }

  const pushEntry = (e: Omit<RawEntry, 'id' | 'nameNorm' | 'ext'>): number => {
    const id = nextGlobalEntryId++;
    const dotIdx = e.name.lastIndexOf('.');
    const ext = e.kind === 'file' && dotIdx > 0 ? e.name.slice(dotIdx + 1).toLowerCase() : '';
    entries.push({
      ...e,
      id,
      nameNorm: normalizeMetadataName(e.name),
      ext,
    });
    return id;
  };

  const ensureFolderChain = (volumeId: number, folderPath: string, modUnix: number, category: string): number => {
    const parts = folderPath.split('/').filter(Boolean);
    let currentPath = '';
    let parentId: number | null = null;
    for (const part of parts) {
      currentPath = currentPath ? `${currentPath}/${part}` : part;
      const cacheKey = `${volumeId}:${currentPath}`;
      const existing = folderCache.get(cacheKey);
      if (existing !== undefined) {
        parentId = existing;
      } else {
        const createdId = pushEntry({
          volumeId,
          parentId,
          name: part,
          relativePath: currentPath,
          kind: 'directory',
          size: 0,
          modifiedUnix: modUnix,
          category,
          description: `Папка диска #${volumeId}`,
          tags: 'folder',
          opaqueFlag0x10: false,
        });
        folderCache.set(cacheKey, createdId);
        parentId = createdId;
      }
    }
    return parentId!;
  };

  const makeFp = (seed: string) =>
    crypto.createHash('sha256').update(seed).digest('hex').slice(0, 12);

  // 1. Flagship Movies with Exact Copies across USB drives
  const exactMasterMovies: {
    folder: string;
    name: string;
    size: number;
    disks: number[];
    category: string;
    desc: string;
    videoMeta: VideoMeta;
  }[] = [
    {
      folder: 'Cinema/Tarkovsky_4K/Solaris_1972',
      name: 'Солярис_1972_Mosfilm_4K_Remux.mkv',
      size: 52_237_543_014, // ~48.65 GB exact
      disks: [4, 11],
      category: 'Классика и Кино СССР',
      desc: 'UHD Blu-ray Remux 2160p HEVC FLAC 2.0 Мосфильм',
      videoMeta: {
        durationSec: 9994, // 02:46:34
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 2,
        audioSummary: 'RUS FLAC 2.0 + Comms',
        bitrateMbps: 41.8,
        workKey: 'solaris::1972',
      },
    },
    {
      folder: 'Cinema/SciFi_UHD/Blade_Runner_1982',
      name: 'Blade_Runner_1982_Final_Cut_UHD.mkv',
      size: 66_689_972_633, // ~62.11 GB exact
      disks: [4, 5, 11, 19],
      category: 'Кино UHD / BD-Remux',
      desc: 'Final Cut 2160p HDR10 TrueHD 7.1 Atmos + RUS DTS-HD',
      videoMeta: {
        durationSec: 7046, // 01:57:26
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 5,
        audioSummary: 'ENG Atmos + 4×RUS',
        bitrateMbps: 75.7,
        workKey: 'blade_runner::1982',
      },
    },
    {
      folder: 'Cinema/IMAX_Vault/Interstellar_2014',
      name: 'Interstellar_2014_IMAX_UHD_Remux.mkv',
      size: 82_033_876_992, // ~76.40 GB exact
      disks: [5, 19, 24],
      category: 'Кино UHD / BD-Remux',
      desc: 'IMAX Variable Aspect Ratio 2160p HEVC DTS-HD MA 5.1',
      videoMeta: {
        durationSec: 10144, // 02:49:04
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 3,
        audioSummary: 'RUS Dub + ENG DTS-HD',
        bitrateMbps: 64.7,
        workKey: 'interstellar::2014',
      },
    },
    {
      folder: 'Cinema/SciFi_90s/Terminator_2_1991',
      name: 'Терминатор_2_Судный_день_1991_UHD_Remux.mkv',
      size: 62_599_148_544, // ~58.30 GB
      disks: [1, 30],
      category: 'Кино UHD / BD-Remux',
      desc: 'Special Extended Edition 2160p HEVC + Гаврилов/Живов/Дубляж',
      videoMeta: {
        durationSec: 9194, // 02:33:14
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 6,
        audioSummary: '5×RUS + ENG DTS-HD',
        bitrateMbps: 54.5,
        workKey: 'terminator_2::1991',
      },
    },
    {
      folder: 'Cinema/IMAX_Vault/Dune_Part_Two_2024',
      name: 'Dune_Part_Two_2024_UHD_BDRemux_DV.mkv',
      size: 76_450_417_868, // ~71.20 GB
      disks: [5, 19],
      category: 'Кино UHD / BD-Remux',
      desc: 'Dolby Vision Profile 7 FEL TrueHD Atmos + RUS Dub',
      videoMeta: {
        durationSec: 9949, // 02:45:49
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 4,
        audioSummary: 'RUS AC3 + ENG Atmos',
        bitrateMbps: 61.5,
        workKey: 'dune_part_two::2024',
      },
    },
    {
      folder: 'Cinema/Tarkovsky_4K/Stalker_1979',
      name: 'Сталкер_1979_Criterion_1080p.mkv',
      size: 31_589_485_772, // ~29.42 GB
      disks: [4, 28],
      category: 'Классика и Кино СССР',
      desc: 'Criterion Restoration 1080p AVC LPCM 1.0',
      videoMeta: {
        durationSec: 9769, // 02:42:49
        width: 1920,
        height: 1080,
        resolutionTag: '1080p',
        codec: 'AVC',
        audioTracks: 1,
        audioSummary: 'RUS LPCM 1.0',
        bitrateMbps: 25.9,
        workKey: 'stalker::1979',
      },
    },
    {
      folder: 'Documentary/NASA_65mm/Apollo_11',
      name: 'Apollo_11_1969_65mm_Archival_2160p.mkv',
      size: 56_801_214_464, // ~52.90 GB
      disks: [24, 33],
      category: 'Документалистика 4K/HD',
      desc: '65mm Todd-AO 8K->4K Scan DTS-HD 5.1',
      videoMeta: {
        durationSec: 5582, // 01:33:02
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 2,
        audioSummary: 'RUS+ENG DTS-HD',
        bitrateMbps: 81.4,
        workKey: 'apollo_11::2019',
      },
    },
  ];

  for (const m of exactMasterMovies) {
    const fp = makeFp(`exact::${m.size}::${m.videoMeta.durationSec}`);
    for (const dId of m.disks) {
      const modUnix = 1709000000 + dId * 14200;
      const pId = ensureFolderChain(dId, m.folder, modUnix, m.category);
      pushEntry({
        volumeId: dId,
        parentId: pId,
        name: m.name,
        relativePath: `${m.folder}/${m.name}`,
        kind: 'file',
        size: m.size,
        modifiedUnix: modUnix,
        category: m.category,
        description: m.desc,
        tags: `video,${m.videoMeta.resolutionTag},${m.videoMeta.codec.toLowerCase()}`,
        fastFingerprint: fp,
        videoMeta: m.videoMeta,
        opaqueFlag0x10: false,
      });
    }
  }

  // 2. Renamed metadata candidates on other USB drives (same byte count and sampled fingerprint; not proof of equality).
  {
    const p15 = ensureFolderChain(15, 'Downloads_Torrents/UHD_Cinema', 1712600000, 'Классика и Кино СССР');
    pushEntry({
      volumeId: 15,
      parentId: p15,
      name: 'Solaris.1972.2160p.UHD.BDRemux.HEVC.FLAC.mkv',
      relativePath: 'Downloads_Torrents/UHD_Cinema/Solaris.1972.2160p.UHD.BDRemux.HEVC.FLAC.mkv',
      kind: 'file',
      size: 52_237_543_014, // identical byte-for-byte to Солярис_1972_Mosfilm_4K_Remux.mkv!
      modifiedUnix: 1712600000,
      category: 'Классика и Кино СССР',
      description: 'Кандидат по метаданным для ремукса Соляриса (англоязычное имя; содержимое не сверено)',
      tags: 'video,2160p,hevc,renamed',
      fastFingerprint: makeFp(`exact::${52_237_543_014}::9994`),
      videoMeta: {
        durationSec: 9994,
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 2,
        audioSummary: 'RUS FLAC 2.0 + Comms',
        bitrateMbps: 41.8,
        workKey: 'solaris::1972',
      },
      opaqueFlag0x10: false,
    });

    const p30 = ensureFolderChain(30, 'Cold_Backup/SciFi_Masters', 1712900000, 'Кино UHD / BD-Remux');
    pushEntry({
      volumeId: 30,
      parentId: p30,
      name: 'Бегущий_по_лезвию_1982_FinalCut_2160p_Master.mkv',
      relativePath: 'Cold_Backup/SciFi_Masters/Бегущий_по_лезвию_1982_FinalCut_2160p_Master.mkv',
      kind: 'file',
      size: 66_689_972_633, // identical byte-for-byte to Blade_Runner_1982_Final_Cut_UHD.mkv!
      modifiedUnix: 1712900000,
      category: 'Кино UHD / BD-Remux',
      description: 'Кандидат по метаданным для UHD ремукса (имя переведено; содержимое не сверено)',
      tags: 'video,2160p,hevc,renamed',
      fastFingerprint: makeFp(`exact::${66_689_972_633}::7046`),
      videoMeta: {
        durationSec: 7046,
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 5,
        audioSummary: 'ENG Atmos + 4×RUS',
        bitrateMbps: 75.7,
        workKey: 'blade_runner::1982',
      },
      opaqueFlag0x10: false,
    });
  }

  // 3. Remuxes / Container Repacks of the SAME Video Stream (different file size, different audio tracks/container, SAME duration ±1s + resolution + codec!)
  const streamRemuxes: {
    diskId: number;
    folder: string;
    name: string;
    size: number;
    cat: string;
    desc: string;
    videoMeta: VideoMeta;
  }[] = [
    {
      diskId: 23,
      folder: 'Custom_DualAudio_Remuxes/Tarkovsky',
      name: 'Solaris_1972_2160p_DualAudio_RUS+ENG_DTSHD.mkv',
      size: 53_880_112_400, // ~50.18 GB (different size due to added DTS-HD track!)
      cat: 'Классика и Кино СССР',
      desc: 'Тот же видеопоток 2160p HEVC, но пересобран контейнер (+дорожка Criterion и субтитры)',
      videoMeta: {
        durationSec: 9994, // matches 02:46:34!
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 4,
        audioSummary: 'RUS+ENG DTS-HD + 4×Sub',
        bitrateMbps: 43.1,
        workKey: 'solaris::1972',
      },
    },
    {
      diskId: 34,
      folder: 'Author_Dubs_4K/SciFi',
      name: 'Blade.Runner.1982.Final.Cut.2160p.6xRUS.Atmos.mkv',
      size: 68_547_619_200, // ~63.84 GB (extra 6 Russian voiceovers muxed in!)
      cat: 'Кино UHD / BD-Remux',
      desc: 'Тот же видеопоток UHD HDR10, перепакован с 6 авторскими переводами',
      videoMeta: {
        durationSec: 7046, // matches 01:57:26!
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 7,
        audioSummary: '6×RUS + ENG Atmos',
        bitrateMbps: 77.8,
        workKey: 'blade_runner::1982',
      },
    },
    {
      diskId: 29,
      folder: 'BDMV_Raw_Streams/Interstellar_UHD/STREAM',
      name: '00001_Interstellar_2014_MainFeature.m2ts',
      size: 83_665_958_912, // ~77.92 GB M2TS transport stream overhead!
      cat: 'Кино UHD / BD-Remux',
      desc: 'Исходный поток .m2ts с Blu-ray диска (видео идентично ремуксу .mkv на #5)',
      videoMeta: {
        durationSec: 10144, // matches 02:49:04!
        width: 3840,
        height: 2160,
        resolutionTag: '2160p',
        codec: 'HEVC',
        audioTracks: 4,
        audioSummary: 'BDMV PGS + DTS-HD',
        bitrateMbps: 66.0,
        workKey: 'interstellar::2014',
      },
    },
    {
      diskId: 39,
      folder: 'Criterion_RusMux/Stalker',
      name: 'Stalker.1979.1080p.Criterion.Remux.Rus.Subs.mkv',
      size: 32_373_293_056, // ~30.15 GB
      cat: 'Классика и Кино СССР',
      desc: 'Перепаковка реставрации Criterion с внешними комментариями',
      videoMeta: {
        durationSec: 9769, // matches 02:42:49!
        width: 1920,
        height: 1080,
        resolutionTag: '1080p',
        codec: 'AVC',
        audioTracks: 2,
        audioSummary: 'RUS LPCM + Comms',
        bitrateMbps: 26.5,
        workKey: 'stalker::1979',
      },
    },
  ];

  for (const rm of streamRemuxes) {
    const pId = ensureFolderChain(rm.diskId, rm.folder, 1713100000, rm.cat);
    pushEntry({
      volumeId: rm.diskId,
      parentId: pId,
      name: rm.name,
      relativePath: `${rm.folder}/${rm.name}`,
      kind: 'file',
      size: rm.size,
      modifiedUnix: 1713100000,
      category: rm.cat,
      description: rm.desc,
      tags: `video,remux_variant,${rm.videoMeta.resolutionTag}`,
      fastFingerprint: makeFp(`remux::${rm.name}::${rm.size}`),
      videoMeta: rm.videoMeta,
      opaqueFlag0x10: false,
    });
  }

  // 4. Lower-Resolution Rips of the SAME Movies (1080p / 720p redundant encodes sitting on older USB disks!)
  const multiQualityRips: {
    diskId: number;
    folder: string;
    name: string;
    size: number;
    cat: string;
    desc: string;
    videoMeta: VideoMeta;
  }[] = [
    {
      diskId: 18,
      folder: 'Criterion_1080p/Solaris_1972',
      name: 'Solaris_1972_1080p_BDRip_x264.mkv',
      size: Math.round(14.2 * GB),
      cat: 'Классика и Кино СССР',
      desc: 'Старый 1080p рип (на диске #4 уже есть эталонный 4K UHD Remux 48.6 ГБ)',
      videoMeta: {
        durationSec: 9995,
        width: 1920,
        height: 1080,
        resolutionTag: '1080p',
        codec: 'AVC',
        audioTracks: 2,
        audioSummary: 'RUS+ENG AC3',
        bitrateMbps: 11.4,
        workKey: 'solaris::1972',
      },
    },
    {
      diskId: 6,
      folder: 'Road_Laptop_Rips/USSR',
      name: 'Солярис_1972_Mosfilm_4K_Remux.mkv', // Deliberately same name as 4K file too, so both video_rips and same_name catch it!
      size: Math.round(4.12 * GB),
      cat: 'Классика и Кино СССР',
      desc: 'Сжатая копия 720p под тем же именем файла',
      videoMeta: {
        durationSec: 9994,
        width: 1280,
        height: 720,
        resolutionTag: '720p',
        codec: 'AVC',
        audioTracks: 1,
        audioSummary: 'RUS AAC 2.0',
        bitrateMbps: 3.3,
        workKey: 'solaris::1972',
      },
    },
    {
      diskId: 16,
      folder: 'SciFi_1080p_Old/Blade_Runner',
      name: 'Blade_Runner_1982_Final_Cut_UHD.mkv',
      size: Math.round(11.84 * GB),
      cat: 'Кино UHD / BD-Remux',
      desc: 'Рип 1080p (в архиве уже есть 4K UHD на #4, #5, #11, #19)',
      videoMeta: {
        durationSec: 7046,
        width: 1920,
        height: 1080,
        resolutionTag: '1080p',
        codec: 'AVC',
        audioTracks: 2,
        audioSummary: 'RUS+ENG DTS',
        bitrateMbps: 13.4,
        workKey: 'blade_runner::1982',
      },
    },
    {
      diskId: 27,
      folder: 'Old_HDD_2015/SciFi_720p',
      name: 'Бегущий_по_лезвию_1982_BDRip_720p.mkv',
      size: Math.round(4.65 * GB),
      cat: 'Кино UHD / BD-Remux',
      desc: 'Устаревший рип 720p со старого диска',
      videoMeta: {
        durationSec: 7045,
        width: 1280,
        height: 720,
        resolutionTag: '720p',
        codec: 'AVC',
        audioTracks: 1,
        audioSummary: 'RUS AC3 5.1',
        bitrateMbps: 5.3,
        workKey: 'blade_runner::1982',
      },
    },
    {
      diskId: 9,
      folder: 'Action_90s/Terminator_2',
      name: 'Terminator_2_1991_Extended_1080p_BDRip.mkv',
      size: Math.round(13.5 * GB),
      cat: 'Кино UHD / BD-Remux',
      desc: 'Версия 1080p (на #1 и #30 хранится 4K UHD Remux)',
      videoMeta: {
        durationSec: 9194,
        width: 1920,
        height: 1080,
        resolutionTag: '1080p',
        codec: 'AVC',
        audioTracks: 3,
        audioSummary: '3×RUS AC3',
        bitrateMbps: 11.7,
        workKey: 'terminator_2::1991',
      },
    },
    {
      diskId: 41,
      folder: 'Legacy_720p/Action',
      name: 'Терминатор_2_1991_720p.mp4',
      size: Math.round(4.9 * GB),
      cat: 'Кино UHD / BD-Remux',
      desc: 'Старый 720p .mp4 рип',
      videoMeta: {
        durationSec: 9193,
        width: 1280,
        height: 720,
        resolutionTag: '720p',
        codec: 'AVC',
        audioTracks: 1,
        audioSummary: 'RUS AAC',
        bitrateMbps: 4.3,
        workKey: 'terminator_2::1991',
      },
    },
    {
      diskId: 14,
      folder: 'Releases_2024/Dune_2',
      name: 'Дюна_Часть_вторая_2024_WEB-DL_1080p.mkv',
      size: Math.round(16.4 * GB),
      cat: 'Кино UHD / BD-Remux',
      desc: 'Первоначальный WEB-DL 1080p (позже заменён на UHD BDRemux на #5)',
      videoMeta: {
        durationSec: 9949,
        width: 1920,
        height: 1080,
        resolutionTag: '1080p',
        codec: 'HEVC',
        audioTracks: 2,
        audioSummary: 'RUS+ENG EAC3',
        bitrateMbps: 13.2,
        workKey: 'dune_part_two::2024',
      },
    },
  ];

  for (const rip of multiQualityRips) {
    const pId = ensureFolderChain(rip.diskId, rip.folder, 1710500000, rip.cat);
    pushEntry({
      volumeId: rip.diskId,
      parentId: pId,
      name: rip.name,
      relativePath: `${rip.folder}/${rip.name}`,
      kind: 'file',
      size: rip.size,
      modifiedUnix: 1710500000,
      category: rip.cat,
      description: rip.desc,
      tags: `video,${rip.videoMeta.resolutionTag},${rip.videoMeta.codec.toLowerCase()}`,
      fastFingerprint: makeFp(`rip::${rip.name}::${rip.size}`),
      videoMeta: rip.videoMeta,
      opaqueFlag0x10: false,
    });
  }

  // 5. Populate all 60 USB drives with a realistic 99% Video Catalog (~8 feature/episode video files per disk + occasional .srt)
  const videoTemplates: {
    branch: string;
    titlePool: string[];
    ext: string;
    cat: string;
    res: '2160p' | '1080p' | '720p';
    codec: string;
    baseGb: number;
    baseDur: number;
  }[] = [
    {
      branch: 'Movies_UHD_Remux',
      titlePool: [
        'Alien_1979_Theatrical_UHD_Remux',
        'The_Thing_1982_2160p_UHD_Remux',
        '2001_Space_Odyssey_1968_UHD_Remux',
        'Apocalypse_Now_1979_Final_Cut_2160p',
        'Heat_1995_Director_Definitive_2160p',
        'The_Matrix_1999_UHD_Atmos_Remux',
        'Lawrence_of_Arabia_1962_Disc1_2160p',
        'Ran_1985_StudioCanal_4K_Remux',
      ],
      ext: 'mkv',
      cat: 'Кино UHD / BD-Remux',
      res: '2160p',
      codec: 'HEVC',
      baseGb: 54.5,
      baseDur: 8420,
    },
    {
      branch: 'Movies_1080p_BluRay',
      titlePool: [
        'Seven_Samurai_1954_BFI_1080p',
        '8_1-2_Fellini_1963_Criterion_1080p',
        'Le_Samourai_1967_Pathé_1080p',
        'Das_Boot_1981_Directors_Cut_1080p',
        ' Amadeus_1984_Theatrical_1080p',
        'No_Country_for_Old_Men_2007_1080p',
        'There_Will_Be_Blood_2007_1080p',
        'Parasite_2019_BW_Edition_1080p',
      ],
      ext: 'mkv',
      cat: 'Кино UHD / BD-Remux',
      res: '1080p',
      codec: 'AVC',
      baseGb: 24.8,
      baseDur: 7980,
    },
    {
      branch: 'USSR_Restored_Cinema',
      titlePool: [
        'Иди_и_смотри_1985_Мосфильм_4K',
        'Зеркало_1974_Реставрация_2160p',
        'Летят_журавли_1957_Criterion_1080p',
        'Кин_дза_дза_1986_4K_Мосфильм',
        'Бриллиантовая_рука_1968_1080p_Remux',
        'Свой_среди_чужих_1974_1080p',
        'Восхождение_1976_Criterion_1080p',
        'Иваново_детство_1962_1080p',
      ],
      ext: 'mkv',
      cat: 'Классика и Кино СССР',
      res: '2160p',
      codec: 'HEVC',
      baseGb: 36.2,
      baseDur: 6180,
    },
    {
      branch: 'Series_Vault/Season_01',
      titlePool: [
        'Chernobyl_2019_S01E01_2160p_UHD',
        'True_Detective_S01E04_1080p_BluRay',
        'The_Wire_S01E01_1080p_Remux',
        'Severance_S01E09_2160p_HDR',
        'The_Expanse_S03E06_2160p_UHD',
        'Band_of_Brothers_E02_1080p_Remux',
        'Better_Call_Saul_S06E07_1080p',
        'Fargo_S01E01_1080p_BluRay',
      ],
      ext: 'mkv',
      cat: 'Сериалы UHD/1080p',
      res: '2160p',
      codec: 'HEVC',
      baseGb: 14.6,
      baseDur: 3480,
    },
    {
      branch: 'Documentary_4K_BBC',
      titlePool: [
        'Planet_Earth_II_01_Islands_2160p_HLG',
        'Blue_Planet_II_02_The_Deep_2160p',
        'Frozen_Planet_II_01_2160p_UHD',
        'Cosmos_Spacetime_Odyssey_E01_1080p',
        'The_World_at_War_1973_E01_1080p',
        'For_All_Mankind_1989_Criterion_4K',
        'Baraka_1992_8K_Scan_1080p_Remux',
        'Samsara_2011_70mm_Scan_1080p',
      ],
      ext: 'mkv',
      cat: 'Документалистика 4K/HD',
      res: '2160p',
      codec: 'HEVC',
      baseGb: 19.4,
      baseDur: 3320,
    },
    {
      branch: 'Concerts_BluRay',
      titlePool: [
        'Hans_Zimmer_Live_ in_Prague_Atmos',
        'Pink_Floyd_Pulse_Restored_1080p',
        'Queen_Rock_Montreal_1981_2160p_UHD',
        'David_Gilmour_Live_at_Pompeii_1080p',
        'Berliner_Philharmoniker_Beethoven9_UHD',
        'Peter_Gabriel_Secret_World_Live_1080p',
      ],
      ext: 'm2ts',
      cat: 'Концерты и Музыкальное видео',
      res: '1080p',
      codec: 'AVC',
      baseGb: 31.5,
      baseDur: 7860,
    },
  ];

  for (let diskId = 1; diskId <= 60; diskId++) {
    for (let slot = 0; slot < 8; slot++) {
      const tpl = videoTemplates[(diskId * 3 + slot) % videoTemplates.length];
      const baseTitle = tpl.titlePool[(diskId + slot) % tpl.titlePool.length].trim();
      const yearNum = 2018 + ((diskId + slot) % 8);
      const fullFolder = `${tpl.branch}`;
      const modUnix = Math.floor(
        Date.UTC(yearNum, (diskId + slot) % 12, 1 + ((diskId * 3 + slot) % 27)) / 1000
      );
      const parentId = ensureFolderChain(diskId, fullFolder, modUnix, tpl.cat);
      const uniqueSuffix = `D${String(diskId).padStart(2, '0')}_${String(slot + 1).padStart(2, '0')}`;
      const fileName = `${baseTitle}_${uniqueSuffix}.${tpl.ext}`;
      const variance = 0.78 + (((diskId * 11 + slot * 7) % 45) / 100);
      const size = Math.round(tpl.baseGb * variance * GB) + diskId * 65536 + slot * 4096;
      const dur = tpl.baseDur + diskId * 17 + slot * 29;
      const width = tpl.res === '2160p' ? 3840 : tpl.res === '1080p' ? 1920 : 1280;
      const height = tpl.res === '2160p' ? 2160 : tpl.res === '1080p' ? 1080 : 720;
      const bitrateMbps = Number(((size * 8) / dur / 1_000_000).toFixed(1));

      pushEntry({
        volumeId: diskId,
        parentId,
        name: fileName,
        relativePath: `${fullFolder}/${fileName}`,
        kind: 'file',
        size,
        modifiedUnix: modUnix,
        category: tpl.cat,
        description: `USB #${diskId} (${volumes[diskId - 1].label})`,
        tags: `video,${tpl.ext},${tpl.res},${tpl.codec.toLowerCase()},vol${diskId}`,
        fastFingerprint: makeFp(`vol${diskId}::${fileName}::${size}`),
        videoMeta: {
          durationSec: dur,
          width,
          height,
          resolutionTag: tpl.res,
          codec: tpl.codec,
          audioTracks: tpl.res === '2160p' ? 3 : 2,
          audioSummary: tpl.res === '2160p' ? 'RUS+ENG DTS-HD' : 'RUS+ENG AC3',
          bitrateMbps,
        },
        opaqueFlag0x10: false,
      });

      // ~1% companion external subtitle file on every 12th video so archive is genuinely ~99% video bytes!
      if ((diskId + slot) % 12 === 0) {
        const subName = `${baseTitle}_${uniqueSuffix}.rus.srt`;
        pushEntry({
          volumeId: diskId,
          parentId,
          name: subName,
          relativePath: `${fullFolder}/${subName}`,
          kind: 'file',
          size: 98_420 + diskId * 120,
          modifiedUnix: modUnix,
          category: 'Дорожки и Субтитры',
          description: 'Внешние русские субтитры UTF-8',
          tags: `srt,subtitles,vol${diskId}`,
          opaqueFlag0x10: false,
        });
      }
    }
  }

  return { volumes, entries, ensureFolderChain, pushEntry };
}

// --- Boolean & Video-Aware Field Query Engine ---

type QueryToken =
  | { kind: 'L' }
  | { kind: 'R' }
  | { kind: 'AND' }
  | { kind: 'OR' }
  | { kind: 'NOT' }
  | { kind: 'WORD'; value: string };

function lexQuery(raw: string): QueryToken[] {
  const out: QueryToken[] = [];
  let i = 0;
  while (i < raw.length) {
    const ch = raw[i];
    if (/\s/.test(ch)) {
      i++;
      continue;
    }
    if (ch === '(') {
      out.push({ kind: 'L' });
      i++;
      continue;
    }
    if (ch === ')') {
      out.push({ kind: 'R' });
      i++;
      continue;
    }
    let word = '';
    let quoted = false;
    let hadQuote = false;
    while (i < raw.length) {
      const c = raw[i];
      if (!quoted && (/\s/.test(c) || c === '(' || c === ')')) break;
      i++;
      if (c === '"') {
        quoted = !quoted;
        hadQuote = true;
      } else if (c === '\\' && quoted) {
        if (i >= raw.length) throw new Error('Незавершённое экранирование в запросе');
        word += raw[i++];
      } else {
        word += c;
      }
    }
    if (quoted) throw new Error('Незакрытая кавычка в поисковом запросе');
    if (!word) continue;
    if (!hadQuote) {
      const upper = word.toUpperCase();
      if (upper === 'AND') { out.push({ kind: 'AND' }); continue; }
      if (upper === 'OR') { out.push({ kind: 'OR' }); continue; }
      if (upper === 'NOT') { out.push({ kind: 'NOT' }); continue; }
    }
    out.push({ kind: 'WORD', value: word });
  }
  return out;
}

function parseByteLiteral(spec: string): { op: '=' | '>' | '>=' | '<' | '<='; bytes: number } | null {
  const m = spec.trim().match(/^(>=|<=|>|<|=)?\s*(\d+(?:\.\d+)?)\s*(b|kb|mb|gb|tb)?$/i);
  if (!m) return null;
  const op = (m[1] as '=' | '>' | '>=' | '<' | '<=' | undefined) ?? '=';
  const num = Number(m[2]);
  if (!Number.isFinite(num)) return null;
  const unit = (m[3] ?? 'b').toLowerCase();
  const mult =
    unit === 'tb' ? TB :
    unit === 'gb' ? GB :
    unit === 'mb' ? MB :
    unit === 'kb' ? 1024 : 1;
  return { op, bytes: Math.round(num * mult) };
}

function compilePredicate(raw: string): (entry: RawEntry) => boolean {
  const trimmed = raw.trim();
  if (!trimmed) return () => true;
  const tokens = lexQuery(trimmed);
  if (tokens.length === 0) return () => true;

  let pos = 0;

  const parseExpr = (): ((e: RawEntry) => boolean) => {
    let left = parseAnd();
    while (tokens[pos]?.kind === 'OR') {
      pos++;
      const right = parseAnd();
      const prev = left;
      left = (e) => prev(e) || right(e);
    }
    return left;
  };

  const parseAnd = (): ((e: RawEntry) => boolean) => {
    let left = parseUnary();
    while (pos < tokens.length) {
      const next = tokens[pos];
      const explicit = next.kind === 'AND';
      const implicit = next.kind === 'WORD' || next.kind === 'L' || next.kind === 'NOT';
      if (!explicit && !implicit) break;
      if (explicit) pos++;
      const right = parseUnary();
      const prev = left;
      left = (e) => prev(e) && right(e);
    }
    return left;
  };

  const parseUnary = (): ((e: RawEntry) => boolean) => {
    if (tokens[pos]?.kind === 'NOT') {
      pos++;
      const inner = parseUnary();
      return (e) => !inner(e);
    }
    if (tokens[pos]?.kind === 'L') {
      pos++;
      const inner = parseExpr();
      if (tokens[pos]?.kind !== 'R') throw new Error('Отсутствует закрывающая скобка ")"');
      pos++;
      return inner;
    }
    const tok = tokens[pos];
    if (!tok || tok.kind !== 'WORD') throw new Error('Ожидался поисковый термин');
    pos++;
    return buildTermMatcher(tok.value);
  };

  const buildTermMatcher = (word: string): ((e: RawEntry) => boolean) => {
    const colonIdx = word.indexOf(':');
    const field = colonIdx > 0 ? word.slice(0, colonIdx).toLowerCase() : 'text';
    const val = colonIdx > 0 ? word.slice(colonIdx + 1) : word;
    if (!val) throw new Error(`Пустое значение после "${field}:"`);
    const vLower = val.toLowerCase();

    switch (field) {
      case 'name':
        return (e) => e.nameNorm.includes(vLower);
      case 'path':
        return (e) => e.relativePath.toLowerCase().includes(vLower);
      case 'ext': {
        const cleanExt = vLower.replace(/^\./, '');
        return (e) => e.kind === 'file' && e.ext === cleanExt;
      }
      case 'res':
      case 'resolution': {
        if (vLower === '4k' || vLower === 'uhd' || vLower === '2160p') {
          return (e) => e.videoMeta?.resolutionTag === '2160p';
        }
        if (vLower === '1080p' || vLower === 'fhd' || vLower === 'fullhd') {
          return (e) => e.videoMeta?.resolutionTag === '1080p';
        }
        if (vLower === '720p' || vLower === 'hd') {
          return (e) => e.videoMeta?.resolutionTag === '720p';
        }
        if (vLower === 'sd' || vLower === 'dvd') {
          return (e) => e.videoMeta?.resolutionTag === 'SD';
        }
        return (e) => (e.videoMeta?.resolutionTag.toLowerCase() ?? '') === vLower;
      }
      case 'codec': {
        const normCodec =
          vLower === 'h265' || vLower === 'x265' ? 'hevc' :
          vLower === 'h264' || vLower === 'x264' ? 'avc' : vLower;
        return (e) => (e.videoMeta?.codec.toLowerCase() ?? '').includes(normCodec);
      }
      case 'dur':
      case 'duration': {
        const m = vLower.match(/^(>=|<=|>|<|=)?\s*(\d+(?:\.\d+)?)\s*(s|sec|m|min|h)?$/);
        if (!m) throw new Error(`Некорректная длительность: dur:${val} (пример: dur:>120m)`);
        const op = m[1] ?? '=';
        const n = Number(m[2]);
        const unit = m[3] ?? 'm';
        const targetSec = unit === 'h' ? n * 3600 : unit === 's' || unit === 'sec' ? n : n * 60;
        return (e) => {
          const d = e.videoMeta?.durationSec;
          if (!d) return false;
          if (op === '=') return Math.abs(d - targetSec) <= 60;
          if (op === '>') return d > targetSec;
          if (op === '>=') return d >= targetSec;
          if (op === '<') return d < targetSec;
          return d <= targetSec;
        };
      }
      case 'type':
        if (vLower === 'video') return (e) => e.kind === 'file' && VIDEO_EXTENSIONS.has(e.ext);
        if (vLower === 'sub' || vLower === 'subs') return (e) => e.kind === 'file' && SUB_EXTENSIONS.has(e.ext);
        if (vLower === 'audio') return (e) => e.kind === 'file' && AUDIO_EXTENSIONS.has(e.ext);
        return (e) => e.category.toLowerCase().includes(vLower) || e.tags.toLowerCase().includes(vLower);
      case 'kind':
        if (vLower === 'file' || vLower === 'files') return (e) => e.kind === 'file';
        if (vLower === 'dir' || vLower === 'directory' || vLower === 'folder') return (e) => e.kind === 'directory';
        return () => false;
      case 'disk':
      case 'vol': {
        const idNum = Number(vLower.replace(/^#/, ''));
        return Number.isInteger(idNum) && idNum > 0 ? (e) => e.volumeId === idNum : () => false;
      }
      case 'size': {
        const parsed = parseByteLiteral(vLower);
        if (!parsed) throw new Error(`Некорректное условие размера: size:${val}`);
        return (e) => {
          if (e.kind !== 'file') return false;
          if (parsed.op === '=') return e.size === parsed.bytes;
          if (parsed.op === '>') return e.size > parsed.bytes;
          if (parsed.op === '>=') return e.size >= parsed.bytes;
          if (parsed.op === '<') return e.size < parsed.bytes;
          return e.size <= parsed.bytes;
        };
      }
      case 'year': {
        const m = vLower.match(/^(>=|<=|>|<|=)?(\d{4})$/);
        if (!m) throw new Error(`Некорректный год: year:${val}`);
        const op = m[1] ?? '=';
        const targetYear = Number(m[2]);
        return (e) => {
          const y = new Date(e.modifiedUnix * 1000).getUTCFullYear();
          if (op === '=') return y === targetYear;
          if (op === '>') return y > targetYear;
          if (op === '>=') return y >= targetYear;
          if (op === '<') return y < targetYear;
          return y <= targetYear;
        };
      }
      case 'dup':
      case 'copies': {
        const m = vLower.match(/^(>=|<=|>|<|=)?(\d+)$/);
        if (!m) throw new Error(`Некорректное условие копий: dup:${val}`);
        const op = m[1] ?? '=';
        const target = Number(m[2]);
        return (e) => {
          if (e.kind !== 'file') return false;
          const cnt = metadataCandidateVolumeCounts.get(metadataCandidateKey(e.name, e.size)) ?? 1;
          if (op === '=') return cnt === target;
          if (op === '>') return cnt > target;
          if (op === '>=') return cnt >= target;
          if (op === '<') return cnt < target;
          return cnt <= target;
        };
      }
      default:
        return (e) =>
          e.nameNorm.includes(vLower) ||
          e.relativePath.toLowerCase().includes(vLower) ||
          e.description.toLowerCase().includes(vLower) ||
          e.tags.toLowerCase().includes(vLower) ||
          (e.videoMeta?.codec.toLowerCase() ?? '').includes(vLower) ||
          (e.videoMeta?.resolutionTag.toLowerCase() ?? '') === vLower;
    }
  };

  const fn = parseExpr();
  if (pos < tokens.length) throw new Error('Лишняя скобка или оператор в запросе');
  return fn;
}

// --- Catalog State & Indexes ---

const { volumes: rawVolumes, entries: rawEntries, ensureFolderChain, pushEntry } = buildInitialCatalog();

// Name-and-size metadata candidate count only; never evidence of identical content or backup.
const metadataCandidateCounts = new Map<string, number>();
const metadataCandidateVolumeCounts = new Map<string, number>();
function rebuildDuplicateClassCounts() {
  metadataCandidateCounts.clear();
  metadataCandidateVolumeCounts.clear();
  for (const e of rawEntries) {
    if (e.kind === 'file' && e.size >= MB) {
      const key = metadataCandidateKey(e.name, e.size);
      metadataCandidateCounts.set(key, (metadataCandidateCounts.get(key) ?? 0) + 1);
    }
  }
  for (const [key, count] of countCandidateVolumes(rawEntries.filter((e) => e.kind === 'file' && e.size >= MB))) {
    metadataCandidateVolumeCounts.set(key, count);
  }
}
rebuildDuplicateClassCounts();

let canonicalSha = crypto
  .createHash('sha256')
  .update(JSON.stringify({ v: 'WhereIsIt-CTF-3.00', count: rawEntries.length }))
  .digest('hex');

let platformPreset: PlatformPreset = 'windows';
const sourceMode: 'linked_ctf' | 'sqlite_local' = 'sqlite_local';
const linkedCtfPath = 'D:\\Catalogs\\Master_Video_60USB.db';
let pinnedGenerationSha = canonicalSha;
let liveSourceSha = canonicalSha;
const probeOk = true;
let simulatedIoDelayMs = 0;
let cancelledIoCount = 0;

const shelfTags = new Map<number, string>();
const physicalBindings: Record<PlatformPreset, Map<number, string>> = {
  windows: new Map<number, string>(),
  macos: new Map<number, string>(),
};

const osMountsByPlatform: Record<PlatformPreset, LiveOsMount[]> = {
  windows: [
    {
      mountPoint: 'E:\\',
      physicalId: 'win-vol-uuid-usb-demo-01',
      serial: 'WD-WX92-5TB',
      label: 'WD_Elements_5TB_Incoming',
      filesystem: 'NTFS',
      capacityBytes: 5 * TB,
      usedBytes: Math.round(3.42 * TB),
      isSystemReserved: false,
    },
  ],
  macos: [
    {
      mountPoint: '/Volumes/WD_Elements_5TB_Incoming',
      physicalId: 'mac-fs-uuid-usb-demo-01',
      serial: 'WD-WX92-5TB',
      label: 'WD_Elements_5TB_Incoming',
      filesystem: 'ExFAT',
      capacityBytes: 5 * TB,
      usedBytes: Math.round(3.42 * TB),
      isSystemReserved: false,
    },
  ],
};

const annotations: AnnotationRecord[] = [];
const globalSearchLatenciesMs: number[] = [8, 11, 9, 12, 10, 14, 9, 11, 13, 10];

function recordGlobalLatency(ms: number) {
  globalSearchLatenciesMs.push(Math.max(1, Math.round(ms)));
  if (globalSearchLatenciesMs.length > 128) globalSearchLatenciesMs.shift();
}

function computeLatencySummary() {
  const count = globalSearchLatenciesMs.length;
  const sorted = [...globalSearchLatenciesMs].sort((a, b) => a - b);
  const nearestRank = (p: number) =>
    sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil((p / 100) * sorted.length) - 1))] ?? 10;
  return {
    sampleCount: count,
    minSampleThreshold: 10,
    p50Ms: nearestRank(50),
    p95Ms: nearestRank(95),
    lastMs: globalSearchLatenciesMs[count - 1] ?? 10,
    sloTargetMs: 150,
    recentSamplesMs: globalSearchLatenciesMs.slice(-24),
  };
}

function buildVolumesList() {
  const entryCounts = new Map<number, number>();
  const videoCounts = new Map<number, number>();
  const indexedBytesByVol = new Map<number, number>();

  for (const e of rawEntries) {
    entryCounts.set(e.volumeId, (entryCounts.get(e.volumeId) ?? 0) + 1);
    if (e.kind === 'file') {
      indexedBytesByVol.set(e.volumeId, (indexedBytesByVol.get(e.volumeId) ?? 0) + e.size);
      if (VIDEO_EXTENSIONS.has(e.ext)) {
        videoCounts.set(e.volumeId, (videoCounts.get(e.volumeId) ?? 0) + 1);
      }
    }
  }

  return rawVolumes.map((v) => {
    const indexedBytes = indexedBytesByVol.get(v.id) ?? 0;
    return {
      ...v,
      indexedBytes,
      usedBytes: Math.max(v.usedBytes, indexedBytes),
      entryCount: entryCounts.get(v.id) ?? 0,
      videoFileCount: videoCounts.get(v.id) ?? 0,
      shelfTag: shelfTags.get(v.id) ?? '',
      shelfConflictWith: [] as number[],
      physicalStatus: 'offline' as PhysicalStatus,
      pairedPhysicalId: null,
      mountedPath: null,
    };
  });
}

const activeSearchJobs = new Map<string, { aborted: boolean }>();
let activeScanJob: ScanJobInternal | null = null;
const recentScanJobs: ScanJobInternal[] = [];

function serializeScanJob(j: ScanJobInternal) {
  return {
    jobId: j.jobId,
    targetLabel: j.targetLabel,
    mountPoint: j.mountPoint,
    physicalId: j.physicalId,
    shelfTag: j.shelfTag,
    status: j.status,
    scannedFiles: j.scannedFiles,
    totalExpectedFiles: j.totalExpectedFiles,
    scannedBytes: j.scannedBytes,
    totalExpectedBytes: j.totalExpectedBytes,
    currentPath: j.currentPath,
    filesPerSec: j.filesPerSec,
    bytesPerSec: j.bytesPerSec,
    duplicatesDetected: j.duplicatesDetected,
    createdVolumeId: j.createdVolumeId,
    startedAtMs: j.startedAtMs,
    elapsedMs: j.elapsedMs,
    pluginsUsed: j.pluginsUsed,
    topFolders: j.topFolders,
    error: j.error,
  };
}

function buildSystemSnapshot() {
  const volumes = buildVolumesList();
  const categories = Array.from(new Set(rawEntries.map((e) => e.category))).sort();
  const totalIndexedBytes = volumes.reduce((acc, v) => acc + v.indexedBytes, 0);
  const totalCapacityBytes = volumes.reduce((acc, v) => acc + v.capacityBytes, 0);
  const totalUsedBytes = volumes.reduce((acc, v) => acc + v.usedBytes, 0);

  return {
    platform: platformPreset,
    sourceMode,
    linkedCtfPath,
    pinnedGenerationSha,
    liveSourceSha,
    tampered: false,
    probeOk,
    simulatedIoDelayMs,
    totalCatalogEntries: rawEntries.length,
    totalIndexedBytes,
    totalCapacityBytes,
    totalUsedBytes,
    volumes,
    osMounts: osMountsByPlatform[platformPreset],
    categories,
    latencySummary: computeLatencySummary(),
    annotations,
    activeScanJob: activeScanJob ? serializeScanJob(activeScanJob) : null,
    recentScanJobs: recentScanJobs.slice(0, 6).map(serializeScanJob),
    cancelledIoCount,
  };
}

// --- DaisyDisk Hierarchical Sunburst Tree Builder ---

function buildVolumeFolderTree(
  volumeId: number,
  rootLabel: string,
  baseFolderPath: string | null,
  matchPred: ((e: RawEntry) => boolean) | null,
  baseColorIndex: number,
  maxDepth: number
): SunburstNode {
  const prefix = baseFolderPath ? `${baseFolderPath}/` : '';
  const files = rawEntries.filter((e) => {
    if (e.volumeId !== volumeId || e.kind !== 'file') return false;
    if (!baseFolderPath) return true;
    return e.relativePath.startsWith(prefix);
  });
  const folderIds = new Map(
    rawEntries.filter((e) => e.volumeId === volumeId && e.kind === 'directory').map((entry) => [entry.relativePath, entry.id])
  );

  interface MutableDir {
    name: string;
    fullPath: string;
    folderEntryId: number | null;
    size: number;
    matchedBytes: number;
    itemCount: number;
    subdirs: Map<string, MutableDir>;
    files: RawEntry[];
  }

  const rootDir: MutableDir = {
    name: rootLabel,
    fullPath: baseFolderPath ?? '',
    folderEntryId: baseFolderPath ? folderIds.get(baseFolderPath) ?? null : null,
    size: 0,
    matchedBytes: 0,
    itemCount: 0,
    subdirs: new Map(),
    files: [],
  };

  for (const f of files) {
    const rel = baseFolderPath ? f.relativePath.slice(prefix.length) : f.relativePath;
    const parts = rel.split('/').filter(Boolean);
    const isMatched = matchPred ? matchPred(f) : false;

    rootDir.size += f.size;
    rootDir.itemCount += 1;
    if (isMatched) rootDir.matchedBytes += f.size;

    let curr = rootDir;
    let builtPath = baseFolderPath ?? '';
    for (let i = 0; i < parts.length - 1; i++) {
      const seg = parts[i];
      builtPath = builtPath ? `${builtPath}/${seg}` : seg;
      let sub = curr.subdirs.get(seg);
      if (!sub) {
        sub = {
          name: seg,
          fullPath: builtPath,
          folderEntryId: folderIds.get(builtPath) ?? null,
          size: 0,
          matchedBytes: 0,
          itemCount: 0,
          subdirs: new Map(),
          files: [],
        };
        curr.subdirs.set(seg, sub);
      }
      sub.size += f.size;
      sub.itemCount += 1;
      if (isMatched) sub.matchedBytes += f.size;
      curr = sub;
    }
    curr.files.push(f);
  }

  const convertNode = (dir: MutableDir, depth: number, inheritedColor: number): SunburstNode => {
    const children: SunburstNode[] = [];
    if (depth < maxDepth) {
      let cIdx = 0;
      for (const sub of dir.subdirs.values()) {
        const color = depth === 0 ? cIdx++ : inheritedColor;
        children.push(convertNode(sub, depth + 1, color));
      }
      for (const file of dir.files) {
        const isMatched = matchPred ? matchPred(file) : false;
        const color = depth === 0 ? cIdx++ : inheritedColor;
        children.push({
          id: `file-${file.id}`,
          name: file.name,
          size: file.size,
          matchedBytes: isMatched ? file.size : 0,
          itemCount: 1,
          kind: 'file',
          volumeId,
          folderPath: dir.fullPath || null,
          entryId: file.id,
          folderEntryId: dir.folderEntryId,
          duplicateCount: file.size >= MB ? (metadataCandidateVolumeCounts.get(metadataCandidateKey(file.name, file.size)) ?? 1) : 1,
          colorIndex: color % 8,
        });
      }
      children.sort((a, b) => b.size - a.size);
    }

    return {
      id: dir.fullPath ? `dir-${volumeId}-${dir.fullPath}` : `vol-root-${volumeId}`,
      name: dir.name,
      size: dir.size,
      matchedBytes: dir.matchedBytes,
      itemCount: dir.itemCount,
      kind: depth === 0 ? 'root' : 'directory',
      volumeId,
      folderPath: dir.fullPath || null,
      entryId: dir.folderEntryId,
      colorIndex: inheritedColor % 8,
      children: children.length > 0 ? children : undefined,
    };
  };

  return convertNode(rootDir, 0, baseColorIndex);
}

// --- Background Test Scan Stream Runner ---
async function executeScanSimulation(
  job: ScanJobInternal,
  existingVolumeId: number | null,
  plugins: string[],
  customNote: string,
  customCode: string
) {
  const sampleFolders = [
    'Movies_UHD_Remux/2026_Masters',
    'Movies_UHD_Remux/Classic_Restorations',
    'Series_Vault/Season_01_2160p',
    'Documentary_4K/IMAX_Space',
  ];

  const steps = 18;
  try {
    for (let step = 1; step <= steps; step++) {
      if (job.aborted) {
        job.status = 'cancelled';
        job.elapsedMs = Date.now() - job.startedAtMs;
        cancelledIoCount++;
        return;
      }
      await new Promise((r) => setTimeout(r, 120));
      if (job.aborted) {
        job.status = 'cancelled';
        job.elapsedMs = Date.now() - job.startedAtMs;
        cancelledIoCount++;
        return;
      }

      const progress = step / steps;
      job.scannedFiles = Math.round(job.totalExpectedFiles * progress);
      job.scannedBytes = Math.round(job.totalExpectedBytes * progress);
      job.elapsedMs = Math.max(1, Date.now() - job.startedAtMs);
      job.filesPerSec = Math.round((job.scannedFiles / job.elapsedMs) * 1000);
      job.bytesPerSec = Math.round((job.scannedBytes / job.elapsedMs) * 1000);
      const folderSample = sampleFolders[step % sampleFolders.length];
      job.currentPath = `${job.mountPoint}${folderSample}/Film_Master_${String(step).padStart(2, '0')}_2160p.mkv`;
    }

    let targetVolId = existingVolumeId;
    if (!targetVolId) {
      targetVolId = rawVolumes.length + 1;
      rawVolumes.push({
        id: targetVolId,
        label: job.targetLabel,
        diskCode: customCode || `USB-${String(targetVolId).padStart(2, '0')}`,
        hardwareModel: 'WD Elements 5TB',
        serialNumber: `SN-SCAN-${targetVolId}`,
        note: customNote || 'Добавлен через сканер USB-носителей',
        recordedRoot: job.mountPoint,
        filesystem: job.filesystem,
        capacityBytes: job.capacityBytes,
        usedBytes: job.totalExpectedBytes,
        pluginsUsed: plugins,
        cataloguedAtUnix: Math.floor(Date.now() / 1000),
        sourceKind: 'scanned_usb',
      });
    } else {
      const vol = rawVolumes.find((v) => v.id === targetVolId);
      if (vol) {
        vol.cataloguedAtUnix = Math.floor(Date.now() / 1000);
        vol.pluginsUsed = plugins;
        if (customNote) vol.note = customNote;
        if (customCode) vol.diskCode = customCode;
      }
    }

    if (!existingVolumeId) {
      const nowUnix = Math.floor(Date.now() / 1000);
      const hasVideoPlugin = plugins.includes('video_header');
      const hasSparse = plugins.includes('sparse_3x64kb');

      const generatedFiles: {
        folder: string;
        name: string;
        size: number;
        cat: string;
        vm: VideoMeta;
      }[] = [
        {
          folder: 'Movies_UHD_Remux/Classic_Restorations',
          name: 'Солярис_1972_Mosfilm_4K_Remux.mkv',
          size: 52_237_543_014,
          cat: 'Классика и Кино СССР',
          vm: {
            durationSec: 9994,
            width: 3840,
            height: 2160,
            resolutionTag: '2160p',
            codec: 'HEVC',
            audioTracks: 2,
            audioSummary: 'RUS FLAC 2.0',
            bitrateMbps: 41.8,
            workKey: 'solaris::1972',
          },
        },
        {
          folder: 'Movies_UHD_Remux/2026_Masters',
          name: 'Oppenheimer_2023_IMAX_2160p_Remux.mkv',
          size: Math.round(78.4 * GB),
          cat: 'Кино UHD / BD-Remux',
          vm: {
            durationSec: 10809,
            width: 3840,
            height: 2160,
            resolutionTag: '2160p',
            codec: 'HEVC',
            audioTracks: 3,
            audioSummary: 'RUS+ENG DTS-HD',
            bitrateMbps: 58.0,
            workKey: 'oppenheimer::2023',
          },
        },
        {
          folder: 'Series_Vault/Season_01_2160p',
          name: 'Shogun_2024_S01E01_2160p_DV.mkv',
          size: Math.round(15.2 * GB),
          cat: 'Сериалы UHD/1080p',
          vm: {
            durationSec: 4190,
            width: 3840,
            height: 2160,
            resolutionTag: '2160p',
            codec: 'HEVC',
            audioTracks: 3,
            audioSummary: 'RUS+JPN+ENG',
            bitrateMbps: 29.0,
          },
        },
      ];

      let dups = 0;
      for (const gf of generatedFiles) {
        if ((metadataCandidateCounts.get(metadataCandidateKey(gf.name, gf.size)) ?? 0) > 0) dups++;
        const pId = ensureFolderChain(targetVolId, gf.folder, nowUnix, gf.cat);
        pushEntry({
          volumeId: targetVolId,
          parentId: pId,
          name: gf.name,
          relativePath: `${gf.folder}/${gf.name}`,
          kind: 'file',
          size: gf.size,
          modifiedUnix: nowUnix,
          category: gf.cat,
          description: `Отсканировано с носителя ${job.targetLabel}`,
          tags: `scanned,video,vol${targetVolId}`,
          fastFingerprint: hasSparse
            ? crypto.createHash('sha256').update(`exact::${gf.size}::${gf.vm.durationSec}`).digest('hex').slice(0, 12)
            : null,
          videoMeta: hasVideoPlugin ? gf.vm : null,
          opaqueFlag0x10: false,
        });
      }
      job.duplicatesDetected = dups;
      rebuildDuplicateClassCounts();
    }

    job.createdVolumeId = targetVolId;
    job.status = 'completed';
    job.elapsedMs = Math.max(1, Date.now() - job.startedAtMs);
  } catch (e) {
    job.status = 'failed';
    job.error = (e as Error).message;
  } finally {
    recentScanJobs.unshift({ ...job });
    if (activeScanJob?.jobId === job.jobId) {
      activeScanJob = null;
    }
  }
}

async function startServer() {
  const app = express();
  app.use(express.json({ limit: '25mb' }));

  // 1. System Snapshot
  app.get('/api/state', (_req: Request, res: Response) => {
    res.json(buildSystemSnapshot());
  });

  // 2. Edit Disk Properties & Fast Rename (WhereIsIt / egui Disk Properties)
  app.post('/api/volumes/update', (req: Request, res: Response) => {
    const {
      volumeId,
      label,
      diskCode,
      hardwareModel,
      capacityBytes,
      filesystem,
      note,
    } = (req.body ?? {}) as {
      volumeId?: number;
      label?: string;
      diskCode?: string;
      hardwareModel?: string;
      capacityBytes?: number;
      filesystem?: string;
      note?: string;
    };

    const id = Number(volumeId);
    const vol = rawVolumes.find((v) => v.id === id);
    if (!vol) {
      res.status(404).json({ error: `Диск #${id} не найден в каталоге.` });
      return;
    }

    if (typeof label === 'string' && label.trim().length > 0) {
      vol.label = label.trim().slice(0, 80);
    }
    if (typeof diskCode === 'string') {
      vol.diskCode = diskCode.trim().slice(0, 24) || `USB-${String(id).padStart(2, '0')}`;
    }
    if (typeof hardwareModel === 'string') {
      vol.hardwareModel = hardwareModel.trim().slice(0, 64);
    }
    if (typeof filesystem === 'string' && filesystem.trim()) {
      vol.filesystem = filesystem.trim().slice(0, 24);
    }
    if (typeof note === 'string') {
      vol.note = note.trim().slice(0, 400);
    }
    if (typeof capacityBytes === 'number' && Number.isFinite(capacityBytes) && capacityBytes > 0) {
      vol.capacityBytes = Math.max(vol.usedBytes, Math.round(capacityBytes));
    }

    res.json({
      ok: true,
      volumeId: vol.id,
      snapshot: buildSystemSnapshot(),
    });
  });

  // 3. Cancel Active Search Query
  app.post('/api/catalog/cancel', (req: Request, res: Response) => {
    const { requestId } = (req.body ?? {}) as { requestId?: string };
    let cancelled = 0;
    if (requestId && activeSearchJobs.has(requestId)) {
      activeSearchJobs.get(requestId)!.aborted = true;
      cancelled = 1;
    } else {
      for (const j of activeSearchJobs.values()) {
        if (!j.aborted) {
          j.aborted = true;
          cancelled++;
        }
      }
    }
    cancelledIoCount += cancelled;
    res.json({ ok: true, cancelled, cancelledIoCount });
  });

  // 4. DaisyDisk Hierarchical Sunburst Endpoint
  app.post('/api/catalog/sunburst', (req: Request, res: Response) => {
    const started = performance.now();
    try {
      const {
        volumeId = null,
        folderPath = null,
        groupBy = 'volumes',
        highlightQuery = '',
      } = (req.body ?? {}) as {
        volumeId?: number | null;
        folderPath?: string | null;
        groupBy?: 'volumes' | 'categories' | 'extensions';
        highlightQuery?: string;
      };

      const matchPred = highlightQuery.trim() ? compilePredicate(highlightQuery) : null;
      const vols = buildVolumesList();
      const volMap = new Map(vols.map((v) => [v.id, v]));

      const breadcrumbs: { label: string; volumeId: number | null; folderPath: string | null }[] = [
        { label: `Весь архив (${vols.length} дисков)`, volumeId: null, folderPath: null },
      ];

      let root: SunburstNode;
      let totalCapacityBytes = vols.reduce((acc, v) => acc + v.capacityBytes, 0);
      let totalUsedBytes = vols.reduce((acc, v) => acc + v.usedBytes, 0);
      let totalIndexedBytes = vols.reduce((acc, v) => acc + v.indexedBytes, 0);

      if (volumeId !== null && volumeId !== undefined) {
        const vId = Number(volumeId);
        const vol = volMap.get(vId);
        if (!vol) {
          res.status(404).json({ error: `Диск #${vId} не найден.` });
          return;
        }
        totalCapacityBytes = vol.capacityBytes;
        totalUsedBytes = vol.usedBytes;
        totalIndexedBytes = vol.indexedBytes;

        breadcrumbs.push({
          label: `#${vol.id} · ${vol.label}`,
          volumeId: vol.id,
          folderPath: null,
        });

        const cleanFolder = folderPath ? folderPath.replace(/^\/+|\/+$/g, '') : null;
        if (cleanFolder) {
          const segs = cleanFolder.split('/');
          let acc = '';
          for (const seg of segs) {
            acc = acc ? `${acc}/${seg}` : seg;
            breadcrumbs.push({ label: seg, volumeId: vol.id, folderPath: acc });
          }
        }

        const rootTitle = cleanFolder ? cleanFolder.split('/').pop()! : `#${vol.id} · ${vol.label}`;
        root = buildVolumeFolderTree(vol.id, rootTitle, cleanFolder, matchPred, (vol.id - 1) % 8, 4);
      } else if (groupBy === 'extensions') {
        const extMap = new Map<string, Map<number, { size: number; matched: number; count: number }>>();
        let grandSize = 0;
        let grandMatched = 0;
        let grandCount = 0;

        for (const e of rawEntries) {
          if (e.kind !== 'file') continue;
          const m = matchPred ? matchPred(e) : false;
          grandSize += e.size;
          grandCount += 1;
          if (m) grandMatched += e.size;

          const extKey = e.ext ? `.${e.ext}` : '(без расш.)';
          let byVol = extMap.get(extKey);
          if (!byVol) {
            byVol = new Map();
            extMap.set(extKey, byVol);
          }
          const slot = byVol.get(e.volumeId) ?? { size: 0, matched: 0, count: 0 };
          slot.size += e.size;
          slot.count += 1;
          if (m) slot.matched += e.size;
          byVol.set(e.volumeId, slot);
        }

        let cIdx = 0;
        const extChildren: SunburstNode[] = [];
        for (const [extLabel, byVol] of extMap.entries()) {
          const color = cIdx++ % 8;
          let extSize = 0;
          let extMatched = 0;
          let extCount = 0;
          const volChildren: SunburstNode[] = [];
          for (const [vid, stats] of byVol.entries()) {
            const vObj = volMap.get(vid);
            extSize += stats.size;
            extMatched += stats.matched;
            extCount += stats.count;
            volChildren.push({
              id: `ext-${extLabel}-vol-${vid}`,
              name: `#${vid} · ${vObj?.label ?? ''}`,
              size: stats.size,
              matchedBytes: stats.matched,
              itemCount: stats.count,
              kind: 'volume',
              volumeId: vid,
              folderPath: null,
              entryId: null,
              colorIndex: color,
            });
          }
          volChildren.sort((a, b) => b.size - a.size);
          extChildren.push({
            id: `ext-${extLabel}`,
            name: extLabel,
            size: extSize,
            matchedBytes: extMatched,
            itemCount: extCount,
            kind: 'category',
            volumeId: null,
            folderPath: null,
            entryId: null,
            searchQuery: extLabel.startsWith('.') ? `ext:${extLabel.slice(1)}` : extLabel,
            colorIndex: color,
            children: volChildren,
          });
        }
        extChildren.sort((a, b) => b.size - a.size);

        root = {
          id: 'archive-extensions-root',
          name: 'Весь архив по контейнерам',
          size: grandSize,
          matchedBytes: grandMatched,
          itemCount: grandCount,
          kind: 'root',
          volumeId: null,
          folderPath: null,
          entryId: null,
          colorIndex: 0,
          children: extChildren,
        };
      } else if (groupBy === 'categories') {
        const catMap = new Map<string, Map<number, { size: number; matched: number; count: number }>>();
        let grandSize = 0;
        let grandMatched = 0;
        let grandCount = 0;

        for (const e of rawEntries) {
          if (e.kind !== 'file') continue;
          const m = matchPred ? matchPred(e) : false;
          grandSize += e.size;
          grandCount += 1;
          if (m) grandMatched += e.size;

          let byVol = catMap.get(e.category);
          if (!byVol) {
            byVol = new Map();
            catMap.set(e.category, byVol);
          }
          const slot = byVol.get(e.volumeId) ?? { size: 0, matched: 0, count: 0 };
          slot.size += e.size;
          slot.count += 1;
          if (m) slot.matched += e.size;
          byVol.set(e.volumeId, slot);
        }

        let cIdx = 0;
        const catChildren: SunburstNode[] = [];
        for (const [catName, byVol] of catMap.entries()) {
          const color = cIdx++ % 8;
          let catSize = 0;
          let catMatched = 0;
          let catCount = 0;
          const volChildren: SunburstNode[] = [];
          for (const [vid, stats] of byVol.entries()) {
            const vObj = volMap.get(vid);
            catSize += stats.size;
            catMatched += stats.matched;
            catCount += stats.count;
            volChildren.push({
              id: `cat-${catName}-vol-${vid}`,
              name: `#${vid} · ${vObj?.label ?? ''}`,
              size: stats.size,
              matchedBytes: stats.matched,
              itemCount: stats.count,
              kind: 'volume',
              volumeId: vid,
              folderPath: null,
              entryId: null,
              colorIndex: color,
            });
          }
          volChildren.sort((a, b) => b.size - a.size);
          catChildren.push({
            id: `cat-${catName}`,
            name: catName,
            size: catSize,
            matchedBytes: catMatched,
            itemCount: catCount,
            kind: 'category',
            volumeId: null,
            folderPath: null,
            entryId: null,
            searchQuery: catName,
            colorIndex: color,
            children: volChildren,
          });
        }
        catChildren.sort((a, b) => b.size - a.size);

        root = {
          id: 'archive-categories-root',
          name: `Весь архив (${vols.length} дисков)`,
          size: grandSize,
          matchedBytes: grandMatched,
          itemCount: grandCount,
          kind: 'root',
          volumeId: null,
          folderPath: null,
          entryId: null,
          colorIndex: 0,
          children: catChildren,
        };
      } else {
        const volNodes: SunburstNode[] = [];
        let grandSize = 0;
        let grandMatched = 0;
        let grandCount = 0;

        for (const vol of vols) {
          const tree = buildVolumeFolderTree(vol.id, `#${vol.id} · ${vol.label}`, null, matchPred, (vol.id - 1) % 8, 2);
          if (tree.size <= 0) continue;
          grandSize += tree.size;
          grandMatched += tree.matchedBytes;
          grandCount += tree.itemCount;
          volNodes.push({
            ...tree,
            id: `vol-${vol.id}`,
            kind: 'volume',
          });
        }
        volNodes.sort((a, b) => b.size - a.size);

        root = {
          id: 'archive-volumes-root',
          name: `Весь архив (${vols.length} дисков)`,
          size: grandSize,
          matchedBytes: grandMatched,
          itemCount: grandCount,
          kind: 'root',
          volumeId: null,
          folderPath: null,
          entryId: null,
          colorIndex: 0,
          children: volNodes,
        };
      }

      res.json({
        root,
        breadcrumbs,
        totalCapacityBytes,
        totalUsedBytes,
        totalIndexedBytes,
        elapsedMs: Math.max(1, Math.round(performance.now() - started)),
      });
    } catch (err) {
      res.status(400).json({ error: (err as Error).message });
    }
  });

  // 5. Multi-Dimensional Video Archive Cross-Sections ("Разрезы архива")
  app.post('/api/catalog/slices', (req: Request, res: Response) => {
    const started = performance.now();
    try {
      const {
        volumeId = null,
        category = null,
      } = (req.body ?? {}) as {
        volumeId?: number | null;
        category?: string | null;
      };

      const vols = buildVolumesList();
      const scopedVols = vols.filter((v) => {
        if (volumeId !== null && volumeId !== undefined && v.id !== Number(volumeId)) return false;
        return true;
      });
      const scopedVolIds = new Set(scopedVols.map((v) => v.id));

      let totalFiles = 0;
      let totalDirectories = 0;
      let totalIndexedBytes = 0;
      const totalCapacityBytes = scopedVols.reduce((acc, v) => acc + v.capacityBytes, 0);

      const catStats = new Map<string, { bytes: number; fileCount: number; vols: Set<number> }>();
      const extStats = new Map<string, { bytes: number; fileCount: number }>();
      const resStats = new Map<string, { bytes: number; fileCount: number; query: string }>();
      const codecStats = new Map<string, { bytes: number; fileCount: number; query: string }>();
      const yearStats = new Map<number, { bytes: number; fileCount: number }>();
      const volFileStats = new Map<number, { bytes: number; fileCount: number }>();
      const dupClassesInScope = new Map<string, {
        sampleEntryId: number;
        name: string;
        size: number;
        copies: number;
        disks: Set<number>;
      }>();

      const sizeBuckets = [
        { id: 'gt50', label: 'UHD BD-Remux (> 50 GB)', rangeHint: '> 50 GB', min: 50 * GB, max: Infinity, query: 'size:>50gb', fileCount: 0, bytes: 0 },
        { id: '20to50', label: 'Remux / 4K (20–50 GB)', rangeHint: '20–50 GB', min: 20 * GB, max: 50 * GB, query: 'size:>=20gb AND size:<=50gb', fileCount: 0, bytes: 0 },
        { id: '8to20', label: '1080p / Серии UHD (8–20 GB)', rangeHint: '8–20 GB', min: 8 * GB, max: 20 * GB, query: 'size:>=8gb AND size:<20gb', fileCount: 0, bytes: 0 },
        { id: '1to8', label: 'Рипы 720p/1080p (1–8 GB)', rangeHint: '1–8 GB', min: 1 * GB, max: 8 * GB, query: 'size:>=1gb AND size:<8gb', fileCount: 0, bytes: 0 },
        { id: 'lt1g', label: 'Субтитры и прочее (< 1 GB)', rangeHint: '< 1 GB', min: 0, max: 1 * GB, query: 'size:<1gb', fileCount: 0, bytes: 0 },
      ];

      for (const e of rawEntries) {
        if (!scopedVolIds.has(e.volumeId)) continue;
        if (category && e.category !== category) continue;

        if (e.kind === 'directory') {
          totalDirectories++;
          continue;
        }

        totalFiles++;
        totalIndexedBytes += e.size;

        const cSlot = catStats.get(e.category) ?? { bytes: 0, fileCount: 0, vols: new Set<number>() };
        cSlot.bytes += e.size;
        cSlot.fileCount += 1;
        cSlot.vols.add(e.volumeId);
        catStats.set(e.category, cSlot);

        const extKey = e.ext || 'other';
        const xSlot = extStats.get(extKey) ?? { bytes: 0, fileCount: 0 };
        xSlot.bytes += e.size;
        xSlot.fileCount += 1;
        extStats.set(extKey, xSlot);

        if (e.videoMeta) {
          const rLabel =
            e.videoMeta.resolutionTag === '2160p'
              ? '4K UHD (2160p)'
              : e.videoMeta.resolutionTag === '1080p'
                ? 'FullHD (1080p)'
                : e.videoMeta.resolutionTag === '720p'
                  ? 'HD (720p)'
                  : 'SD / DVD';
          const rQuery = `res:${e.videoMeta.resolutionTag.toLowerCase()}`;
          const rSlot = resStats.get(rLabel) ?? { bytes: 0, fileCount: 0, query: rQuery };
          rSlot.bytes += e.size;
          rSlot.fileCount += 1;
          resStats.set(rLabel, rSlot);

          const cLabel = e.videoMeta.codec;
          const cQuery = `codec:${e.videoMeta.codec.toLowerCase()}`;
          const cdSlot = codecStats.get(cLabel) ?? { bytes: 0, fileCount: 0, query: cQuery };
          cdSlot.bytes += e.size;
          cdSlot.fileCount += 1;
          codecStats.set(cLabel, cdSlot);
        }

        for (const b of sizeBuckets) {
          if (e.size >= b.min && e.size < b.max) {
            b.fileCount += 1;
            b.bytes += e.size;
            break;
          }
        }

        const yr = new Date(e.modifiedUnix * 1000).getUTCFullYear();
        const ySlot = yearStats.get(yr) ?? { bytes: 0, fileCount: 0 };
        ySlot.bytes += e.size;
        ySlot.fileCount += 1;
        yearStats.set(yr, ySlot);

        const vSlot = volFileStats.get(e.volumeId) ?? { bytes: 0, fileCount: 0 };
        vSlot.bytes += e.size;
        vSlot.fileCount += 1;
        volFileStats.set(e.volumeId, vSlot);

        if (e.size >= MB) {
          const candidateKey = metadataCandidateKey(e.name, e.size);
          const existingDup = dupClassesInScope.get(candidateKey);
          if (!existingDup) {
            dupClassesInScope.set(candidateKey, {
              sampleEntryId: e.id,
              name: e.name,
              size: e.size,
              copies: 1,
              disks: new Set([e.volumeId]),
            });
          } else {
            existingDup.copies += 1;
            existingDup.disks.add(e.volumeId);
          }
        }
      }

      const safeTotal = Math.max(1, totalIndexedBytes);

      const byCategory = Array.from(catStats.entries())
        .sort((a, b) => b[1].bytes - a[1].bytes)
        .map(([cat, s], idx) => ({
          category: cat,
          fileCount: s.fileCount,
          volumeCount: s.vols.size,
          bytes: s.bytes,
          sharePct: (s.bytes / safeTotal) * 100,
          searchQuery: `"${cat.split(' ')[0]}"`,
          colorIndex: idx % 8,
        }));

      const byExtension = Array.from(extStats.entries())
        .sort((a, b) => b[1].bytes - a[1].bytes)
        .slice(0, 12)
        .map(([ext, s]) => ({
          ext: ext === 'other' ? '(без расш.)' : `.${ext}`,
          fileCount: s.fileCount,
          bytes: s.bytes,
          avgBytes: s.fileCount > 0 ? Math.round(s.bytes / s.fileCount) : 0,
          sharePct: (s.bytes / safeTotal) * 100,
          searchQuery: ext === 'other' ? '' : `ext:${ext}`,
        }));

      const byResolution = Array.from(resStats.entries())
        .sort((a, b) => b[1].bytes - a[1].bytes)
        .map(([resolution, s]) => ({
          resolution,
          fileCount: s.fileCount,
          bytes: s.bytes,
          sharePct: (s.bytes / safeTotal) * 100,
          searchQuery: s.query,
        }));

      const byVideoCodec = Array.from(codecStats.entries())
        .sort((a, b) => b[1].bytes - a[1].bytes)
        .map(([codec, s]) => ({
          codec,
          fileCount: s.fileCount,
          bytes: s.bytes,
          sharePct: (s.bytes / safeTotal) * 100,
          searchQuery: s.query,
        }));

      const bySizeBucket = sizeBuckets.map((b) => ({
        id: b.id,
        label: b.label,
        rangeHint: b.rangeHint,
        fileCount: b.fileCount,
        bytes: b.bytes,
        sharePct: (b.bytes / safeTotal) * 100,
        searchQuery: b.query,
      }));

      const byYear = Array.from(yearStats.entries())
        .sort((a, b) => b[0] - a[0])
        .map(([year, s]) => ({
          year,
          fileCount: s.fileCount,
          bytes: s.bytes,
          sharePct: (s.bytes / safeTotal) * 100,
          searchQuery: `year:${year}`,
        }));

      let uniqueLogicalBytes = 0;
      let duplicateExcessBytes = 0;
      let singleCopyBytes = 0;
      let multiCopyProtectedBytes = 0;

      const tiers = {
        single: { distinctClasses: 0, fileInstances: 0, logicalBytes: 0, occupiedBytes: 0, reclaimableBytes: 0 },
        paired: { distinctClasses: 0, fileInstances: 0, logicalBytes: 0, occupiedBytes: 0, reclaimableBytes: 0 },
        excessive: { distinctClasses: 0, fileInstances: 0, logicalBytes: 0, occupiedBytes: 0, reclaimableBytes: 0 },
      };

      const topDupCandidates: {
        sampleEntryId: number;
        name: string;
        size: number;
        copies: number;
        excessBytes: number;
        diskIds: number[];
      }[] = [];

      const candidateVolumeCounts = countCandidateVolumes(
        rawEntries.filter((entry) => entry.kind === 'file' && entry.size >= MB)
      );

      for (const [candidateKey, cls] of dupClassesInScope.entries()) {
        const globalVolumeCount = candidateVolumeCounts.get(candidateKey) ?? cls.disks.size;
        const occupied = cls.size * cls.copies;
        const excess = cls.size * Math.max(0, cls.copies - 1);
        uniqueLogicalBytes += cls.size;
        duplicateExcessBytes += excess;

        if (globalVolumeCount <= 1) {
          singleCopyBytes += occupied;
          tiers.single.distinctClasses += 1;
          tiers.single.fileInstances += cls.copies;
          tiers.single.logicalBytes += cls.size;
          tiers.single.occupiedBytes += occupied;
        } else if (globalVolumeCount === 2) {
          multiCopyProtectedBytes += occupied;
          tiers.paired.distinctClasses += 1;
          tiers.paired.fileInstances += cls.copies;
          tiers.paired.logicalBytes += cls.size;
          tiers.paired.occupiedBytes += occupied;
          tiers.paired.reclaimableBytes += excess;
        } else {
          multiCopyProtectedBytes += occupied;
          tiers.excessive.distinctClasses += 1;
          tiers.excessive.fileInstances += cls.copies;
          tiers.excessive.logicalBytes += cls.size;
          tiers.excessive.occupiedBytes += occupied;
          tiers.excessive.reclaimableBytes += excess;
        }

        if (cls.copies > 1) {
          topDupCandidates.push({
            sampleEntryId: cls.sampleEntryId,
            name: cls.name,
            size: cls.size,
            copies: cls.copies,
            excessBytes: excess,
            diskIds: Array.from(cls.disks).sort((a, b) => a - b),
          });
        }
      }

      topDupCandidates.sort((a, b) => b.excessBytes - a.excessBytes);

      const byDuplicationTier = [
        {
          tier: 'single' as const,
          label: 'Один диск с кандидатом',
          description: 'Совпадение метаданных найдено на одном диске; резерв не подтверждён',
          ...tiers.single,
          searchQuery: 'dup:1',
        },
        {
          tier: 'paired' as const,
          label: 'Два диска с кандидатом',
          description: 'Совпадения метаданных найдены на двух дисках; содержимое не сверено',
          ...tiers.paired,
          searchQuery: 'dup:2',
        },
        {
          tier: 'excessive' as const,
          label: '3+ диска с кандидатом',
          description: 'Совпадения метаданных найдены на трёх и более дисках; содержимое не сверено',
          ...tiers.excessive,
          searchQuery: 'dup:>=3',
        },
      ];

      const topVolumes = scopedVols
        .map((v) => {
          const vs = volFileStats.get(v.id);
          return {
            volumeId: v.id,
            label: v.label,
            diskCode: v.diskCode,
            shelfTag: '',
            physicalStatus: v.physicalStatus,
            mountedPath: v.mountedPath,
            indexedBytes: vs?.bytes ?? 0,
            usedBytes: v.usedBytes,
            capacityBytes: v.capacityBytes,
            fileCount: vs?.fileCount ?? 0,
            shelfConflict: false,
          };
        })
        .sort((a, b) => b.indexedBytes - a.indexedBytes || a.volumeId - b.volumeId);

      const scopeLabel = volumeId
        ? `Диск #${volumeId} (${scopedVols[0]?.label ?? ''})`
        : `Весь видеоархив (${scopedVols.length} USB)`;

      res.json({
        scopeLabel,
        volumeCount: scopedVols.length,
        totalFiles,
        totalDirectories,
        totalIndexedBytes,
        totalCapacityBytes,
        uniqueLogicalBytes,
        duplicateExcessBytes,
        singleCopyBytes,
        multiCopyProtectedBytes,
        elapsedMs: Math.max(1, Math.round(performance.now() - started)),
        byCategory,
        byExtension,
        byResolution,
        byVideoCodec,
        bySizeBucket,
        byYear,
        byDuplicationTier,
        byShelfRack: [],
        topDuplicateGroups: topDupCandidates.slice(0, 12),
        topVolumes: topVolumes.slice(0, 20),
      });
    } catch (err) {
      res.status(400).json({ error: (err as Error).message });
    }
  });

  // 6. Main Search / Browse / 4-Mode Video Duplicates / Copy Candidates Endpoint
  app.post('/api/catalog/ancestors', (req: Request, res: Response) => {
    const volumeId = Number(req.body?.volumeId);
    let parentId = req.body?.parentId === null ? null : Number(req.body?.parentId);
    if (!Number.isInteger(volumeId) || (parentId !== null && !Number.isInteger(parentId))) {
      return res.status(400).json({ error: 'Invalid folder scope' });
    }
    const ancestors: { id: number; name: string }[] = [];
    const visited = new Set<number>();
    while (parentId !== null && !visited.has(parentId)) {
      visited.add(parentId);
      const folder = rawEntries.find((entry) => entry.id === parentId && entry.volumeId === volumeId && entry.kind === 'directory');
      if (!folder) return res.status(404).json({ error: 'Folder not found' });
      ancestors.push({ id: folder.id, name: folder.name });
      parentId = folder.parentId;
    }
    return res.json({ ancestors: ancestors.reverse() });
  });

  app.post('/api/catalog/query', async (req: Request, res: Response) => {
    const started = performance.now();
    const {
      requestId,
      mode = 'search',
      query = '',
      volumeId = null,
      parentId = null,
      folderPrefix = null,
      category = null,
      duplicateMode = 'exact_bytes',
      sourceEntryId = null,
      candidateVolumeId = null,
      otherDisksOnly = true,
      offset = 0,
      limit = 250,
    } = (req.body ?? {}) as {
      requestId?: string;
      mode?: 'search' | 'browse' | 'duplicates' | 'copy_candidates';
      query?: string;
      volumeId?: number | null;
      parentId?: number | null;
      folderPrefix?: string | null;
      category?: string | null;
      duplicateMode?: string;
      sourceEntryId?: number | null;
      candidateVolumeId?: number | null;
      otherDisksOnly?: boolean;
      offset?: number;
      limit?: number;
    };

    const jobKey = requestId || crypto.randomUUID();
    const jobState = { aborted: false };
    activeSearchJobs.set(jobKey, jobState);

    const checkAbort = () => {
      if (jobState.aborted) {
        const err = new Error('Поисковый запрос прерван.');
        (err as Error & { code?: string }).code = 'query_cancelled';
        throw err;
      }
    };

    try {
      checkAbort();
      const vols = buildVolumesList();
      const volLabelMap = new Map(vols.map((v) => [v.id, v.label]));
      const volObjMap = new Map(vols.map((v) => [v.id, v]));

      const clampedLimit = Math.max(1, Math.min(250, Number(limit) || 250));
      const safeOffset = Math.max(0, Number(offset) || 0);

      let matched: RawEntry[] = [];
      let scopeDescription = `Все носители (${vols.length})`;
      let copyCandidatesMeta = null;
      let duplicateSummary: {
        modeUsed: 'exact_bytes' | 'video_stream' | 'video_rips' | 'same_name';
        groupCount: number;
        totalFiles: number;
        reclaimableBytes: number;
      } | null = null;

      const itemGroupMeta = new Map<
        number,
        { key: string; label: string; count: number; excessBytes: number; disks: number[] }
      >();

      if (mode === 'copy_candidates') {
        const target = rawEntries.find((e) => e.id === Number(sourceEntryId) && e.kind === 'file');
        if (!target) {
          res.status(404).json({ error: 'Выбранный файл не найден в каталоге.' });
          return;
        }
        // Metadata-only candidates: matching name+size or an explicitly recorded work key.
        const allSameClass = rawEntries.filter((e) => {
          if (e.kind !== 'file') return false;
          if (target.videoMeta?.workKey && e.videoMeta?.workKey === target.videoMeta.workKey) return true;
          return e.size >= MB && metadataCandidateKey(e.name, e.size) === metadataCandidateKey(target.name, target.size);
        });

        const byDisk = new Map<number, number>();
        for (const item of allSameClass) {
          byDisk.set(item.volumeId, (byDisk.get(item.volumeId) ?? 0) + 1);
        }

        let groups = Array.from(byDisk.entries())
          .sort((a, b) => a[0] - b[0])
          .map(([vid, count]) => {
            const vObj = volObjMap.get(vid);
            return {
              volumeId: vid,
              label: vObj?.label ?? `Disk #${vid}`,
              count,
              shelfTag: '',
              physicalStatus: 'offline' as PhysicalStatus,
            };
          });

        const allTotal = allSameClass.length;
        const otherTotal = allSameClass.filter((e) => e.volumeId !== target.volumeId || e.id !== target.id).length;
        if (otherDisksOnly) groups = groups.filter((g) => g.volumeId !== target.volumeId);

        matched = allSameClass
          .filter((e) => {
            if (otherDisksOnly && e.volumeId === target.volumeId) return false;
            if (candidateVolumeId !== null && candidateVolumeId !== undefined && e.volumeId !== Number(candidateVolumeId)) {
              return false;
            }
            return true;
          })
          .sort((a, b) => b.size - a.size || a.volumeId - b.volumeId);

        copyCandidatesMeta = {
          sourceEntryId: target.id,
          sourceEntryName: target.name,
          sourceVolumeId: target.volumeId,
          candidateVolumeId: candidateVolumeId ? Number(candidateVolumeId) : null,
          allTotal,
          otherTotal,
          otherDisksOnly,
          distinctDisks: groups.length,
          diskGroups: groups,
          basis: 'exact_normalized_name_and_size' as const,
          verifiedBytes: false as const,
        };
        scopeDescription = `Копии и версии «${target.name}» на других USB`;
      } else if (mode === 'duplicates') {
        const pred = compilePredicate(query);
        const filesOnly = rawEntries.filter((e) => {
          if (e.kind !== 'file') return false;
          if (volumeId !== null && volumeId !== undefined && e.volumeId !== Number(volumeId)) return false;
          return pred(e);
        });

        const normMode: 'exact_bytes' | 'video_stream' | 'video_rips' | 'same_name' =
          duplicateMode === 'video_stream'
            ? 'video_stream'
            : duplicateMode === 'video_rips'
              ? 'video_rips'
              : duplicateMode === 'same_name' || duplicateMode === 'same_name_diff_size'
                ? 'same_name'
                : 'exact_bytes';

        const buckets = new Map<string, RawEntry[]>();

        for (const f of filesOnly) {
          if (normMode === 'exact_bytes') {
            if (f.size < MB) continue;
            const key = `bytes::${f.size}`;
            const list = buckets.get(key) ?? [];
            list.push(f);
            buckets.set(key, list);
          } else if (normMode === 'video_stream') {
            // Similar stream metadata only: duration bucket, resolution and codec do not prove stream identity.
            if (!f.videoMeta) continue;
            const durBucket = Math.round(f.videoMeta.durationSec / 2);
            const key = `vstream::${durBucket}::${f.videoMeta.resolutionTag}::${f.videoMeta.codec}`;
            const list = buckets.get(key) ?? [];
            list.push(f);
            buckets.set(key, list);
          } else if (normMode === 'video_rips') {
            // Same film across 4K / 1080p / 720p: group by workKey OR duration bucket (±2s)
            if (!f.videoMeta) continue;
            const key = f.videoMeta.workKey
              ? `work::${f.videoMeta.workKey}`
              : `durwork::${Math.round(f.videoMeta.durationSec / 2)}`;
            const list = buckets.get(key) ?? [];
            list.push(f);
            buckets.set(key, list);
          } else {
            const key = `name::${f.nameNorm}`;
            const list = buckets.get(key) ?? [];
            list.push(f);
            buckets.set(key, list);
          }
        }

        interface DupGroup {
          key: string;
          label: string;
          entries: RawEntry[];
          excessBytes: number;
          sortWeight: number;
          disks: number[];
        }
        const validGroups: DupGroup[] = [];
        let totalReclaimable = 0;

        for (const [key, group] of buckets.entries()) {
          if (group.length < 2) continue;

          if (normMode === 'video_stream') {
            // Show groups where at least 2 files have DIFFERENT byte sizes (i.e. remuxes/repacks that byte-size alone misses!)
            // OR multiple copies of that stream
            const distinctSizes = new Set(group.map((g) => g.size));
            if (distinctSizes.size < 2) continue;
          } else if (normMode === 'video_rips') {
            // Show groups where resolutions/qualities differ (e.g. 2160p vs 1080p vs 720p)
            const distinctRes = new Set(group.map((g) => g.videoMeta?.resolutionTag));
            if (distinctRes.size < 2) continue;
          } else if (normMode === 'same_name') {
            const distinctSizes = new Set(group.map((g) => g.size));
            if (distinctSizes.size < 2) continue;
          }

          // Sort highest quality / largest master first!
          group.sort((a, b) => b.size - a.size || a.volumeId - b.volumeId);

          // Hypothetical excess bytes in candidate groups; NOT proven reclaimable or safe to delete.
          const excess = group.slice(1).reduce((acc, item) => acc + item.size, 0);
          totalReclaimable += excess;
          const disks = Array.from(new Set(group.map((g) => g.volumeId))).sort((a, b) => a - b);

          let groupLabel = group[0].name;
          if (normMode === 'exact_bytes') {
            const distinctNames = new Set(group.map((g) => g.nameNorm));
            groupLabel =
              distinctNames.size > 1
                ? `${group[0].name} (+ переим. копия)`
                : group[0].name;
          } else if (normMode === 'video_stream' && group[0].videoMeta) {
            groupLabel = `Один видеопоток ${group[0].videoMeta.resolutionTag} ${group[0].videoMeta.codec} · разная упаковка/озвучка`;
          } else if (normMode === 'video_rips') {
            const resChain = Array.from(new Set(group.map((g) => g.videoMeta?.resolutionTag))).join(' → ');
            groupLabel = `${group[0].name.replace(/_/g, ' ')} (${resChain})`;
          }

          validGroups.push({
            key,
            label: groupLabel,
            entries: group,
            excessBytes: excess,
            sortWeight: excess,
            disks,
          });
        }

        validGroups.sort((a, b) => b.sortWeight - a.sortWeight);
        for (const grp of validGroups) {
          for (const e of grp.entries) {
            matched.push(e);
            itemGroupMeta.set(e.id, {
              key: grp.key,
              label: grp.label,
              count: grp.entries.length,
              excessBytes: grp.excessBytes,
              disks: grp.disks,
            });
          }
        }

        duplicateSummary = {
          modeUsed: normMode,
          groupCount: validGroups.length,
          totalFiles: matched.length,
          reclaimableBytes: totalReclaimable,
        };

        scopeDescription =
          normMode === 'video_stream'
            ? 'Ремуксы и перепаковки одного видеопотока (совпадение длительности + разрешения + кодека)'
            : normMode === 'video_rips'
              ? 'Разные рипы одного фильма (4K UHD vs 1080p vs 720p)'
              : normMode === 'same_name'
                ? 'Одноимённые файлы разного размера'
                : 'Кандидаты по имени и размеру; содержимое не проверено';
      } else if (mode === 'browse' && volumeId) {
        const targetParent = parentId === null || parentId === undefined ? null : Number(parentId);
        matched = rawEntries
          .filter((e) => e.volumeId === Number(volumeId) && e.parentId === targetParent)
          .sort((a, b) => {
            if (a.kind !== b.kind) return a.kind === 'directory' ? -1 : 1;
            return b.size - a.size || a.nameNorm.localeCompare(b.nameNorm);
          });
        const vLabel = volLabelMap.get(Number(volumeId)) ?? `#${volumeId}`;
        scopeDescription = `Обзор диска #${volumeId} · ${vLabel}`;
      } else {
        const pred = compilePredicate(query);
        const normPrefix = folderPrefix ? `${folderPrefix.replace(/^\/+|\/+$/g, '')}/` : null;
        matched = rawEntries
          .filter((e) => {
            if (volumeId !== null && volumeId !== undefined && e.volumeId !== Number(volumeId)) return false;
            if (normPrefix && !e.relativePath.startsWith(normPrefix) && e.relativePath !== folderPrefix) return false;
            if (category && e.category !== category) return false;
            return pred(e);
          })
          .sort((a, b) => {
            if (a.kind !== b.kind) return a.kind === 'file' ? -1 : 1;
            return b.size - a.size || a.nameNorm.localeCompare(b.nameNorm);
          });

        if (volumeId && folderPrefix) {
          scopeDescription = `Диск #${volumeId} · /${folderPrefix}`;
        } else if (volumeId) {
          scopeDescription = `Диск #${volumeId} (${volLabelMap.get(Number(volumeId)) ?? ''})`;
        } else if (category) {
          scopeDescription = `${category} · все диски`;
        }
      }

      let totalMatchedBytes = 0;
      for (const m of matched) {
        if (m.kind === 'file') totalMatchedBytes += m.size;
      }

      const pagedRaw = matched.slice(safeOffset, safeOffset + clampedLimit);
      const items = pagedRaw.map((e) => {
        const gMeta = itemGroupMeta.get(e.id);
        const exactCopies = e.kind === 'file' && e.size >= MB
          ? (metadataCandidateVolumeCounts.get(metadataCandidateKey(e.name, e.size)) ?? 1)
          : 1;
        return {
          id: e.id,
          volumeId: e.volumeId,
          volumeLabel: volLabelMap.get(e.volumeId) ?? `Disk #${e.volumeId}`,
          parentId: e.parentId,
          name: e.name,
          nameNorm: e.nameNorm,
          relativePath: e.relativePath,
          ext: e.ext,
          kind: e.kind,
          size: e.size,
          modifiedUnix: e.modifiedUnix,
          category: e.category,
          description: e.description,
          tags: e.tags,
          duplicateCount: exactCopies,
          fastFingerprint: e.fastFingerprint ?? null,
          videoMeta: e.videoMeta ?? null,
          dupGroupKey: gMeta?.key ?? null,
          dupGroupLabel: gMeta?.label ?? null,
          dupGroupCount: gMeta?.count,
          dupGroupExcessBytes: gMeta?.excessBytes,
          dupGroupDisks: gMeta?.disks,
          activeOverlayNote: null,
          opaqueFlag0x10: e.opaqueFlag0x10,
        };
      });

      const currentPageDiskCounts: Record<number, number> = {};
      for (const row of items) {
        currentPageDiskCounts[row.volumeId] = (currentPageDiskCounts[row.volumeId] ?? 0) + 1;
      }

      const elapsedMs = Math.max(1, Math.round(performance.now() - started));
      if (mode === 'search' && !volumeId && !category && query.trim().length > 0) {
        recordGlobalLatency(elapsedMs);
      }

      res.json({
        items,
        total: matched.length,
        totalMatchedBytes,
        offset: safeOffset,
        limit: clampedLimit,
        hasMore: safeOffset + items.length < matched.length,
        elapsedMs,
        sourceMode,
        verifiedGenerationSha: pinnedGenerationSha,
        scopeDescription,
        currentPageDiskCounts,
        duplicateSummary,
        copyCandidatesMeta,
        latencySummary: computeLatencySummary(),
      });
    } catch (err) {
      const e = err as Error & { code?: string };
      const status = e.code === 'query_cancelled' ? 499 : 400;
      res.status(status).json({ error: e.message, code: e.code ?? 'query_error' });
    } finally {
      activeSearchJobs.delete(jobKey);
    }
  });

  // 7. Start Simulated Background USB Scan Job (for testing progress & cancel in ScanDiskModal)
  app.post('/api/scan/start', (req: Request, res: Response) => {
    if (activeScanJob && activeScanJob.status === 'running') {
      res.status(409).json({ error: 'Сканирование уже выполняется.' });
      return;
    }

    const {
      targetVolumeId = null,
      label = '',
      diskCode = '',
      note = '',
      plugins = ['fs_stat', 'video_header'],
    } = (req.body ?? {}) as {
      targetVolumeId?: number | null;
      label?: string;
      diskCode?: string;
      note?: string;
      plugins?: string[];
    };

    const targetMount = osMountsByPlatform[platformPreset][0];
    const existingVolId =
      targetVolumeId && Number.isInteger(Number(targetVolumeId)) ? Number(targetVolumeId) : null;
    const existingVol = existingVolId ? rawVolumes.find((v) => v.id === existingVolId) : null;

    const job: ScanJobInternal = {
      jobId: `scan-${Date.now()}`,
      targetLabel: label.trim() || existingVol?.label || targetMount.label,
      mountPoint: targetMount.mountPoint,
      physicalId: targetMount.physicalId,
      filesystem: targetMount.filesystem,
      capacityBytes: targetMount.capacityBytes,
      shelfTag: '',
      status: 'running',
      aborted: false,
      scannedFiles: 0,
      totalExpectedFiles: 1_840,
      scannedBytes: 0,
      totalExpectedBytes: targetMount.usedBytes,
      currentPath: targetMount.mountPoint,
      filesPerSec: 0,
      bytesPerSec: 0,
      duplicatesDetected: 0,
      createdVolumeId: existingVolId,
      startedAtMs: Date.now(),
      elapsedMs: 0,
      pluginsUsed: plugins,
    };

    activeScanJob = job;
    void executeScanSimulation(job, existingVolId, plugins, note.trim(), diskCode.trim());
    res.json({ ok: true, job: serializeScanJob(job) });
  });

  // 8. Cancel Running Volume Scan Job
  app.post('/api/scan/cancel', (_req: Request, res: Response) => {
    if (activeScanJob && activeScanJob.status === 'running') {
      activeScanJob.aborted = true;
      res.json({ ok: true, cancelledJobId: activeScanJob.jobId });
      return;
    }
    res.json({ ok: false });
  });

  // 9. Ingest Real Directory Selected by User in ScanDiskModal (with WhereIsIt-style Plugins!)
  app.post('/api/scan/ingest-local', (req: Request, res: Response) => {
    const {
      folderName = 'USB_Volume',
      targetVolumeId = null,
      diskCode = '',
      hardwareModel = 'USB External Drive',
      note = '',
      capacityBytes = 0,
      plugins = { videoHeader: true, sparseHash: false, archiveList: false },
      files = [],
    } = (req.body ?? {}) as {
      folderName?: string;
      targetVolumeId?: number | null;
      diskCode?: string;
      hardwareModel?: string;
      note?: string;
      capacityBytes?: number;
      plugins?: { videoHeader?: boolean; sparseHash?: boolean; archiveList?: boolean };
      files?: {
        relativePath: string;
        name: string;
        size: number;
        lastModifiedMs: number;
        fastFingerprint?: string | null;
        videoMeta?: VideoMeta | null;
      }[];
    };

    if (!Array.isArray(files) || files.length === 0) {
      res.status(400).json({ error: 'В выбранной папке нет доступных для индексации файлов.' });
      return;
    }

    let totalBytes = 0;
    for (const f of files) totalBytes += Math.max(0, Number(f.size) || 0);

    const pluginList = ['fs_stat'];
    if (plugins?.videoHeader) pluginList.push('video_header');
    if (plugins?.sparseHash) pluginList.push('sparse_3x64kb');
    if (plugins?.archiveList) pluginList.push('archive_list');

    let targetVolId =
      targetVolumeId && Number.isInteger(Number(targetVolumeId))
        ? Number(targetVolumeId)
        : null;
    const existingVol = targetVolId ? rawVolumes.find((v) => v.id === targetVolId) : undefined;

    if (existingVol && targetVolId) {
      for (let i = rawEntries.length - 1; i >= 0; i--) {
        if (rawEntries[i].volumeId === targetVolId) {
          rawEntries.splice(i, 1);
        }
      }
      for (const k of Array.from(folderCache.keys())) {
        if (k.startsWith(`${targetVolId}:`)) folderCache.delete(k);
      }
      if (folderName.trim()) existingVol.label = folderName.trim().slice(0, 80);
      if (diskCode.trim()) existingVol.diskCode = diskCode.trim().slice(0, 24);
      if (hardwareModel.trim()) existingVol.hardwareModel = hardwareModel.trim().slice(0, 64);
      if (note.trim()) existingVol.note = note.trim().slice(0, 400);
      existingVol.usedBytes = totalBytes;
      existingVol.capacityBytes = Math.max(
        capacityBytes > 0 ? capacityBytes : existingVol.capacityBytes,
        totalBytes
      );
      existingVol.pluginsUsed = pluginList;
      existingVol.cataloguedAtUnix = Math.floor(Date.now() / 1000);
    } else {
      targetVolId = rawVolumes.length + 1;
      const defaultCap = capacityBytes > 0 ? capacityBytes : Math.max(Math.round(totalBytes * 1.25), 2 * TB);
      rawVolumes.push({
        id: targetVolId,
        label: folderName.trim().slice(0, 80) || `USB_Disk_${targetVolId}`,
        diskCode: diskCode.trim().slice(0, 24) || `USB-${String(targetVolId).padStart(2, '0')}`,
        hardwareModel: hardwareModel.trim().slice(0, 64) || 'USB External HDD',
        serialNumber: `SN-LOCAL-${targetVolId}`,
        note: note.trim().slice(0, 400),
        recordedRoot: `USB:/${folderName}`,
        filesystem: 'NTFS/exFAT',
        capacityBytes: Math.max(defaultCap, totalBytes),
        usedBytes: totalBytes,
        pluginsUsed: pluginList,
        cataloguedAtUnix: Math.floor(Date.now() / 1000),
        sourceKind: 'scanned_usb',
      });
    }

    let duplicatesFound = 0;
    let videoHeadersCollected = 0;
    const cappedFiles = files.slice(0, 15000);

    for (const item of cappedFiles) {
      const cleanRel = String(item.relativePath || item.name).replace(/^\/+/, '');
      const parts = cleanRel.split('/').filter(Boolean);
      const fileName = parts[parts.length - 1] || item.name || 'unnamed';
      const dirPart = parts.length > 1 ? parts.slice(0, -1).join('/') : '';
      const dotIdx = fileName.lastIndexOf('.');
      const ext = dotIdx > 0 ? fileName.slice(dotIdx + 1).toLowerCase() : '';
      const category = categorizeByExt(ext, dirPart);
      const modUnix = item.lastModifiedMs ? Math.floor(item.lastModifiedMs / 1000) : Math.floor(Date.now() / 1000);
      const size = Math.max(0, Number(item.size) || 0);

      if (size >= MB && (metadataCandidateCounts.get(metadataCandidateKey(fileName, size)) ?? 0) > 0) duplicatesFound++;

      let vMeta: VideoMeta | null = null;
      if (plugins?.videoHeader) {
        vMeta = item.videoMeta ?? inferVideoMetaFromNameAndSize(fileName, size);
        if (vMeta) videoHeadersCollected++;
      }

      const parentId = dirPart ? ensureFolderChain(targetVolId, dirPart, modUnix, category) : null;
      pushEntry({
        volumeId: targetVolId,
        parentId,
        name: fileName,
        relativePath: cleanRel,
        kind: 'file',
        size,
        modifiedUnix: modUnix,
        category,
        description: `USB #${targetVolId} (${folderName})`,
        tags: `usb,${ext || 'file'},vol${targetVolId}`,
        fastFingerprint: plugins?.sparseHash ? (item.fastFingerprint ?? null) : null,
        videoMeta: vMeta,
        opaqueFlag0x10: false,
      });
    }

    rebuildDuplicateClassCounts();
    canonicalSha = crypto
      .createHash('sha256')
      .update(JSON.stringify({ v: 'WhereIsIt-CTF-3.00', count: rawEntries.length }))
      .digest('hex');
    liveSourceSha = canonicalSha;
    pinnedGenerationSha = canonicalSha;

    res.json({
      ok: true,
      createdVolumeId: targetVolId,
      updatedExisting: Boolean(existingVol),
      indexedFiles: cappedFiles.length,
      videoHeadersCollected,
      totalBytes,
      duplicatesFound,
      snapshot: buildSystemSnapshot(),
    });
  });

  // 10. Verify Pin Before Clipboard Export
  app.post('/api/catalog/verify-pin', (_req: Request, res: Response) => {
    res.json({
      ok: true,
      sourceMode,
      verifiedGenerationSha: pinnedGenerationSha,
      volumes: buildVolumesList(),
    });
  });

  // 11. Read-Only Binary .CTF Inspector
  app.post('/api/ctf/inspect', (req: Request, res: Response) => {
    const { fileName = 'catalog.CTF', base64Head = '', totalByteLength = 0 } = (req.body ?? {}) as {
      fileName?: string;
      base64Head?: string;
      totalByteLength?: number;
    };
    const buf = Buffer.from(base64Head, 'base64');
    const sha256 = crypto.createHash('sha256').update(buf).digest('hex');
    const asciiPreview = buf
      .subarray(0, Math.min(64, buf.length))
      .toString('latin1')
      .replace(/[^\x20-\x7E]/g, '·');
    const looksLikeWhereIsItCtf =
      asciiPreview.includes('WhereIsIt') ||
      asciiPreview.includes('Catalog') ||
      fileName.toLowerCase().endsWith('.ctf');

    let opaque0x10Estimate = 0;
    for (let i = 0; i < buf.length; i++) {
      if (buf[i] === 0x10) opaque0x10Estimate++;
    }

    res.json({
      fileName,
      byteLength: Number(totalByteLength) || buf.length,
      sha256,
      headerAscii: asciiPreview || '(пустой заголовок)',
      looksLikeWhereIsItCtf,
      observedVersionMarker: asciiPreview.includes('3.00') ? 'WhereIsIt Catalog 3.00' : 'Binary CTF Stream',
      opaque0x10CountEstimate: opaque0x10Estimate,
      readOnlySafe: true,
      notes: [
        'Файл проверен строго в режиме read-only без изменения исходного каталога.',
      ],
    });
  });

  // 12. Read-Only Local Disk Backup Coverage Check (Zero Catalog Writes)
  app.post('/api/catalog/match-local', (req: Request, res: Response) => {
    const started = performance.now();
    try {
      const { files = [] } = (req.body ?? {}) as {
        files?: { key: string; name: string; size: number }[];
      };

      const capped = Array.isArray(files) ? files.slice(0, 10000) : [];
      const volMap = new Map(rawVolumes.map((v) => [v.id, v.label]));

      // This simulated lookup returns name+size metadata candidates only. It never establishes content equality.
      const byNameSizeIndex = buildMetadataCandidateIndex(
        rawEntries
          .filter((e) => e.kind === 'file')
          .map((e) => ({
            name: e.name,
            size: e.size,
            volumeId: e.volumeId,
            volumeLabel: volMap.get(e.volumeId) ?? `Disk #${e.volumeId}`,
            relativePath: e.relativePath,
          }))
      );

      const matches: Record<string, { volumeId: number; volumeLabel: string; relativePath: string }[]> = {};
      let totalMatchedKeys = 0;
      for (const f of capped) {
        const sz = Number(f.size) || 0;
        const nsKey = metadataCandidateKey(String(f.name ?? ''), sz);
        const hits = byNameSizeIndex.get(nsKey);
        if (hits && hits.length > 0) {
          matches[f.key] = hits;
          totalMatchedKeys++;
        }
      }

      res.json({
        matches,
        totalMatchedKeys,
        checkedCount: capped.length,
        elapsedMs: Math.max(1, Math.round(performance.now() - started)),
      });
    } catch (err) {
      res.status(400).json({ error: (err as Error).message });
    }
  });

  if (process.env.NODE_ENV !== 'production') {
    const vite = await createViteServer({
      server: { middlewareMode: true },
      appType: 'spa',
    });
    app.use(vite.middlewares);
  } else {
    const distPath = path.join(__dirname, 'dist');
    app.use(express.static(distPath));
    app.get('/{*splat}', (_req: Request, res: Response) => {
      res.sendFile(path.join(distPath, 'index.html'));
    });
  }

  const PORT = 3000;
  app.listen(PORT, '127.0.0.1', () => {
    console.log(`WhereIsIt AI running on http://127.0.0.1:${PORT}`);
  });
}

startServer().catch((err) => {
  console.error('Failed to start server:', err);
  process.exit(1);
});
