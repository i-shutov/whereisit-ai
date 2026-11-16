export type PhysicalStatus = 'connected' | 'offline' | 'unverified' | 'conflict';

export type PlatformPreset = 'windows' | 'macos';

export interface VideoMeta {
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

export interface ScanPluginConfig {
  videoHeader: boolean;
  sparseHash: boolean;
  archiveList: boolean;
}

export interface CatalogVolume {
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
  indexedBytes: number;
  entryCount: number;
  videoFileCount: number;
  pluginsUsed: string[];
  cataloguedAtUnix: number;
  shelfTag: string;
  shelfConflictWith: number[];
  physicalStatus: PhysicalStatus;
  pairedPhysicalId: string | null;
  mountedPath: string | null;
  sourceKind?: 'archive_ctf' | 'scanned_usb' | 'local_folder';
}

export type DuplicateCriterion =
  | 'exact_bytes'
  | 'video_stream'
  | 'video_rips'
  | 'same_name'
  | 'name_size'
  | 'size_renamed'
  | 'same_name_diff_size';

export interface CatalogEntry {
  id: number;
  volumeId: number;
  volumeLabel: string;
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
  duplicateCount: number;
  fastFingerprint?: string | null;
  videoMeta?: VideoMeta | null;
  dupGroupKey?: string | null;
  dupGroupLabel?: string | null;
  dupGroupCount?: number;
  dupGroupExcessBytes?: number;
  dupGroupDisks?: number[];
  activeOverlayNote?: string | null;
  opaqueFlag0x10?: boolean;
}

export interface CandidateDiskGroup {
  volumeId: number;
  label: string;
  count: number;
  shelfTag: string;
  physicalStatus: PhysicalStatus;
}

export interface LiveOsMount {
  mountPoint: string;
  physicalId: string;
  serial: string;
  label: string;
  filesystem: string;
  capacityBytes: number;
  usedBytes: number;
  isSystemReserved: boolean;
  isCloneSimulation?: boolean;
  catalogVolumeId?: number | null;
}

export interface SearchLatencySummary {
  sampleCount: number;
  minSampleThreshold: number;
  p50Ms: number | null;
  p95Ms: number | null;
  lastMs: number | null;
  sloTargetMs: number;
  recentSamplesMs: number[];
}

export interface CatalogPageResponse {
  items: CatalogEntry[];
  total: number;
  totalMatchedBytes: number;
  offset: number;
  limit: number;
  hasMore: boolean;
  elapsedMs: number;
  sourceMode: 'linked_ctf' | 'sqlite_local';
  verifiedGenerationSha: string | null;
  scopeDescription: string;
  currentPageDiskCounts: Record<number, number>;
  duplicateSummary?: {
    modeUsed: DuplicateCriterion;
    groupCount: number;
    totalFiles: number;
    reclaimableBytes: number;
  } | null;
  copyCandidatesMeta?: {
    sourceEntryId: number;
    sourceEntryName: string;
    sourceVolumeId: number;
    candidateVolumeId: number | null;
    allTotal: number;
    otherTotal: number;
    otherDisksOnly: boolean;
    distinctDisks: number;
    diskGroups: CandidateDiskGroup[];
    basis: 'exact_normalized_name_and_size';
    verifiedBytes: false;
  } | null;
  latencySummary: SearchLatencySummary;
}

export interface SunburstNode {
  id: string;
  name: string;
  size: number;
  matchedBytes: number;
  itemCount: number;
  kind: 'root' | 'category' | 'volume' | 'directory' | 'file' | 'other_group';
  volumeId: number | null;
  folderPath: string | null;
  entryId: number | null;
  shelfTag?: string;
  physicalStatus?: PhysicalStatus;
  duplicateCount?: number;
  searchQuery?: string;
  colorIndex: number;
  children?: SunburstNode[];
}

export interface SunburstResponse {
  root: SunburstNode;
  breadcrumbs: { label: string; volumeId: number | null; folderPath: string | null }[];
  totalCapacityBytes: number;
  totalUsedBytes: number;
  totalIndexedBytes: number;
  elapsedMs: number;
}

export interface SliceCategoryRow {
  category: string;
  fileCount: number;
  volumeCount: number;
  bytes: number;
  sharePct: number;
  searchQuery: string;
  colorIndex: number;
}

export interface SliceExtensionRow {
  ext: string;
  fileCount: number;
  bytes: number;
  avgBytes: number;
  sharePct: number;
  searchQuery: string;
}

export interface SliceResolutionRow {
  resolution: string;
  fileCount: number;
  bytes: number;
  sharePct: number;
  searchQuery: string;
}

export interface SliceVideoCodecRow {
  codec: string;
  fileCount: number;
  bytes: number;
  sharePct: number;
  searchQuery: string;
}

export interface SliceSizeBucketRow {
  id: string;
  label: string;
  rangeHint: string;
  fileCount: number;
  bytes: number;
  sharePct: number;
  searchQuery: string;
}

export interface SliceYearRow {
  year: number;
  fileCount: number;
  bytes: number;
  sharePct: number;
  searchQuery: string;
}

export interface SliceDuplicationRow {
  tier: 'single' | 'paired' | 'excessive';
  label: string;
  description: string;
  distinctClasses: number;
  fileInstances: number;
  logicalBytes: number;
  occupiedBytes: number;
  reclaimableBytes: number;
  searchQuery: string;
}

export interface SliceShelfGroupRow {
  rack: string;
  volumeCount: number;
  connectedCount: number;
  conflictCount: number;
  capacityBytes: number;
  usedBytes: number;
  indexedBytes: number;
}

export interface SliceDuplicateGroup {
  sampleEntryId: number;
  name: string;
  size: number;
  copies: number;
  excessBytes: number;
  diskIds: number[];
}

export interface SliceVolumeRow {
  volumeId: number;
  label: string;
  diskCode?: string;
  shelfTag: string;
  physicalStatus: PhysicalStatus;
  mountedPath: string | null;
  indexedBytes: number;
  usedBytes: number;
  capacityBytes: number;
  fileCount: number;
  shelfConflict: boolean;
}

export interface ArchiveSlicesResponse {
  scopeLabel: string;
  volumeCount: number;
  totalFiles: number;
  totalDirectories: number;
  totalIndexedBytes: number;
  totalCapacityBytes: number;
  uniqueLogicalBytes: number;
  duplicateExcessBytes: number;
  singleCopyBytes: number;
  multiCopyProtectedBytes: number;
  elapsedMs: number;
  byCategory: SliceCategoryRow[];
  byExtension: SliceExtensionRow[];
  byResolution?: SliceResolutionRow[];
  byVideoCodec?: SliceVideoCodecRow[];
  bySizeBucket: SliceSizeBucketRow[];
  byYear: SliceYearRow[];
  byDuplicationTier: SliceDuplicationRow[];
  byShelfRack: SliceShelfGroupRow[];
  topDuplicateGroups: SliceDuplicateGroup[];
  topVolumes: SliceVolumeRow[];
}

export interface ScanJobState {
  jobId: string;
  targetLabel: string;
  mountPoint: string;
  physicalId: string;
  shelfTag: string;
  status: 'running' | 'completed' | 'cancelled' | 'failed';
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

export interface AnnotationRecord {
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

export interface SystemSnapshot {
  platform: PlatformPreset;
  sourceMode: 'linked_ctf' | 'sqlite_local';
  linkedCtfPath: string;
  pinnedGenerationSha: string;
  liveSourceSha: string;
  tampered: boolean;
  probeOk: boolean;
  simulatedIoDelayMs: number;
  totalCatalogEntries: number;
  totalIndexedBytes: number;
  totalCapacityBytes: number;
  totalUsedBytes: number;
  volumes: CatalogVolume[];
  osMounts: LiveOsMount[];
  categories: string[];
  latencySummary: SearchLatencySummary;
  annotations: AnnotationRecord[];
  activeScanJob: ScanJobState | null;
  recentScanJobs: ScanJobState[];
  cancelledIoCount: number;
}

export interface CtfInspectResult {
  fileName: string;
  byteLength: number;
  sha256: string;
  headerAscii: string;
  looksLikeWhereIsItCtf: boolean;
  observedVersionMarker: string;
  opaque0x10CountEstimate: number;
  readOnlySafe: true;
  notes: string[];
}
