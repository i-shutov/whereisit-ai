# WhereIsIt AI — Windows1.0 master plan

Owner-approved22.11.2026: **releaseWindowsx64 stable1.0 unsigned; USBpairing disabled; tabs/sunburst/analyzer later; macOS/Linux releases later.** Keep portableRust and cross-platformcompile/tests. EarlierWin10testing accepted ashistorical owner-confirmedevidence; norecreatedVM required.

The canonical executable plan is [Windows1.0](RELEASE_1_0_WIN11_ACTION_PLAN_2026-11-21.md), with [machinegates](../release/RELEASE_1_0_GATE_MATRIX_20261116.json) and [releaseacceptance](../release/ACCEPTANCE.json). Older all-platformsigning/GUI gates are historical and do not override this ownerscope.

RC4/main751fa545 remainsimmutable. The1.0 candidate usesactualversion1.0.0 and a separateWindowsstablepublisher, exactsource/run/hash validation, generatedRC4upgrade/rollback tests, actualinstalledGUI acceptance, and draftassetredownload/checksumverification beforepublish. No furtherpermission isneeded: publicationisexplicitlyauthorized afterQA.

NativeRust/egui, PR58UXauthority and metadata-only/data-safety contracts remain. CTFsources are read-only; edit/exportonlyderivatives/newfiles. SupportedCTF compatibility is independentlyaccepted inoriginalWhereIsIt; no claimsforarbitrarycompressed/encryptedvariants. No catalogedfileexecution/contenthash/defaultnetwork/cleanup export or metadata-equality/backup guarantee.

Onlyactualin-scopeP0/P1 defects blockWindows1.0. Deferredfeatures/platforms are explicitlyrecorded; missinghardwarepairingtests are not falselymarkedPASS because thatfeature isdisabled. WarmsearchSLO is scoped to no concurrentUIautomation; observedloadedFAILs staydocumented. The finalreleaseverdict ties actualinstaller, GUI anddatasafety evidence to thepublishedsource; it doesnotrequire recruiting a new externalQA person.
