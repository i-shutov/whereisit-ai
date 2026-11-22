import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { HardDriveDownload, Moon, Sun } from 'lucide-react';
import type { SystemSnapshot } from './types/atlas';
import { LocatorWorkspace } from './components/LocatorWorkspace';
import type { SearchPresetJump } from './components/LocatorWorkspace';
import { ArchiveSlicesView } from './components/ArchiveSlicesView';
import { VolumeSunburstView } from './components/VolumeSunburstView';
import { DisksInventoryView } from './components/DisksInventoryView';
import { LocalDiskAnalyzer } from './components/LocalDiskAnalyzer';
import { DiskPropertiesModal } from './components/DiskPropertiesModal';
import { ScanDiskModal } from './components/ScanDiskModal';
import { formatBytes } from './utils/format';
import { useI18n } from './i18n';

type ActiveTab = 'search' | 'slices' | 'sunburst' | 'disks' | 'local';

export default function App() {
  const { lang, setLang, t } = useI18n();
  const [activeTab, setActiveTab] = useState<ActiveTab>('search');
  const [visitedTabs, setVisitedTabs] = useState<Set<ActiveTab>>(() => new Set(['search']));
  const tabRefs = useRef<Partial<Record<ActiveTab, HTMLButtonElement | null>>>({});
  const [darkMode, setDarkMode] = useState<boolean>(true);
  const [snapshot, setSnapshot] = useState<SystemSnapshot | null>(null);
  const [toast, setToast] = useState<{ message: string; tone: 'info' | 'error' | 'success' } | null>(null);

  // Synchronized scope state across Search, Archive Slices & Volume Sunburst
  const [slicesVolumeId, setSlicesVolumeId] = useState<number | null>(null);
  const [sunburstVolumeId, setSunburstVolumeId] = useState<number | null>(1);
  const [sunburstFolderPath, setSunburstFolderPath] = useState<string | null>(null);
  const [sunburstHighlightQuery, setSunburstHighlightQuery] = useState<string>('');
  const [searchJump, setSearchJump] = useState<SearchPresetJump | null>(null);

  // Desktop modal windows (1:1 mapping to egui::Window)
  const [scanModalOpen, setScanModalOpen] = useState<boolean>(false);
  const [scanTargetVolumeId, setScanTargetVolumeId] = useState<number | null>(null);
  const [propsVolumeId, setPropsVolumeId] = useState<number | null>(null);

  useEffect(() => {
    document.documentElement.classList.toggle('dark', darkMode);
  }, [darkMode]);

  const notify = useCallback((message: string, tone: 'info' | 'error' | 'success' = 'info') => {
    setToast({ message, tone });
  }, []);

  const selectTab = useCallback((tab: ActiveTab) => {
    const focusedElement = document.activeElement;
    if (focusedElement instanceof HTMLElement && focusedElement.closest('[role="tabpanel"]')) {
      tabRefs.current[tab]?.focus();
    }
    setActiveTab(tab);
    setVisitedTabs((visited) => (visited.has(tab) ? visited : new Set(visited).add(tab)));
  }, []);

  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 4000);
    return () => window.clearTimeout(timer);
  }, [toast]);

  const refreshSnapshot = useCallback(async () => {
    const resp = await fetch('/api/state');
    if (resp.ok) {
      setSnapshot((await resp.json()) as SystemSnapshot);
    }
  }, []);

  useEffect(() => {
    void refreshSnapshot();
  }, [refreshSnapshot]);

  // Global shortcut Ctrl+N / Cmd+N -> Open Scan Disk Window
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'n') {
        e.preventDefault();
        setScanTargetVolumeId(null);
        setScanModalOpen(true);
      }
    };
    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, []);

  const openScanModal = useCallback((volumeId: number | null) => {
    setScanTargetVolumeId(volumeId);
    setScanModalOpen(true);
  }, []);

  const openSunburstAt = useCallback(
    (volumeId: number | null, folderPath: string | null, highlightQuery = '') => {
      setSunburstVolumeId(volumeId);
      setSunburstFolderPath(folderPath);
      setSunburstHighlightQuery(highlightQuery);
      selectTab('sunburst');
    },
    [selectTab]
  );

  const openSlicesAt = useCallback((volumeId: number | null) => {
    setSlicesVolumeId(volumeId);
    selectTab('slices');
  }, [selectTab]);

  const jumpToSearch = useCallback(
    (params: { volumeId: number | null; folderPrefix: string | null; query?: string; copySourceEntryId?: number | null }) => {
      setSearchJump({
        nonce: Date.now(),
        volumeId: params.volumeId,
        folderPrefix: params.folderPrefix,
        query: params.query,
        copySourceEntryId: params.copySourceEntryId ?? null,
      });
      selectTab('search');
    },
    [selectTab]
  );

  const activePropsVolume = useMemo(
    () => (propsVolumeId !== null ? snapshot?.volumes.find((v) => v.id === propsVolumeId) ?? null : null),
    [propsVolumeId, snapshot]
  );

  const navItems: { id: ActiveTab; label: string }[] = [
    { id: 'search', label: t('Поиск', 'Search') },
    { id: 'slices', label: t('Разрезы', 'Slices') },
    { id: 'sunburst', label: t('Карта объёма', 'Volume Map') },
    {
      id: 'disks',
      label: snapshot ? `${t('Носители', 'USB Disks')} (${snapshot.volumes.length})` : t('Носители', 'USB Disks'),
    },
    { id: 'local', label: t('Локальный ПК', 'Local PC') },
  ];

  const handleTabKeyDown = (event: React.KeyboardEvent<HTMLButtonElement>, current: ActiveTab) => {
    const currentIndex = navItems.findIndex((item) => item.id === current);
    let nextIndex: number | null = null;
    if (event.key === 'ArrowRight' || event.key === 'ArrowDown') nextIndex = (currentIndex + 1) % navItems.length;
    else if (event.key === 'ArrowLeft' || event.key === 'ArrowUp') nextIndex = (currentIndex - 1 + navItems.length) % navItems.length;
    else if (event.key === 'Home') nextIndex = 0;
    else if (event.key === 'End') nextIndex = navItems.length - 1;
    if (nextIndex === null) return;
    event.preventDefault();
    const nextTab = navItems[nextIndex].id;
    selectTab(nextTab);
    tabRefs.current[nextTab]?.focus();
  };

  return (
    <div className="min-h-screen flex flex-col bg-slate-50 text-slate-900 dark:bg-slate-950 dark:text-slate-100">
      {/* Top Bar (egui::TopBottomPanel::top) */}
      <header className="flex items-center justify-between gap-4 px-5 py-2.5 border-b border-slate-200 dark:border-slate-800 bg-white dark:bg-slate-950">
        <a
          href="#search"
          onClick={(e) => {
            e.preventDefault();
            selectTab('search');
          }}
          className="font-display text-base font-bold tracking-tight text-slate-900 dark:text-white whitespace-nowrap shrink-0"
        >
          WhereIsIt AI
        </a>

        <nav role="tablist" aria-label={t('Рабочие пространства', 'Workspaces')} className="flex items-center gap-5 text-sm font-medium text-slate-600 dark:text-slate-400 overflow-x-auto">
          {navItems.map((item) => (
            <button
              key={item.id}
              ref={(node) => { tabRefs.current[item.id] = node; }}
              type="button"
              role="tab"
              id={`workspace-tab-${item.id}`}
              aria-selected={activeTab === item.id}
              aria-controls={`workspace-panel-${item.id}`}
              tabIndex={activeTab === item.id ? 0 : -1}
              onKeyDown={(event) => handleTabKeyDown(event, item.id)}
              onClick={() => selectTab(item.id)}
              className={`py-1 transition-colors whitespace-nowrap shrink-0 cursor-pointer border-b-2 ${
                activeTab === item.id
                  ? 'border-blue-600 text-slate-900 dark:text-white font-semibold'
                  : 'border-transparent hover:text-slate-900 dark:hover:text-slate-200'
              }`}
            >
              {item.label}
            </button>
          ))}
        </nav>

        <div className="flex items-center shrink-0">
          <button
            type="button"
            onClick={() => openScanModal(null)}
            title={t('Открыть окно сканирования USB-диска (Ctrl+N)', 'Open Scan USB Disk dialog (Ctrl+N)')}
            className="inline-flex items-center gap-1.5 px-3.5 py-1.5 text-xs font-semibold rounded bg-blue-600 text-white hover:bg-blue-500 transition-colors whitespace-nowrap shrink-0 cursor-pointer"
          >
            <HardDriveDownload className="w-3.5 h-3.5" />
            {t('+ Сканировать диск…', '+ Scan Disk…')}
          </button>
        </div>
      </header>

      {/* Quiet Context & Language/Theme Bar */}
      <div className="px-5 py-1.5 border-b border-slate-200/80 dark:border-slate-800/80 bg-slate-100/70 dark:bg-slate-900/40 flex flex-wrap items-center justify-between gap-2 text-xs text-slate-600 dark:text-slate-400">
        <div className="flex flex-wrap items-center gap-2 font-mono tabular-nums">
          {snapshot ? (
            <>
              <span>
                {snapshot.volumes.length} {t('USB-дисков в видеоархиве', 'USB drives in video archive')}
              </span>
              <span aria-hidden="true">·</span>
              <span>
                {t('в индексе', 'indexed')} {formatBytes(snapshot.totalIndexedBytes)} / {formatBytes(snapshot.totalCapacityBytes)}
              </span>
              <span aria-hidden="true" className="hidden md:inline">·</span>
              <span className="hidden md:inline font-sans text-slate-500">
                {activeTab === 'local'
                  ? t('Локальный ПК: оперативный анализ без записи в базу', 'Local PC: ephemeral analysis, zero DB writes')
                  : t('F2 — быстро переименовать диск · Alt+Enter — свойства диска', 'F2 — fast rename disk · Alt+Enter — disk properties')}
              </span>
            </>
          ) : (
            <span>{t('Загрузка каталога…', 'Loading catalog…')}</span>
          )}
        </div>

        <div className="flex items-center gap-2 shrink-0">
          <div
            role="group"
            aria-label={t('Язык интерфейса', 'Interface language')}
            className="inline-flex p-0.5 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 font-mono text-[11px]"
          >
            <button
              type="button"
              onClick={() => setLang('ru')}
              className={`px-2 py-0.5 rounded cursor-pointer whitespace-nowrap ${
                lang === 'ru'
                  ? 'bg-blue-600 text-white font-semibold'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
              }`}
            >
              RU
            </button>
            <button
              type="button"
              onClick={() => setLang('en')}
              className={`px-2 py-0.5 rounded cursor-pointer whitespace-nowrap ${
                lang === 'en'
                  ? 'bg-blue-600 text-white font-semibold'
                  : 'text-slate-600 dark:text-slate-400 hover:text-slate-900 dark:hover:text-white'
              }`}
            >
              EN
            </button>
          </div>

          <button
            type="button"
            onClick={() => setDarkMode((d) => !d)}
            className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded border border-slate-300 dark:border-slate-700 bg-white dark:bg-slate-900 text-slate-700 dark:text-slate-300 hover:bg-slate-50 dark:hover:bg-slate-800 whitespace-nowrap shrink-0 cursor-pointer"
          >
            {darkMode ? <Sun className="w-3.5 h-3.5" /> : <Moon className="w-3.5 h-3.5" />}
            {darkMode ? t('Светлая', 'Light') : t('Тёмная', 'Dark')}
          </button>
        </div>
      </div>

      {/* Central Workspace */}
      <main className="flex-1 w-full max-w-[1440px] mx-auto px-4 sm:px-5 py-3">
        {!snapshot ? (
          <div className="h-96 border border-slate-200 dark:border-slate-800 rounded-md bg-white dark:bg-slate-900/40 flex items-center justify-center text-sm text-slate-500">
            {t('Инициализация видеокаталога…', 'Initializing video catalog…')}
          </div>
        ) : (
          <>
            <div role="tabpanel" id="workspace-panel-search" aria-labelledby="workspace-tab-search" hidden={activeTab !== 'search'}>
              {visitedTabs.has('search') && <LocatorWorkspace
                isActive={activeTab === 'search'}
                snapshot={snapshot}
                presetJump={searchJump}
                onRefreshSnapshot={refreshSnapshot}
                onOpenSunburst={(vid, fpath) => openSunburstAt(vid, fpath)}
                onOpenSlices={openSlicesAt}
                onOpenDiskProperties={(vid) => setPropsVolumeId(vid)}
                onOpenScanModal={openScanModal}
                onNotify={notify}
              />}
            </div>
            <div role="tabpanel" id="workspace-panel-slices" aria-labelledby="workspace-tab-slices" hidden={activeTab !== 'slices'}>
              {visitedTabs.has('slices') && <ArchiveSlicesView
                isActive={activeTab === 'slices'}
                snapshot={snapshot}
                initialVolumeId={slicesVolumeId}
                onJumpToSearch={(params) => jumpToSearch(params)}
                onJumpToCopyCandidates={(entryId) => jumpToSearch({ volumeId: null, folderPrefix: null, copySourceEntryId: entryId })}
                onOpenSunburst={(vid, fpath, hl) => openSunburstAt(vid, fpath, hl)}
                onNotify={notify}
              />}
            </div>
            <div role="tabpanel" id="workspace-panel-sunburst" aria-labelledby="workspace-tab-sunburst" hidden={activeTab !== 'sunburst'}>
              {visitedTabs.has('sunburst') && <VolumeSunburstView
                isActive={activeTab === 'sunburst'}
                snapshot={snapshot}
                selectedVolumeId={sunburstVolumeId}
                selectedFolderPath={sunburstFolderPath}
                initialHighlightQuery={sunburstHighlightQuery}
                onNavigateScope={(vid, fpath) => { setSunburstVolumeId(vid); setSunburstFolderPath(fpath); }}
                onJumpToSearch={(params) => jumpToSearch(params)}
                onJumpToCopyCandidates={(entryId) => jumpToSearch({ volumeId: null, folderPrefix: null, copySourceEntryId: entryId })}
                onOpenSlices={openSlicesAt}
                onNotify={notify}
              />}
            </div>
            <div role="tabpanel" id="workspace-panel-disks" aria-labelledby="workspace-tab-disks" hidden={activeTab !== 'disks'}>
              {visitedTabs.has('disks') && <DisksInventoryView
                snapshot={snapshot}
                onRefreshSnapshot={refreshSnapshot}
                onOpenSunburst={(vid) => openSunburstAt(vid, null)}
                onOpenSlices={openSlicesAt}
                onOpenSearch={(vid) => jumpToSearch({ volumeId: vid, folderPrefix: null })}
                onOpenDiskProperties={(vid) => setPropsVolumeId(vid)}
                onOpenScanModal={openScanModal}
                onNotify={notify}
              />}
            </div>
            <div role="tabpanel" id="workspace-panel-local" aria-labelledby="workspace-tab-local" hidden={activeTab !== 'local'}>
              {visitedTabs.has('local') && <LocalDiskAnalyzer snapshot={snapshot} onNotify={notify} />}
            </div>
          </>
        )}
      </main>

      {/* Modal Window 1: Disk Properties (egui::Window) */}
      <DiskPropertiesModal
        volume={activePropsVolume}
        onClose={() => setPropsVolumeId(null)}
        onSaved={refreshSnapshot}
        onOpenRescan={(vid) => openScanModal(vid)}
        onNotify={notify}
      />

      {/* Modal Window 2: Scan USB Disk + Lightweight WhereIsIt Plugins (egui::Window) */}
      {snapshot && (
        <ScanDiskModal
          open={scanModalOpen}
          snapshot={snapshot}
          initialTargetVolumeId={scanTargetVolumeId}
          onClose={() => setScanModalOpen(false)}
          onRefreshSnapshot={refreshSnapshot}
          onJumpToVolumeSearch={(vid) => jumpToSearch({ volumeId: vid, folderPrefix: null })}
          onNotify={notify}
        />
      )}

      {toast && (
        <div
          role="status"
          className={`fixed bottom-4 right-4 z-50 max-w-md px-4 py-2.5 rounded border shadow-lg text-xs font-medium ${
            toast.tone === 'error'
              ? 'bg-red-950 text-red-100 border-red-800'
              : toast.tone === 'success'
                ? 'bg-slate-900 text-emerald-300 border-slate-700'
                : 'bg-slate-900 text-slate-100 border-slate-700'
          }`}
        >
          {toast.message}
        </div>
      )}
    </div>
  );
}
