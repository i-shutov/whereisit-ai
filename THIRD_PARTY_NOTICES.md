# Уведомления о сторонних компонентах

Список генерируется из зафиксированных registry-пакетов в Cargo.lock.
Он охватывает зависимости сборки, тестов и всех поддержанных платформ;
не каждая из перечисленных библиотек попадает в каждый бинарный файл.
Лицензионные формулировки получены из опубликованных Cargo.toml;
файлы лицензий/NOTICE скопированы из соответствующих Cargo-пакетов.
При отсутствии отдельного файла в архиве остаётся выражение лицензии
из его манифеста. Наличие файла не заменяет юридического анализа.

SHA-256 Cargo.lock: `c17a634032255bc94e7ff39249c3515b2020ba99922ef6edc58066ff31fbee1f`.

## Встроенный SQLite

rusqlite 0.43.0 + libsqlite3-sys 0.40.0
с включённым bundled используют SQLite 3.54.1.
Проверено по sqlite3.c опубликованного crate, а не по системной SQLite.
В исходнике SQLite авторы отказываются от авторского права.
Исходный заголовок сохранён: [COPYRIGHT.txt](licenses/SQLite-3.54.1/COPYRIGHT.txt).
Исправление WAL-reset включено в SQLite 3.51.3 и позднее.
Версия подтверждается также при сборке и тестах.

## Зафиксированные registry-пакеты

| Пакет | Версия | Лицензия из Cargo.toml | Файлы |
|---|---|---|---|
| ab_glyph | 0.2.32 | `Apache-2.0` | [LICENSE](licenses/ab_glyph-0.2.32/LICENSE) |
| ab_glyph_rasterizer | 0.1.10 | `Apache-2.0` | [LICENSE](licenses/ab_glyph_rasterizer-0.1.10/LICENSE) |
| accesskit | 0.25.2 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| accesskit_atspi_common | 0.21.1 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| accesskit_consumer | 0.39.2 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| accesskit_ios | 0.2.2 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| accesskit_macos | 0.27.2 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| accesskit_unix | 0.24.1 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| accesskit_windows | 0.35.2 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| accesskit_winit | 0.34.2 | `Apache-2.0` | Отдельный файл в архиве отсутствует |
| adler2 | 2.0.1 | `0BSD OR MIT OR Apache-2.0` | [LICENSE-0BSD](licenses/adler2-2.0.1/LICENSE-0BSD), [LICENSE-APACHE](licenses/adler2-2.0.1/LICENSE-APACHE), [LICENSE-MIT](licenses/adler2-2.0.1/LICENSE-MIT) |
| ahash | 0.8.12 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/ahash-0.8.12/LICENSE-APACHE), [LICENSE-MIT](licenses/ahash-0.8.12/LICENSE-MIT) |
| allocator-api2 | 0.2.21 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/allocator-api2-0.2.21/LICENSE-APACHE), [LICENSE-MIT](licenses/allocator-api2-0.2.21/LICENSE-MIT) |
| android-activity | 0.6.1 | `MIT OR Apache-2.0` | [LICENSE](licenses/android-activity-0.6.1/LICENSE), [LICENSE-APACHE](licenses/android-activity-0.6.1/LICENSE-APACHE), [LICENSE-MIT](licenses/android-activity-0.6.1/LICENSE-MIT) |
| android-properties | 0.2.2 | `MIT` | [LICENSE](licenses/android-properties-0.2.2/LICENSE) |
| android_system_properties | 0.1.6 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/android_system_properties-0.1.6/LICENSE-APACHE), [LICENSE-MIT](licenses/android_system_properties-0.1.6/LICENSE-MIT) |
| arboard | 3.7.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE.txt](licenses/arboard-3.7.0/LICENSE-APACHE.txt), [LICENSE-MIT.txt](licenses/arboard-3.7.0/LICENSE-MIT.txt) |
| arrayref | 0.3.9 | `BSD-2-Clause` | [LICENSE](licenses/arrayref-0.3.9/LICENSE) |
| arrayvec | 0.7.8 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/arrayvec-0.7.8/LICENSE-APACHE), [LICENSE-MIT](licenses/arrayvec-0.7.8/LICENSE-MIT) |
| as-raw-xcb-connection | 1.0.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/as-raw-xcb-connection-1.0.1/LICENSE-APACHE), [LICENSE-MIT](licenses/as-raw-xcb-connection-1.0.1/LICENSE-MIT) |
| ash | 0.38.0+1.3.281 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/ash-0.38.0+1.3.281/LICENSE-APACHE), [LICENSE-MIT](licenses/ash-0.38.0+1.3.281/LICENSE-MIT) |
| async-broadcast | 0.7.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/async-broadcast-0.7.2/LICENSE-APACHE), [LICENSE-MIT](licenses/async-broadcast-0.7.2/LICENSE-MIT) |
| async-channel | 2.5.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/async-channel-2.5.0/LICENSE-APACHE), [LICENSE-MIT](licenses/async-channel-2.5.0/LICENSE-MIT) |
| async-executor | 1.14.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/async-executor-1.14.1/LICENSE-APACHE), [LICENSE-MIT](licenses/async-executor-1.14.1/LICENSE-MIT) |
| async-io | 2.6.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/async-io-2.6.0/LICENSE-APACHE), [LICENSE-MIT](licenses/async-io-2.6.0/LICENSE-MIT) |
| async-lock | 3.5.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/async-lock-3.5.0/LICENSE-APACHE), [LICENSE-MIT](licenses/async-lock-3.5.0/LICENSE-MIT) |
| async-process | 2.5.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/async-process-2.5.0/LICENSE-APACHE), [LICENSE-MIT](licenses/async-process-2.5.0/LICENSE-MIT) |
| async-recursion | 1.2.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/async-recursion-1.2.0/LICENSE-APACHE), [LICENSE-MIT](licenses/async-recursion-1.2.0/LICENSE-MIT) |
| async-signal | 0.2.14 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/async-signal-0.2.14/LICENSE-APACHE), [LICENSE-MIT](licenses/async-signal-0.2.14/LICENSE-MIT) |
| async-task | 4.7.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/async-task-4.7.1/LICENSE-APACHE), [LICENSE-MIT](licenses/async-task-4.7.1/LICENSE-MIT) |
| async-trait | 0.1.92 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/async-trait-0.1.92/LICENSE-APACHE), [LICENSE-MIT](licenses/async-trait-0.1.92/LICENSE-MIT) |
| atomic-waker | 1.1.2 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/atomic-waker-1.1.2/LICENSE-APACHE), [LICENSE-MIT](licenses/atomic-waker-1.1.2/LICENSE-MIT), [LICENSE-THIRD-PARTY](licenses/atomic-waker-1.1.2/LICENSE-THIRD-PARTY) |
| atspi | 0.29.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE2.txt](licenses/atspi-0.29.0/LICENSE-APACHE2.txt), [LICENSE-MIT.txt](licenses/atspi-0.29.0/LICENSE-MIT.txt) |
| atspi-common | 0.13.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE2.txt](licenses/atspi-common-0.13.0/LICENSE-APACHE2.txt), [LICENSE-MIT.txt](licenses/atspi-common-0.13.0/LICENSE-MIT.txt) |
| atspi-proxies | 0.13.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE2.txt](licenses/atspi-proxies-0.13.0/LICENSE-APACHE2.txt), [LICENSE-MIT.txt](licenses/atspi-proxies-0.13.0/LICENSE-MIT.txt) |
| autocfg | 1.5.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/autocfg-1.5.1/LICENSE-APACHE), [LICENSE-MIT](licenses/autocfg-1.5.1/LICENSE-MIT) |
| bit-set | 0.10.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/bit-set-0.10.0/LICENSE-APACHE), [LICENSE-MIT](licenses/bit-set-0.10.0/LICENSE-MIT) |
| bit-vec | 0.9.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/bit-vec-0.9.1/LICENSE-APACHE), [LICENSE-MIT](licenses/bit-vec-0.9.1/LICENSE-MIT) |
| bitflags | 1.3.2 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/bitflags-1.3.2/LICENSE-APACHE), [LICENSE-MIT](licenses/bitflags-1.3.2/LICENSE-MIT) |
| bitflags | 2.13.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/bitflags-2.13.2/LICENSE-APACHE), [LICENSE-MIT](licenses/bitflags-2.13.2/LICENSE-MIT) |
| block-buffer | 0.12.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/block-buffer-0.12.1/LICENSE-APACHE), [LICENSE-MIT](licenses/block-buffer-0.12.1/LICENSE-MIT) |
| block2 | 0.5.1 | `MIT` | Отдельный файл в архиве отсутствует |
| block2 | 0.6.2 | `MIT` | Отдельный файл в архиве отсутствует |
| blocking | 1.7.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/blocking-1.7.0/LICENSE-APACHE), [LICENSE-MIT](licenses/blocking-1.7.0/LICENSE-MIT) |
| bumpalo | 3.20.4 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/bumpalo-3.20.4/LICENSE-APACHE), [LICENSE-MIT](licenses/bumpalo-3.20.4/LICENSE-MIT) |
| bytemuck | 1.25.2 | `Zlib OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/bytemuck-1.25.2/LICENSE-APACHE), [LICENSE-MIT](licenses/bytemuck-1.25.2/LICENSE-MIT), [LICENSE-ZLIB](licenses/bytemuck-1.25.2/LICENSE-ZLIB) |
| bytemuck_derive | 1.12.1 | `Zlib OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/bytemuck_derive-1.12.1/LICENSE-APACHE), [LICENSE-MIT](licenses/bytemuck_derive-1.12.1/LICENSE-MIT), [LICENSE-ZLIB](licenses/bytemuck_derive-1.12.1/LICENSE-ZLIB) |
| byteorder-lite | 0.1.0 | `Unlicense OR MIT` | [LICENSE-MIT](licenses/byteorder-lite-0.1.0/LICENSE-MIT), [UNLICENSE](licenses/byteorder-lite-0.1.0/UNLICENSE) |
| bytes | 1.12.1 | `MIT` | [LICENSE](licenses/bytes-1.12.1/LICENSE) |
| calloop | 0.13.0 | `MIT` | [LICENSE.txt](licenses/calloop-0.13.0/LICENSE.txt) |
| calloop | 0.14.5 | `MIT` | [LICENSE.txt](licenses/calloop-0.14.5/LICENSE.txt) |
| calloop-wayland-source | 0.3.0 | `MIT` | [LICENSE.txt](licenses/calloop-wayland-source-0.3.0/LICENSE.txt) |
| calloop-wayland-source | 0.4.1 | `MIT` | [LICENSE.txt](licenses/calloop-wayland-source-0.4.1/LICENSE.txt) |
| cc | 1.11.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/cc-1.11.2/LICENSE-APACHE), [LICENSE-MIT](licenses/cc-1.11.2/LICENSE-MIT) |
| cfg-if | 1.0.5 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/cfg-if-1.0.5/LICENSE-APACHE), [LICENSE-MIT](licenses/cfg-if-1.0.5/LICENSE-MIT) |
| cfg_aliases | 0.2.2 | `MIT` | [LICENSE](licenses/cfg_aliases-0.2.2/LICENSE), [NOTICES.md](licenses/cfg_aliases-0.2.2/NOTICES.md) |
| cgl | 0.3.2 | `MIT / Apache-2.0` | [COPYING](licenses/cgl-0.3.2/COPYING), [LICENSE-APACHE](licenses/cgl-0.3.2/LICENSE-APACHE), [LICENSE-MIT](licenses/cgl-0.3.2/LICENSE-MIT) |
| chrono | 0.4.45 | `MIT OR Apache-2.0` | [LICENSE.txt](licenses/chrono-0.4.45/LICENSE.txt) |
| clipboard-win | 5.4.3 | `BSL-1.0` | Отдельный файл в архиве отсутствует |
| codespan-reporting | 0.13.1 | `Apache-2.0` | [LICENSE](licenses/codespan-reporting-0.13.1/LICENSE) |
| color | 0.3.3 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/color-0.3.3/LICENSE-APACHE), [LICENSE-MIT](licenses/color-0.3.3/LICENSE-MIT) |
| combine | 4.6.9 | `MIT` | [LICENSE](licenses/combine-4.6.9/LICENSE) |
| concurrent-queue | 2.5.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/concurrent-queue-2.5.0/LICENSE-APACHE), [LICENSE-MIT](licenses/concurrent-queue-2.5.0/LICENSE-MIT) |
| const-oid | 0.10.3 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/const-oid-0.10.3/LICENSE-APACHE), [LICENSE-MIT](licenses/const-oid-0.10.3/LICENSE-MIT) |
| const-random | 0.1.18 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/const-random-0.1.18/LICENSE-APACHE), [LICENSE-MIT](licenses/const-random-0.1.18/LICENSE-MIT) |
| const-random-macro | 0.1.16 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/const-random-macro-0.1.16/LICENSE-APACHE), [LICENSE-MIT](licenses/const-random-macro-0.1.16/LICENSE-MIT) |
| core-foundation | 0.9.4 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/core-foundation-0.9.4/LICENSE-APACHE), [LICENSE-MIT](licenses/core-foundation-0.9.4/LICENSE-MIT) |
| core-foundation-sys | 0.8.7 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/core-foundation-sys-0.8.7/LICENSE-APACHE), [LICENSE-MIT](licenses/core-foundation-sys-0.8.7/LICENSE-MIT) |
| core-graphics | 0.23.2 | `MIT OR Apache-2.0` | [COPYRIGHT](licenses/core-graphics-0.23.2/COPYRIGHT), [LICENSE-APACHE](licenses/core-graphics-0.23.2/LICENSE-APACHE), [LICENSE-MIT](licenses/core-graphics-0.23.2/LICENSE-MIT) |
| core-graphics-types | 0.1.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/core-graphics-types-0.1.3/LICENSE-APACHE), [LICENSE-MIT](licenses/core-graphics-types-0.1.3/LICENSE-MIT) |
| cpufeatures | 0.3.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/cpufeatures-0.3.1/LICENSE-APACHE), [LICENSE-MIT](licenses/cpufeatures-0.3.1/LICENSE-MIT) |
| crc32fast | 1.5.5 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/crc32fast-1.5.5/LICENSE-APACHE), [LICENSE-MIT](licenses/crc32fast-1.5.5/LICENSE-MIT) |
| crossbeam-utils | 0.8.23 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/crossbeam-utils-0.8.23/LICENSE-APACHE), [LICENSE-MIT](licenses/crossbeam-utils-0.8.23/LICENSE-MIT) |
| crunchy | 0.2.4 | `MIT` | [LICENSE](licenses/crunchy-0.2.4/LICENSE) |
| crypto-common | 0.2.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/crypto-common-0.2.2/LICENSE-APACHE), [LICENSE-MIT](licenses/crypto-common-0.2.2/LICENSE-MIT) |
| csv | 1.4.0 | `Unlicense/MIT` | [COPYING](licenses/csv-1.4.0/COPYING), [LICENSE-MIT](licenses/csv-1.4.0/LICENSE-MIT), [UNLICENSE](licenses/csv-1.4.0/UNLICENSE) |
| csv-core | 0.1.13 | `Unlicense/MIT` | [COPYING](licenses/csv-core-0.1.13/COPYING), [LICENSE-MIT](licenses/csv-core-0.1.13/LICENSE-MIT), [UNLICENSE](licenses/csv-core-0.1.13/UNLICENSE) |
| cursor-icon | 1.2.0 | `MIT OR Apache-2.0 OR Zlib` | [LICENSE-APACHE](licenses/cursor-icon-1.2.0/LICENSE-APACHE), [LICENSE-MIT](licenses/cursor-icon-1.2.0/LICENSE-MIT), [LICENSE-ZLIB](licenses/cursor-icon-1.2.0/LICENSE-ZLIB) |
| digest | 0.11.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/digest-0.11.3/LICENSE-APACHE), [LICENSE-MIT](licenses/digest-0.11.3/LICENSE-MIT) |
| dispatch | 0.2.0 | `MIT` | Отдельный файл в архиве отсутствует |
| dispatch2 | 0.3.1 | `Zlib OR Apache-2.0 OR MIT` | Отдельный файл в архиве отсутствует |
| displaydoc | 0.2.7 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/displaydoc-0.2.7/LICENSE-APACHE), [LICENSE-MIT](licenses/displaydoc-0.2.7/LICENSE-MIT) |
| dlib | 0.5.3 | `MIT` | [LICENSE.txt](licenses/dlib-0.5.3/LICENSE.txt) |
| document-features | 0.2.12 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/document-features-0.2.12/LICENSE-APACHE), [LICENSE-MIT](licenses/document-features-0.2.12/LICENSE-MIT) |
| downcast-rs | 1.2.1 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/downcast-rs-1.2.1/LICENSE-APACHE), [LICENSE-MIT](licenses/downcast-rs-1.2.1/LICENSE-MIT) |
| dpi | 0.1.2 | `Apache-2.0 AND MIT` | [LICENSE](licenses/dpi-0.1.2/LICENSE), [LICENSE-LIBM-MIT](licenses/dpi-0.1.2/LICENSE-LIBM-MIT) |
| ecolor | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| eframe | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| egui | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| egui-wgpu | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| egui-winit | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| egui_extras | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| egui_glow | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| egui_system_font_provider | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| either | 1.19.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/either-1.19.0/LICENSE-APACHE), [LICENSE-MIT](licenses/either-1.19.0/LICENSE-MIT) |
| emath | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| endi | 1.1.1 | `MIT` | [LICENSE-MIT](licenses/endi-1.1.1/LICENSE-MIT) |
| enum-map | 3.1.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| enum-map-derive | 1.0.1 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| enumflags2 | 0.7.12 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/enumflags2-0.7.12/LICENSE-APACHE), [LICENSE-MIT](licenses/enumflags2-0.7.12/LICENSE-MIT) |
| enumflags2_derive | 0.7.12 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/enumflags2_derive-0.7.12/LICENSE-APACHE), [LICENSE-MIT](licenses/enumflags2_derive-0.7.12/LICENSE-MIT) |
| enumn | 0.1.14 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/enumn-0.1.14/LICENSE-APACHE), [LICENSE-MIT](licenses/enumn-0.1.14/LICENSE-MIT) |
| epaint | 0.37.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| epaint_default_fonts | 0.37.0 | `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0` | Отдельный файл в архиве отсутствует |
| equivalent | 1.0.3 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/equivalent-1.0.3/LICENSE-APACHE), [LICENSE-MIT](licenses/equivalent-1.0.3/LICENSE-MIT) |
| errno | 0.3.15 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/errno-0.3.15/LICENSE-APACHE), [LICENSE-MIT](licenses/errno-0.3.15/LICENSE-MIT) |
| error-code | 3.4.0 | `BSL-1.0` | [LICENSE](licenses/error-code-3.4.0/LICENSE) |
| euclid | 0.22.14 | `MIT OR Apache-2.0` | [COPYRIGHT](licenses/euclid-0.22.14/COPYRIGHT), [LICENSE-APACHE](licenses/euclid-0.22.14/LICENSE-APACHE), [LICENSE-MIT](licenses/euclid-0.22.14/LICENSE-MIT) |
| event-listener | 5.4.2 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/event-listener-5.4.2/LICENSE-APACHE), [LICENSE-MIT](licenses/event-listener-5.4.2/LICENSE-MIT) |
| event-listener-strategy | 0.5.4 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/event-listener-strategy-0.5.4/LICENSE-APACHE), [LICENSE-MIT](licenses/event-listener-strategy-0.5.4/LICENSE-MIT) |
| fallible-iterator | 0.3.0 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/fallible-iterator-0.3.0/LICENSE-APACHE), [LICENSE-MIT](licenses/fallible-iterator-0.3.0/LICENSE-MIT) |
| fallible-streaming-iterator | 0.1.9 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/fallible-streaming-iterator-0.1.9/LICENSE-APACHE), [LICENSE-MIT](licenses/fallible-streaming-iterator-0.1.9/LICENSE-MIT) |
| fastrand | 2.5.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/fastrand-2.5.0/LICENSE-APACHE), [LICENSE-MIT](licenses/fastrand-2.5.0/LICENSE-MIT) |
| fax | 0.2.7 | `MIT` | [LICENSE](licenses/fax-0.2.7/LICENSE) |
| fdeflate | 0.3.7 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/fdeflate-0.3.7/LICENSE-APACHE), [LICENSE-MIT](licenses/fdeflate-0.3.7/LICENSE-MIT) |
| fearless_simd | 0.4.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/fearless_simd-0.4.1/LICENSE-APACHE), [LICENSE-MIT](licenses/fearless_simd-0.4.1/LICENSE-MIT) |
| find-msvc-tools | 0.1.15 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/find-msvc-tools-0.1.15/LICENSE-APACHE), [LICENSE-MIT](licenses/find-msvc-tools-0.1.15/LICENSE-MIT) |
| fixedbitset | 0.5.9 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/fixedbitset-0.5.9/LICENSE-APACHE), [LICENSE-MIT](licenses/fixedbitset-0.5.9/LICENSE-MIT) |
| flate2 | 1.1.11 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/flate2-1.1.11/LICENSE-APACHE), [LICENSE-MIT](licenses/flate2-1.1.11/LICENSE-MIT) |
| foldhash | 0.1.5 | `Zlib` | [LICENSE](licenses/foldhash-0.1.5/LICENSE) |
| foldhash | 0.2.0 | `Zlib` | [LICENSE](licenses/foldhash-0.2.0/LICENSE) |
| font-types | 0.12.7 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/font-types-0.12.7/LICENSE-APACHE), [LICENSE-MIT](licenses/font-types-0.12.7/LICENSE-MIT) |
| fontique | 0.11.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/fontique-0.11.1/LICENSE-APACHE), [LICENSE-MIT](licenses/fontique-0.11.1/LICENSE-MIT) |
| foreign-types | 0.5.0 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/foreign-types-0.5.0/LICENSE-APACHE), [LICENSE-MIT](licenses/foreign-types-0.5.0/LICENSE-MIT) |
| foreign-types-macros | 0.2.4 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/foreign-types-macros-0.2.4/LICENSE-APACHE), [LICENSE-MIT](licenses/foreign-types-macros-0.2.4/LICENSE-MIT) |
| foreign-types-shared | 0.3.1 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/foreign-types-shared-0.3.1/LICENSE-APACHE), [LICENSE-MIT](licenses/foreign-types-shared-0.3.1/LICENSE-MIT) |
| form_urlencoded | 1.2.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/form_urlencoded-1.2.2/LICENSE-APACHE), [LICENSE-MIT](licenses/form_urlencoded-1.2.2/LICENSE-MIT) |
| futures-core | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/futures-core-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/futures-core-0.3.34/LICENSE-MIT) |
| futures-io | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/futures-io-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/futures-io-0.3.34/LICENSE-MIT) |
| futures-lite | 2.6.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/futures-lite-2.6.1/LICENSE-APACHE), [LICENSE-MIT](licenses/futures-lite-2.6.1/LICENSE-MIT), [LICENSE-THIRD-PARTY](licenses/futures-lite-2.6.1/LICENSE-THIRD-PARTY) |
| futures-macro | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/futures-macro-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/futures-macro-0.3.34/LICENSE-MIT) |
| futures-task | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/futures-task-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/futures-task-0.3.34/LICENSE-MIT) |
| futures-util | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/futures-util-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/futures-util-0.3.34/LICENSE-MIT) |
| gethostname | 1.1.0 | `Apache-2.0` | [LICENSE](licenses/gethostname-1.1.0/LICENSE) |
| getrandom | 0.2.17 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/getrandom-0.2.17/LICENSE-APACHE), [LICENSE-MIT](licenses/getrandom-0.2.17/LICENSE-MIT) |
| getrandom | 0.3.4 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/getrandom-0.3.4/LICENSE-APACHE), [LICENSE-MIT](licenses/getrandom-0.3.4/LICENSE-MIT) |
| getrandom | 0.4.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/getrandom-0.4.3/LICENSE-APACHE), [LICENSE-MIT](licenses/getrandom-0.4.3/LICENSE-MIT) |
| gl_generator | 0.14.0 | `Apache-2.0` | Отдельный файл в архиве отсутствует |
| glifo | 0.3.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/glifo-0.3.0/LICENSE-APACHE), [LICENSE-MIT](licenses/glifo-0.3.0/LICENSE-MIT) |
| glow | 0.17.0 | `MIT OR Apache-2.0 OR Zlib` | [LICENSE-APACHE](licenses/glow-0.17.0/LICENSE-APACHE), [LICENSE-MIT](licenses/glow-0.17.0/LICENSE-MIT), [LICENSE-ZLIB](licenses/glow-0.17.0/LICENSE-ZLIB) |
| glutin | 0.32.3 | `Apache-2.0` | [LICENSE](licenses/glutin-0.32.3/LICENSE) |
| glutin-winit | 0.5.0 | `MIT` | [LICENSE](licenses/glutin-winit-0.5.0/LICENSE) |
| glutin_egl_sys | 0.7.1 | `Apache-2.0` | [LICENSE](licenses/glutin_egl_sys-0.7.1/LICENSE) |
| glutin_glx_sys | 0.6.1 | `Apache-2.0` | [LICENSE](licenses/glutin_glx_sys-0.6.1/LICENSE) |
| glutin_wgl_sys | 0.6.1 | `Apache-2.0` | [LICENSE](licenses/glutin_wgl_sys-0.6.1/LICENSE) |
| gpu-allocator | 0.28.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/gpu-allocator-0.28.0/LICENSE-APACHE), [LICENSE-MIT](licenses/gpu-allocator-0.28.0/LICENSE-MIT) |
| guillotiere | 0.7.0 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/guillotiere-0.7.0/LICENSE-APACHE), [LICENSE-MIT](licenses/guillotiere-0.7.0/LICENSE-MIT) |
| half | 2.7.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/half-2.7.1/LICENSE-APACHE), [LICENSE-MIT](licenses/half-2.7.1/LICENSE-MIT) |
| harfrust | 0.12.0 | `MIT` | [LICENSE](licenses/harfrust-0.12.0/LICENSE) |
| hashbrown | 0.15.5 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hashbrown-0.15.5/LICENSE-APACHE), [LICENSE-MIT](licenses/hashbrown-0.15.5/LICENSE-MIT) |
| hashbrown | 0.16.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hashbrown-0.16.1/LICENSE-APACHE), [LICENSE-MIT](licenses/hashbrown-0.16.1/LICENSE-MIT) |
| hashbrown | 0.17.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hashbrown-0.17.1/LICENSE-APACHE), [LICENSE-MIT](licenses/hashbrown-0.17.1/LICENSE-MIT) |
| hashlink | 0.12.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hashlink-0.12.2/LICENSE-APACHE), [LICENSE-MIT](licenses/hashlink-0.12.2/LICENSE-MIT) |
| hermit-abi | 0.5.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hermit-abi-0.5.3/LICENSE-APACHE), [LICENSE-MIT](licenses/hermit-abi-0.5.3/LICENSE-MIT) |
| hex | 0.4.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hex-0.4.3/LICENSE-APACHE), [LICENSE-MIT](licenses/hex-0.4.3/LICENSE-MIT) |
| hybrid-array | 0.4.15 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hybrid-array-0.4.15/LICENSE-APACHE), [LICENSE-MIT](licenses/hybrid-array-0.4.15/LICENSE-MIT) |
| iana-time-zone | 0.1.65 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/iana-time-zone-0.1.65/LICENSE-APACHE), [LICENSE-MIT](licenses/iana-time-zone-0.1.65/LICENSE-MIT) |
| iana-time-zone-haiku | 0.1.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/iana-time-zone-haiku-0.1.2/LICENSE-APACHE), [LICENSE-MIT](licenses/iana-time-zone-haiku-0.1.2/LICENSE-MIT) |
| icu_collections | 2.4.0 | `Unicode-3.0` | [LICENSE](licenses/icu_collections-2.4.0/LICENSE) |
| icu_locale_core | 2.4.0 | `Unicode-3.0` | [LICENSE](licenses/icu_locale_core-2.4.0/LICENSE) |
| icu_normalizer | 2.4.0 | `Unicode-3.0` | [LICENSE](licenses/icu_normalizer-2.4.0/LICENSE) |
| icu_normalizer_data | 2.4.0 | `Unicode-3.0` | [LICENSE](licenses/icu_normalizer_data-2.4.0/LICENSE) |
| icu_properties | 2.4.0 | `Unicode-3.0` | [LICENSE](licenses/icu_properties-2.4.0/LICENSE) |
| icu_properties_data | 2.4.0 | `Unicode-3.0` | [LICENSE](licenses/icu_properties_data-2.4.0/LICENSE) |
| icu_provider | 2.4.0 | `Unicode-3.0` | [LICENSE](licenses/icu_provider-2.4.0/LICENSE) |
| idna | 1.1.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/idna-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/idna-1.1.0/LICENSE-MIT) |
| idna_adapter | 1.2.2 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/idna_adapter-1.2.2/LICENSE-APACHE), [LICENSE-MIT](licenses/idna_adapter-1.2.2/LICENSE-MIT) |
| image | 0.25.10 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/image-0.25.10/LICENSE-APACHE), [LICENSE-MIT](licenses/image-0.25.10/LICENSE-MIT) |
| indexmap | 2.14.2 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/indexmap-2.14.2/LICENSE-APACHE), [LICENSE-MIT](licenses/indexmap-2.14.2/LICENSE-MIT) |
| itertools | 0.15.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/itertools-0.15.0/LICENSE-APACHE), [LICENSE-MIT](licenses/itertools-0.15.0/LICENSE-MIT) |
| itoa | 1.0.19 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/itoa-1.0.19/LICENSE-APACHE), [LICENSE-MIT](licenses/itoa-1.0.19/LICENSE-MIT) |
| jni | 0.22.4 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| jni-macros | 0.22.4 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| jni-sys | 0.3.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/jni-sys-0.3.1/LICENSE-APACHE), [LICENSE-MIT](licenses/jni-sys-0.3.1/LICENSE-MIT) |
| jni-sys | 0.4.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/jni-sys-0.4.1/LICENSE-APACHE), [LICENSE-MIT](licenses/jni-sys-0.4.1/LICENSE-MIT) |
| jni-sys-macros | 0.4.1 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| jobserver | 0.1.35 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/jobserver-0.1.35/LICENSE-APACHE), [LICENSE-MIT](licenses/jobserver-0.1.35/LICENSE-MIT) |
| js-sys | 0.3.107 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/js-sys-0.3.107/LICENSE-APACHE), [LICENSE-MIT](licenses/js-sys-0.3.107/LICENSE-MIT) |
| khronos-egl | 6.0.0 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/khronos-egl-6.0.0/LICENSE-APACHE), [LICENSE-MIT](licenses/khronos-egl-6.0.0/LICENSE-MIT) |
| khronos_api | 3.1.0 | `Apache-2.0` | Отдельный файл в архиве отсутствует |
| kurbo | 0.13.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/kurbo-0.13.1/LICENSE-APACHE), [LICENSE-MIT](licenses/kurbo-0.13.1/LICENSE-MIT) |
| libc | 0.2.191 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/libc-0.2.191/LICENSE-APACHE), [LICENSE-MIT](licenses/libc-0.2.191/LICENSE-MIT) |
| libloading | 0.8.9 | `ISC` | [LICENSE](licenses/libloading-0.8.9/LICENSE) |
| libm | 0.2.16 | `MIT` | [LICENSE.txt](licenses/libm-0.2.16/LICENSE.txt) |
| libredox | 0.1.25 | `MIT` | [LICENSE](licenses/libredox-0.1.25/LICENSE) |
| libsqlite3-sys | 0.40.0 | `MIT` | [LICENSE](licenses/libsqlite3-sys-0.40.0/LICENSE) |
| linebender_resource_handle | 0.1.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/linebender_resource_handle-0.1.1/LICENSE-APACHE), [LICENSE-MIT](licenses/linebender_resource_handle-0.1.1/LICENSE-MIT) |
| linux-raw-sys | 0.12.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [COPYRIGHT](licenses/linux-raw-sys-0.12.1/COPYRIGHT), [LICENSE-APACHE](licenses/linux-raw-sys-0.12.1/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/linux-raw-sys-0.12.1/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/linux-raw-sys-0.12.1/LICENSE-MIT) |
| linux-raw-sys | 0.4.15 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [COPYRIGHT](licenses/linux-raw-sys-0.4.15/COPYRIGHT), [LICENSE-APACHE](licenses/linux-raw-sys-0.4.15/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/linux-raw-sys-0.4.15/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/linux-raw-sys-0.4.15/LICENSE-MIT) |
| litemap | 0.8.3 | `Unicode-3.0` | [LICENSE](licenses/litemap-0.8.3/LICENSE) |
| litrs | 1.0.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/litrs-1.0.1/LICENSE-APACHE), [LICENSE-MIT](licenses/litrs-1.0.1/LICENSE-MIT) |
| lock_api | 0.4.14 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/lock_api-0.4.14/LICENSE-APACHE), [LICENSE-MIT](licenses/lock_api-0.4.14/LICENSE-MIT) |
| log | 0.4.35 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/log-0.4.35/LICENSE-APACHE), [LICENSE-MIT](licenses/log-0.4.35/LICENSE-MIT) |
| memchr | 2.8.3 | `Unlicense OR MIT` | [COPYING](licenses/memchr-2.8.3/COPYING), [LICENSE-MIT](licenses/memchr-2.8.3/LICENSE-MIT), [UNLICENSE](licenses/memchr-2.8.3/UNLICENSE) |
| memmap2 | 0.9.12 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/memmap2-0.9.12/LICENSE-APACHE), [LICENSE-MIT](licenses/memmap2-0.9.12/LICENSE-MIT) |
| memoffset | 0.9.1 | `MIT` | [LICENSE](licenses/memoffset-0.9.1/LICENSE) |
| miniz_oxide | 0.8.9 | `MIT OR Zlib OR Apache-2.0` | [LICENSE](licenses/miniz_oxide-0.8.9/LICENSE), [LICENSE-APACHE.md](licenses/miniz_oxide-0.8.9/LICENSE-APACHE.md), [LICENSE-MIT.md](licenses/miniz_oxide-0.8.9/LICENSE-MIT.md), [LICENSE-ZLIB.md](licenses/miniz_oxide-0.8.9/LICENSE-ZLIB.md) |
| miniz_oxide | 0.9.1 | `MIT OR Zlib OR Apache-2.0` | [LICENSE](licenses/miniz_oxide-0.9.1/LICENSE), [LICENSE-APACHE.md](licenses/miniz_oxide-0.9.1/LICENSE-APACHE.md), [LICENSE-MIT.md](licenses/miniz_oxide-0.9.1/LICENSE-MIT.md), [LICENSE-ZLIB.md](licenses/miniz_oxide-0.9.1/LICENSE-ZLIB.md) |
| moxcms | 0.8.1 | `BSD-3-Clause OR Apache-2.0` | [LICENSE-APACHE.md](licenses/moxcms-0.8.1/LICENSE-APACHE.md), [LICENSE.md](licenses/moxcms-0.8.1/LICENSE.md) |
| naga | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/naga-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/naga-30.0.2/LICENSE.MIT) |
| naga-types | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/naga-types-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/naga-types-30.0.2/LICENSE.MIT) |
| ndk | 0.9.0 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| ndk-context | 0.1.1 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| ndk-sys | 0.6.0+11769913 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| nohash-hasher | 0.2.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/nohash-hasher-0.2.0/LICENSE-APACHE), [LICENSE-MIT](licenses/nohash-hasher-0.2.0/LICENSE-MIT) |
| nom | 8.0.0 | `MIT` | [LICENSE](licenses/nom-8.0.0/LICENSE) |
| num-bigint | 0.5.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-bigint-0.5.1/LICENSE-APACHE), [LICENSE-MIT](licenses/num-bigint-0.5.1/LICENSE-MIT) |
| num-integer | 0.1.47 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-integer-0.1.47/LICENSE-APACHE), [LICENSE-MIT](licenses/num-integer-0.1.47/LICENSE-MIT) |
| num-traits | 0.2.19 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-traits-0.2.19/LICENSE-APACHE), [LICENSE-MIT](licenses/num-traits-0.2.19/LICENSE-MIT) |
| num_enum | 0.7.6 | `BSD-3-Clause OR MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num_enum-0.7.6/LICENSE-APACHE), [LICENSE-BSD](licenses/num_enum-0.7.6/LICENSE-BSD), [LICENSE-MIT](licenses/num_enum-0.7.6/LICENSE-MIT) |
| num_enum_derive | 0.7.6 | `BSD-3-Clause OR MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num_enum_derive-0.7.6/LICENSE-APACHE), [LICENSE-BSD](licenses/num_enum_derive-0.7.6/LICENSE-BSD), [LICENSE-MIT](licenses/num_enum_derive-0.7.6/LICENSE-MIT) |
| objc-sys | 0.3.5 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2 | 0.5.3 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2 | 0.6.5 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-app-kit | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-app-kit | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` | Отдельный файл в архиве отсутствует |
| objc2-cloud-kit | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-contacts | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-core-data | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-core-foundation | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` | Отдельный файл в архиве отсутствует |
| objc2-core-graphics | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` | Отдельный файл в архиве отсутствует |
| objc2-core-image | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-core-location | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-core-text | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` | Отдельный файл в архиве отсутствует |
| objc2-encode | 4.1.0 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-foundation | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-foundation | 0.3.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-io-surface | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` | Отдельный файл в архиве отсутствует |
| objc2-link-presentation | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-metal | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-metal | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` | Отдельный файл в архиве отсутствует |
| objc2-quartz-core | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-quartz-core | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` | Отдельный файл в архиве отсутствует |
| objc2-symbols | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-ui-kit | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-ui-kit | 0.3.2 | `Zlib OR Apache-2.0 OR MIT` | Отдельный файл в архиве отсутствует |
| objc2-uniform-type-identifiers | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| objc2-user-notifications | 0.2.2 | `MIT` | Отдельный файл в архиве отсутствует |
| once_cell | 1.21.4 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/once_cell-1.21.4/LICENSE-APACHE), [LICENSE-MIT](licenses/once_cell-1.21.4/LICENSE-MIT) |
| orbclient | 0.3.55 | `MIT` | [LICENSE](licenses/orbclient-0.3.55/LICENSE) |
| ordered-float | 5.5.1 | `MIT` | [LICENSE-MIT](licenses/ordered-float-5.5.1/LICENSE-MIT) |
| ordered-stream | 0.2.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/ordered-stream-0.2.0/LICENSE-APACHE), [LICENSE-MIT](licenses/ordered-stream-0.2.0/LICENSE-MIT) |
| os_pipe | 1.2.3 | `MIT` | [LICENSE](licenses/os_pipe-1.2.3/LICENSE) |
| owned_ttf_parser | 0.25.1 | `Apache-2.0` | [LICENSE](licenses/owned_ttf_parser-0.25.1/LICENSE) |
| parking | 2.2.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/parking-2.2.1/LICENSE-APACHE), [LICENSE-MIT](licenses/parking-2.2.1/LICENSE-MIT), [LICENSE-THIRD-PARTY](licenses/parking-2.2.1/LICENSE-THIRD-PARTY) |
| parking_lot | 0.12.5 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/parking_lot-0.12.5/LICENSE-APACHE), [LICENSE-MIT](licenses/parking_lot-0.12.5/LICENSE-MIT) |
| parking_lot_core | 0.9.12 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/parking_lot_core-0.9.12/LICENSE-APACHE), [LICENSE-MIT](licenses/parking_lot_core-0.9.12/LICENSE-MIT) |
| parlance | 0.1.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/parlance-0.1.1/LICENSE-APACHE), [LICENSE-MIT](licenses/parlance-0.1.1/LICENSE-MIT) |
| parquet | 60.1.0 | `Apache-2.0` | [LICENSE.txt](licenses/parquet-60.1.0/LICENSE.txt), [NOTICE.txt](licenses/parquet-60.1.0/NOTICE.txt) |
| peniko | 0.6.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/peniko-0.6.1/LICENSE-APACHE), [LICENSE-MIT](licenses/peniko-0.6.1/LICENSE-MIT) |
| percent-encoding | 2.3.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/percent-encoding-2.3.2/LICENSE-APACHE), [LICENSE-MIT](licenses/percent-encoding-2.3.2/LICENSE-MIT) |
| petgraph | 0.8.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/petgraph-0.8.3/LICENSE-APACHE), [LICENSE-MIT](licenses/petgraph-0.8.3/LICENSE-MIT) |
| phf | 0.14.0 | `MIT` | [LICENSE](licenses/phf-0.14.0/LICENSE) |
| phf_generator | 0.14.0 | `MIT` | [LICENSE](licenses/phf_generator-0.14.0/LICENSE) |
| phf_macros | 0.14.0 | `MIT` | [LICENSE](licenses/phf_macros-0.14.0/LICENSE) |
| phf_shared | 0.14.0 | `MIT` | [LICENSE](licenses/phf_shared-0.14.0/LICENSE) |
| pin-project | 1.1.13 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/pin-project-1.1.13/LICENSE-APACHE), [LICENSE-MIT](licenses/pin-project-1.1.13/LICENSE-MIT) |
| pin-project-internal | 1.1.13 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/pin-project-internal-1.1.13/LICENSE-APACHE), [LICENSE-MIT](licenses/pin-project-internal-1.1.13/LICENSE-MIT) |
| pin-project-lite | 0.2.17 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/pin-project-lite-0.2.17/LICENSE-APACHE), [LICENSE-MIT](licenses/pin-project-lite-0.2.17/LICENSE-MIT) |
| piper | 0.2.5 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/piper-0.2.5/LICENSE-APACHE), [LICENSE-MIT](licenses/piper-0.2.5/LICENSE-MIT) |
| pkg-config | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/pkg-config-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/pkg-config-0.3.34/LICENSE-MIT) |
| plain | 0.2.3 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/plain-0.2.3/LICENSE-APACHE), [LICENSE-MIT](licenses/plain-0.2.3/LICENSE-MIT) |
| png | 0.18.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/png-0.18.1/LICENSE-APACHE), [LICENSE-MIT](licenses/png-0.18.1/LICENSE-MIT) |
| poll-promise | 0.3.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/poll-promise-0.3.0/LICENSE-APACHE), [LICENSE-MIT](licenses/poll-promise-0.3.0/LICENSE-MIT) |
| polling | 3.11.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/polling-3.11.0/LICENSE-APACHE), [LICENSE-MIT](licenses/polling-3.11.0/LICENSE-MIT) |
| pollster | 1.0.1 | `Apache-2.0/MIT` | [LICENSE-APACHE](licenses/pollster-1.0.1/LICENSE-APACHE), [LICENSE-MIT](licenses/pollster-1.0.1/LICENSE-MIT) |
| polycool | 0.4.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/polycool-0.4.0/LICENSE-APACHE), [LICENSE-MIT](licenses/polycool-0.4.0/LICENSE-MIT) |
| portable-atomic | 1.15.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/portable-atomic-1.15.0/LICENSE-APACHE), [LICENSE-MIT](licenses/portable-atomic-1.15.0/LICENSE-MIT) |
| portable-atomic-util | 0.2.8 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/portable-atomic-util-0.2.8/LICENSE-APACHE), [LICENSE-MIT](licenses/portable-atomic-util-0.2.8/LICENSE-MIT) |
| potential_utf | 0.1.6 | `Unicode-3.0` | [LICENSE](licenses/potential_utf-0.1.6/LICENSE) |
| presser | 0.3.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/presser-0.3.1/LICENSE-APACHE), [LICENSE-MIT](licenses/presser-0.3.1/LICENSE-MIT) |
| proc-macro-crate | 3.5.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/proc-macro-crate-3.5.0/LICENSE-APACHE), [LICENSE-MIT](licenses/proc-macro-crate-3.5.0/LICENSE-MIT) |
| proc-macro2 | 1.0.107 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/proc-macro2-1.0.107/LICENSE-APACHE), [LICENSE-MIT](licenses/proc-macro2-1.0.107/LICENSE-MIT) |
| profiling | 1.0.18 | `MIT OR Apache-2.0` | Отдельный файл в архиве отсутствует |
| pxfm | 0.1.30 | `BSD-3-Clause OR Apache-2.0` | [LICENSE-APACHE.md](licenses/pxfm-0.1.30/LICENSE-APACHE.md), [LICENSE.md](licenses/pxfm-0.1.30/LICENSE.md) |
| quick-error | 2.0.1 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/quick-error-2.0.1/LICENSE-APACHE), [LICENSE-MIT](licenses/quick-error-2.0.1/LICENSE-MIT) |
| quick-xml | 0.41.0 | `MIT` | [LICENSE-MIT.md](licenses/quick-xml-0.41.0/LICENSE-MIT.md) |
| quote | 1.0.47 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/quote-1.0.47/LICENSE-APACHE), [LICENSE-MIT](licenses/quote-1.0.47/LICENSE-MIT) |
| r-efi | 5.3.0 | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | Отдельный файл в архиве отсутствует |
| r-efi | 6.0.0 | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | Отдельный файл в архиве отсутствует |
| range-alloc | 0.1.5 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/range-alloc-0.1.5/LICENSE.APACHE), [LICENSE.MIT](licenses/range-alloc-0.1.5/LICENSE.MIT) |
| raw-window-handle | 0.6.2 | `MIT OR Apache-2.0 OR Zlib` | [LICENSE-APACHE.md](licenses/raw-window-handle-0.6.2/LICENSE-APACHE.md), [LICENSE-MIT.md](licenses/raw-window-handle-0.6.2/LICENSE-MIT.md), [LICENSE-ZLIB.md](licenses/raw-window-handle-0.6.2/LICENSE-ZLIB.md) |
| raw-window-metal | 1.1.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/raw-window-metal-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/raw-window-metal-1.1.0/LICENSE-MIT) |
| read-fonts | 0.41.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/read-fonts-0.41.0/LICENSE-APACHE), [LICENSE-MIT](licenses/read-fonts-0.41.0/LICENSE-MIT) |
| redox_syscall | 0.4.1 | `MIT` | [LICENSE](licenses/redox_syscall-0.4.1/LICENSE) |
| redox_syscall | 0.5.18 | `MIT` | [LICENSE](licenses/redox_syscall-0.5.18/LICENSE) |
| redox_syscall | 0.9.5 | `MIT` | [LICENSE](licenses/redox_syscall-0.9.5/LICENSE) |
| renderdoc-sys | 1.1.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/renderdoc-sys-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/renderdoc-sys-1.1.0/LICENSE-MIT) |
| ron | 0.12.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/ron-0.12.3/LICENSE-APACHE), [LICENSE-MIT](licenses/ron-0.12.3/LICENSE-MIT) |
| roxmltree | 0.21.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/roxmltree-0.21.2/LICENSE-APACHE), [LICENSE-MIT](licenses/roxmltree-0.21.2/LICENSE-MIT) |
| rsqlite-vfs | 0.2.0 | `MIT` | Отдельный файл в архиве отсутствует |
| rusqlite | 0.43.0 | `MIT` | [LICENSE](licenses/rusqlite-0.43.0/LICENSE) |
| rustc-hash | 1.1.0 | `Apache-2.0/MIT` | [LICENSE-APACHE](licenses/rustc-hash-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/rustc-hash-1.1.0/LICENSE-MIT) |
| rustc-hash | 2.1.3 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/rustc-hash-2.1.3/LICENSE-APACHE), [LICENSE-MIT](licenses/rustc-hash-2.1.3/LICENSE-MIT) |
| rustc_version | 0.4.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/rustc_version-0.4.1/LICENSE-APACHE), [LICENSE-MIT](licenses/rustc_version-0.4.1/LICENSE-MIT) |
| rustix | 0.38.44 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [COPYRIGHT](licenses/rustix-0.38.44/COPYRIGHT), [LICENSE-APACHE](licenses/rustix-0.38.44/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/rustix-0.38.44/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/rustix-0.38.44/LICENSE-MIT) |
| rustix | 1.1.6 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [COPYRIGHT](licenses/rustix-1.1.6/COPYRIGHT), [LICENSE-APACHE](licenses/rustix-1.1.6/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/rustix-1.1.6/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/rustix-1.1.6/LICENSE-MIT) |
| rustversion | 1.0.23 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/rustversion-1.0.23/LICENSE-APACHE), [LICENSE-MIT](licenses/rustversion-1.0.23/LICENSE-MIT) |
| ryu | 1.0.23 | `Apache-2.0 OR BSL-1.0` | [LICENSE-APACHE](licenses/ryu-1.0.23/LICENSE-APACHE), [LICENSE-BOOST](licenses/ryu-1.0.23/LICENSE-BOOST) |
| same-file | 1.0.6 | `Unlicense/MIT` | [COPYING](licenses/same-file-1.0.6/COPYING), [LICENSE-MIT](licenses/same-file-1.0.6/LICENSE-MIT), [UNLICENSE](licenses/same-file-1.0.6/UNLICENSE) |
| scoped-tls | 1.0.1 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/scoped-tls-1.0.1/LICENSE-APACHE), [LICENSE-MIT](licenses/scoped-tls-1.0.1/LICENSE-MIT) |
| scopeguard | 1.2.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/scopeguard-1.2.0/LICENSE-APACHE), [LICENSE-MIT](licenses/scopeguard-1.2.0/LICENSE-MIT) |
| sctk-adwaita | 0.10.1 | `MIT` | [LICENSE](licenses/sctk-adwaita-0.10.1/LICENSE) |
| self_cell | 1.3.0 | `Apache-2.0 OR GPL-2.0-only` | [LICENSE-APACHE](licenses/self_cell-1.3.0/LICENSE-APACHE), [LICENSE-GPLv2](licenses/self_cell-1.3.0/LICENSE-GPLv2) |
| semver | 1.0.28 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/semver-1.0.28/LICENSE-APACHE), [LICENSE-MIT](licenses/semver-1.0.28/LICENSE-MIT) |
| send_wrapper | 0.6.0 | `MIT/Apache-2.0` | [LICENSE-APACHE.txt](licenses/send_wrapper-0.6.0/LICENSE-APACHE.txt), [LICENSE-MIT.txt](licenses/send_wrapper-0.6.0/LICENSE-MIT.txt) |
| seq-macro | 0.3.6 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/seq-macro-0.3.6/LICENSE-APACHE), [LICENSE-MIT](licenses/seq-macro-0.3.6/LICENSE-MIT) |
| serde | 1.0.229 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/serde-1.0.229/LICENSE-APACHE), [LICENSE-MIT](licenses/serde-1.0.229/LICENSE-MIT) |
| serde_core | 1.0.229 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/serde_core-1.0.229/LICENSE-APACHE), [LICENSE-MIT](licenses/serde_core-1.0.229/LICENSE-MIT) |
| serde_derive | 1.0.229 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/serde_derive-1.0.229/LICENSE-APACHE), [LICENSE-MIT](licenses/serde_derive-1.0.229/LICENSE-MIT) |
| serde_json | 1.0.154 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/serde_json-1.0.154/LICENSE-APACHE), [LICENSE-MIT](licenses/serde_json-1.0.154/LICENSE-MIT) |
| serde_repr | 0.1.21 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/serde_repr-0.1.21/LICENSE-APACHE), [LICENSE-MIT](licenses/serde_repr-0.1.21/LICENSE-MIT) |
| sha2 | 0.11.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/sha2-0.11.1/LICENSE-APACHE), [LICENSE-MIT](licenses/sha2-0.11.1/LICENSE-MIT) |
| shlex | 2.0.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/shlex-2.0.1/LICENSE-APACHE), [LICENSE-MIT](licenses/shlex-2.0.1/LICENSE-MIT) |
| signal-hook-registry | 1.4.8 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/signal-hook-registry-1.4.8/LICENSE-APACHE), [LICENSE-MIT](licenses/signal-hook-registry-1.4.8/LICENSE-MIT) |
| simd-adler32 | 0.3.10 | `MIT` | [LICENSE.md](licenses/simd-adler32-0.3.10/LICENSE.md) |
| simd_cesu8 | 1.2.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/simd_cesu8-1.2.0/LICENSE-APACHE), [LICENSE-MIT](licenses/simd_cesu8-1.2.0/LICENSE-MIT) |
| simdutf8 | 0.1.5 | `MIT OR Apache-2.0` | [LICENSE-Apache](licenses/simdutf8-0.1.5/LICENSE-Apache), [LICENSE-MIT](licenses/simdutf8-0.1.5/LICENSE-MIT) |
| siphasher | 1.0.4 | `MIT OR Apache-2.0` | [COPYING](licenses/siphasher-1.0.4/COPYING), [LICENSE-APACHE](licenses/siphasher-1.0.4/LICENSE-APACHE), [LICENSE-MIT](licenses/siphasher-1.0.4/LICENSE-MIT) |
| skrifa | 0.44.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/skrifa-0.44.0/LICENSE-APACHE), [LICENSE-MIT](licenses/skrifa-0.44.0/LICENSE-MIT) |
| slab | 0.4.12 | `MIT` | [LICENSE](licenses/slab-0.4.12/LICENSE) |
| slotmap | 1.1.1 | `Zlib` | [LICENSE](licenses/slotmap-1.1.1/LICENSE) |
| smallvec | 1.16.5 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/smallvec-1.16.5/LICENSE-APACHE), [LICENSE-MIT](licenses/smallvec-1.16.5/LICENSE-MIT) |
| smithay-client-toolkit | 0.19.2 | `MIT` | [LICENSE.txt](licenses/smithay-client-toolkit-0.19.2/LICENSE.txt) |
| smithay-client-toolkit | 0.20.0 | `MIT` | [LICENSE.txt](licenses/smithay-client-toolkit-0.20.0/LICENSE.txt) |
| smithay-clipboard | 0.7.3 | `MIT` | [LICENSE](licenses/smithay-clipboard-0.7.3/LICENSE) |
| smol_str | 0.2.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/smol_str-0.2.2/LICENSE-APACHE), [LICENSE-MIT](licenses/smol_str-0.2.2/LICENSE-MIT) |
| snap | 1.1.2 | `BSD-3-Clause` | [COPYING](licenses/snap-1.1.2/COPYING) |
| spirv | 0.4.0+sdk-1.4.341.0 | `Apache-2.0` | Отдельный файл в архиве отсутствует |
| sqlite-wasm-rs | 0.6.3 | `MIT` | [LICENSE](licenses/sqlite-wasm-rs-0.6.3/LICENSE) |
| stable_deref_trait | 1.2.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/stable_deref_trait-1.2.1/LICENSE-APACHE), [LICENSE-MIT](licenses/stable_deref_trait-1.2.1/LICENSE-MIT) |
| static_assertions | 1.1.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/static_assertions-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/static_assertions-1.1.0/LICENSE-MIT) |
| strict-num | 0.1.1 | `MIT` | [LICENSE](licenses/strict-num-0.1.1/LICENSE) |
| syn | 2.0.119 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/syn-2.0.119/LICENSE-APACHE), [LICENSE-MIT](licenses/syn-2.0.119/LICENSE-MIT) |
| syn | 3.0.10 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/syn-3.0.10/LICENSE-APACHE), [LICENSE-MIT](licenses/syn-3.0.10/LICENSE-MIT) |
| synstructure | 0.14.0 | `MIT` | [LICENSE](licenses/synstructure-0.14.0/LICENSE) |
| sys-locale | 0.3.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/sys-locale-0.3.2/LICENSE-APACHE), [LICENSE-MIT](licenses/sys-locale-0.3.2/LICENSE-MIT) |
| tempfile | 3.27.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/tempfile-3.27.0/LICENSE-APACHE), [LICENSE-MIT](licenses/tempfile-3.27.0/LICENSE-MIT) |
| termcolor | 1.4.1 | `Unlicense OR MIT` | [COPYING](licenses/termcolor-1.4.1/COPYING), [LICENSE-MIT](licenses/termcolor-1.4.1/LICENSE-MIT), [UNLICENSE](licenses/termcolor-1.4.1/UNLICENSE) |
| thiserror | 1.0.69 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/thiserror-1.0.69/LICENSE-APACHE), [LICENSE-MIT](licenses/thiserror-1.0.69/LICENSE-MIT) |
| thiserror | 2.0.21 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/thiserror-2.0.21/LICENSE-APACHE), [LICENSE-MIT](licenses/thiserror-2.0.21/LICENSE-MIT) |
| thiserror-impl | 1.0.69 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/thiserror-impl-1.0.69/LICENSE-APACHE), [LICENSE-MIT](licenses/thiserror-impl-1.0.69/LICENSE-MIT) |
| thiserror-impl | 2.0.21 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/thiserror-impl-2.0.21/LICENSE-APACHE), [LICENSE-MIT](licenses/thiserror-impl-2.0.21/LICENSE-MIT) |
| tiff | 0.11.3 | `MIT` | [LICENSE](licenses/tiff-0.11.3/LICENSE) |
| tiny-keccak | 2.0.2 | `CC0-1.0` | [LICENSE](licenses/tiny-keccak-2.0.2/LICENSE) |
| tiny-skia | 0.11.4 | `BSD-3-Clause` | [LICENSE](licenses/tiny-skia-0.11.4/LICENSE) |
| tiny-skia-path | 0.11.4 | `BSD-3-Clause` | [LICENSE](licenses/tiny-skia-path-0.11.4/LICENSE) |
| tinystr | 0.8.5 | `Unicode-3.0` | [LICENSE](licenses/tinystr-0.8.5/LICENSE) |
| tokio | 1.54.1 | `MIT` | [LICENSE](licenses/tokio-1.54.1/LICENSE) |
| toml_datetime | 1.1.3+spec-1.1.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/toml_datetime-1.1.3+spec-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/toml_datetime-1.1.3+spec-1.1.0/LICENSE-MIT) |
| toml_edit | 0.25.19+spec-1.1.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/toml_edit-0.25.19+spec-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/toml_edit-0.25.19+spec-1.1.0/LICENSE-MIT) |
| toml_parser | 1.1.5+spec-1.1.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/toml_parser-1.1.5+spec-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/toml_parser-1.1.5+spec-1.1.0/LICENSE-MIT) |
| tracing | 0.1.44 | `MIT` | [LICENSE](licenses/tracing-0.1.44/LICENSE) |
| tracing-attributes | 0.1.31 | `MIT` | [LICENSE](licenses/tracing-attributes-0.1.31/LICENSE) |
| tracing-core | 0.1.36 | `MIT` | [LICENSE](licenses/tracing-core-0.1.36/LICENSE) |
| tree_magic_mini | 3.2.2 | `MIT` | [LICENSE](licenses/tree_magic_mini-3.2.2/LICENSE) |
| ttf-parser | 0.25.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/ttf-parser-0.25.1/LICENSE-APACHE), [LICENSE-MIT](licenses/ttf-parser-0.25.1/LICENSE-MIT) |
| twox-hash | 2.1.5 | `MIT` | [LICENSE.txt](licenses/twox-hash-2.1.5/LICENSE.txt) |
| type-map | 0.5.1 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/type-map-0.5.1/LICENSE-APACHE), [LICENSE-MIT](licenses/type-map-0.5.1/LICENSE-MIT) |
| typeid | 1.0.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/typeid-1.0.3/LICENSE-APACHE), [LICENSE-MIT](licenses/typeid-1.0.3/LICENSE-MIT) |
| typenum | 1.20.1 | `MIT OR Apache-2.0` | [LICENSE](licenses/typenum-1.20.1/LICENSE), [LICENSE-APACHE](licenses/typenum-1.20.1/LICENSE-APACHE), [LICENSE-MIT](licenses/typenum-1.20.1/LICENSE-MIT) |
| uds_windows | 1.2.1 | `MIT` | [LICENSE](licenses/uds_windows-1.2.1/LICENSE) |
| unicode-general-category | 1.1.0 | `Apache-2.0` | [LICENSE](licenses/unicode-general-category-1.1.0/LICENSE) |
| unicode-ident | 1.0.27 | `(MIT OR Apache-2.0) AND Unicode-3.0` | [LICENSE-APACHE](licenses/unicode-ident-1.0.27/LICENSE-APACHE), [LICENSE-MIT](licenses/unicode-ident-1.0.27/LICENSE-MIT), [LICENSE-UNICODE](licenses/unicode-ident-1.0.27/LICENSE-UNICODE) |
| unicode-properties | 0.1.5 | `(MIT OR Apache-2.0) AND Unicode-3.0` | [COPYRIGHT](licenses/unicode-properties-0.1.5/COPYRIGHT), [LICENSE-APACHE](licenses/unicode-properties-0.1.5/LICENSE-APACHE), [LICENSE-MIT](licenses/unicode-properties-0.1.5/LICENSE-MIT), [LICENSE-UNICODE](licenses/unicode-properties-0.1.5/LICENSE-UNICODE) |
| unicode-script | 0.5.9 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/unicode-script-0.5.9/LICENSE-APACHE), [LICENSE-MIT](licenses/unicode-script-0.5.9/LICENSE-MIT) |
| unicode-segmentation | 1.13.3 | `MIT OR Apache-2.0` | [COPYRIGHT](licenses/unicode-segmentation-1.13.3/COPYRIGHT), [LICENSE-APACHE](licenses/unicode-segmentation-1.13.3/LICENSE-APACHE), [LICENSE-MIT](licenses/unicode-segmentation-1.13.3/LICENSE-MIT) |
| unicode-width | 0.2.3 | `MIT OR Apache-2.0` | [COPYRIGHT](licenses/unicode-width-0.2.3/COPYRIGHT), [LICENSE-APACHE](licenses/unicode-width-0.2.3/LICENSE-APACHE), [LICENSE-MIT](licenses/unicode-width-0.2.3/LICENSE-MIT) |
| url | 2.5.8 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/url-2.5.8/LICENSE-APACHE), [LICENSE-MIT](licenses/url-2.5.8/LICENSE-MIT) |
| utf8_iter | 1.1.0 | `Apache-2.0 OR MIT` | [COPYRIGHT](licenses/utf8_iter-1.1.0/COPYRIGHT), [LICENSE-APACHE](licenses/utf8_iter-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/utf8_iter-1.1.0/LICENSE-MIT) |
| utf_types | 1.0.0 | `Apache-2.0 OR MIT` | [COPYRIGHT](licenses/utf_types-1.0.0/COPYRIGHT), [LICENSE-APACHE](licenses/utf_types-1.0.0/LICENSE-APACHE), [LICENSE-MIT](licenses/utf_types-1.0.0/LICENSE-MIT) |
| uuid | 1.29.3 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/uuid-1.29.3/LICENSE-APACHE), [LICENSE-MIT](licenses/uuid-1.29.3/LICENSE-MIT) |
| vcpkg | 0.2.15 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/vcpkg-0.2.15/LICENSE-APACHE), [LICENSE-MIT](licenses/vcpkg-0.2.15/LICENSE-MIT) |
| vello_common | 0.2.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/vello_common-0.2.0/LICENSE-APACHE), [LICENSE-MIT](licenses/vello_common-0.2.0/LICENSE-MIT) |
| vello_cpu | 0.2.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/vello_cpu-0.2.0/LICENSE-APACHE), [LICENSE-MIT](licenses/vello_cpu-0.2.0/LICENSE-MIT) |
| version_check | 0.9.5 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/version_check-0.9.5/LICENSE-APACHE), [LICENSE-MIT](licenses/version_check-0.9.5/LICENSE-MIT) |
| walkdir | 2.5.0 | `Unlicense/MIT` | [COPYING](licenses/walkdir-2.5.0/COPYING), [LICENSE-MIT](licenses/walkdir-2.5.0/LICENSE-MIT), [UNLICENSE](licenses/walkdir-2.5.0/UNLICENSE) |
| wasi | 0.11.1+wasi-snapshot-preview1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/wasi-0.11.1+wasi-snapshot-preview1/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/wasi-0.11.1+wasi-snapshot-preview1/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/wasi-0.11.1+wasi-snapshot-preview1/LICENSE-MIT) |
| wasip2 | 1.0.4+wasi-0.2.12 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/wasip2-1.0.4+wasi-0.2.12/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/wasip2-1.0.4+wasi-0.2.12/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/wasip2-1.0.4+wasi-0.2.12/LICENSE-MIT) |
| wasm-bindgen | 0.2.130 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wasm-bindgen-0.2.130/LICENSE-APACHE), [LICENSE-MIT](licenses/wasm-bindgen-0.2.130/LICENSE-MIT) |
| wasm-bindgen-futures | 0.4.80 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wasm-bindgen-futures-0.4.80/LICENSE-APACHE), [LICENSE-MIT](licenses/wasm-bindgen-futures-0.4.80/LICENSE-MIT) |
| wasm-bindgen-macro | 0.2.130 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wasm-bindgen-macro-0.2.130/LICENSE-APACHE), [LICENSE-MIT](licenses/wasm-bindgen-macro-0.2.130/LICENSE-MIT) |
| wasm-bindgen-macro-support | 0.2.130 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wasm-bindgen-macro-support-0.2.130/LICENSE-APACHE), [LICENSE-MIT](licenses/wasm-bindgen-macro-support-0.2.130/LICENSE-MIT) |
| wasm-bindgen-shared | 0.2.130 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wasm-bindgen-shared-0.2.130/LICENSE-APACHE), [LICENSE-MIT](licenses/wasm-bindgen-shared-0.2.130/LICENSE-MIT) |
| wayland-backend | 0.3.17 | `MIT` | [LICENSE.txt](licenses/wayland-backend-0.3.17/LICENSE.txt) |
| wayland-client | 0.31.15 | `MIT` | [LICENSE.txt](licenses/wayland-client-0.31.15/LICENSE.txt) |
| wayland-csd-frame | 0.3.0 | `MIT` | [LICENSE](licenses/wayland-csd-frame-0.3.0/LICENSE) |
| wayland-cursor | 0.31.14 | `MIT` | [LICENSE.txt](licenses/wayland-cursor-0.31.14/LICENSE.txt) |
| wayland-protocols | 0.32.13 | `MIT` | [LICENSE.txt](licenses/wayland-protocols-0.32.13/LICENSE.txt) |
| wayland-protocols-experimental | 20250721.0.1 | `MIT` | [LICENSE.txt](licenses/wayland-protocols-experimental-20250721.0.1/LICENSE.txt) |
| wayland-protocols-misc | 0.3.12 | `MIT` | [LICENSE.txt](licenses/wayland-protocols-misc-0.3.12/LICENSE.txt) |
| wayland-protocols-plasma | 0.3.12 | `MIT` | [LICENSE.txt](licenses/wayland-protocols-plasma-0.3.12/LICENSE.txt) |
| wayland-protocols-wlr | 0.3.12 | `MIT` | [LICENSE.txt](licenses/wayland-protocols-wlr-0.3.12/LICENSE.txt) |
| wayland-scanner | 0.31.11 | `MIT` | [LICENSE.txt](licenses/wayland-scanner-0.31.11/LICENSE.txt) |
| wayland-sys | 0.31.11 | `MIT` | [LICENSE.txt](licenses/wayland-sys-0.31.11/LICENSE.txt) |
| web-sys | 0.3.107 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/web-sys-0.3.107/LICENSE-APACHE), [LICENSE-MIT](licenses/web-sys-0.3.107/LICENSE-MIT) |
| web-time | 1.1.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/web-time-1.1.0/LICENSE-APACHE), [LICENSE-MIT](licenses/web-time-1.1.0/LICENSE-MIT) |
| webbrowser | 1.2.7 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/webbrowser-1.2.7/LICENSE-APACHE), [LICENSE-MIT](licenses/webbrowser-1.2.7/LICENSE-MIT) |
| weezl | 0.1.12 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/weezl-0.1.12/LICENSE-APACHE), [LICENSE-MIT](licenses/weezl-0.1.12/LICENSE-MIT) |
| wgpu | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/wgpu-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/wgpu-30.0.2/LICENSE.MIT) |
| wgpu-core | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/wgpu-core-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/wgpu-core-30.0.2/LICENSE.MIT) |
| wgpu-core-deps-apple | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/wgpu-core-deps-apple-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/wgpu-core-deps-apple-30.0.2/LICENSE.MIT) |
| wgpu-core-deps-emscripten | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/wgpu-core-deps-emscripten-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/wgpu-core-deps-emscripten-30.0.2/LICENSE.MIT) |
| wgpu-core-deps-wasm | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/wgpu-core-deps-wasm-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/wgpu-core-deps-wasm-30.0.2/LICENSE.MIT) |
| wgpu-core-deps-windows-linux-android | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/wgpu-core-deps-windows-linux-android-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/wgpu-core-deps-windows-linux-android-30.0.2/LICENSE.MIT) |
| wgpu-hal | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/wgpu-hal-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/wgpu-hal-30.0.2/LICENSE.MIT) |
| wgpu-naga-bridge | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/wgpu-naga-bridge-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/wgpu-naga-bridge-30.0.2/LICENSE.MIT) |
| wgpu-types | 30.0.2 | `MIT OR Apache-2.0` | [LICENSE.APACHE](licenses/wgpu-types-30.0.2/LICENSE.APACHE), [LICENSE.MIT](licenses/wgpu-types-30.0.2/LICENSE.MIT) |
| winapi-util | 0.1.11 | `Unlicense OR MIT` | [COPYING](licenses/winapi-util-0.1.11/COPYING), [LICENSE-MIT](licenses/winapi-util-0.1.11/LICENSE-MIT), [UNLICENSE](licenses/winapi-util-0.1.11/UNLICENSE) |
| windows | 0.62.2 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-0.62.2/license-apache-2.0), [license-mit](licenses/windows-0.62.2/license-mit) |
| windows-collections | 0.3.2 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-collections-0.3.2/license-apache-2.0), [license-mit](licenses/windows-collections-0.3.2/license-mit) |
| windows-core | 0.62.2 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-core-0.62.2/license-apache-2.0), [license-mit](licenses/windows-core-0.62.2/license-mit) |
| windows-future | 0.3.2 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-future-0.3.2/license-apache-2.0), [license-mit](licenses/windows-future-0.3.2/license-mit) |
| windows-implement | 0.60.2 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-implement-0.60.2/license-apache-2.0), [license-mit](licenses/windows-implement-0.60.2/license-mit) |
| windows-interface | 0.59.3 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-interface-0.59.3/license-apache-2.0), [license-mit](licenses/windows-interface-0.59.3/license-mit) |
| windows-link | 0.2.1 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-link-0.2.1/license-apache-2.0), [license-mit](licenses/windows-link-0.2.1/license-mit) |
| windows-numerics | 0.3.1 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-numerics-0.3.1/license-apache-2.0), [license-mit](licenses/windows-numerics-0.3.1/license-mit) |
| windows-result | 0.4.1 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-result-0.4.1/license-apache-2.0), [license-mit](licenses/windows-result-0.4.1/license-mit) |
| windows-strings | 0.5.1 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-strings-0.5.1/license-apache-2.0), [license-mit](licenses/windows-strings-0.5.1/license-mit) |
| windows-sys | 0.52.0 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-sys-0.52.0/license-apache-2.0), [license-mit](licenses/windows-sys-0.52.0/license-mit) |
| windows-sys | 0.59.0 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-sys-0.59.0/license-apache-2.0), [license-mit](licenses/windows-sys-0.59.0/license-mit) |
| windows-sys | 0.61.2 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-sys-0.61.2/license-apache-2.0), [license-mit](licenses/windows-sys-0.61.2/license-mit) |
| windows-targets | 0.52.6 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-targets-0.52.6/license-apache-2.0), [license-mit](licenses/windows-targets-0.52.6/license-mit) |
| windows-threading | 0.2.1 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-threading-0.2.1/license-apache-2.0), [license-mit](licenses/windows-threading-0.2.1/license-mit) |
| windows_aarch64_gnullvm | 0.52.6 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows_aarch64_gnullvm-0.52.6/license-apache-2.0), [license-mit](licenses/windows_aarch64_gnullvm-0.52.6/license-mit) |
| windows_aarch64_msvc | 0.52.6 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows_aarch64_msvc-0.52.6/license-apache-2.0), [license-mit](licenses/windows_aarch64_msvc-0.52.6/license-mit) |
| windows_i686_gnu | 0.52.6 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows_i686_gnu-0.52.6/license-apache-2.0), [license-mit](licenses/windows_i686_gnu-0.52.6/license-mit) |
| windows_i686_gnullvm | 0.52.6 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows_i686_gnullvm-0.52.6/license-apache-2.0), [license-mit](licenses/windows_i686_gnullvm-0.52.6/license-mit) |
| windows_i686_msvc | 0.52.6 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows_i686_msvc-0.52.6/license-apache-2.0), [license-mit](licenses/windows_i686_msvc-0.52.6/license-mit) |
| windows_x86_64_gnu | 0.52.6 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows_x86_64_gnu-0.52.6/license-apache-2.0), [license-mit](licenses/windows_x86_64_gnu-0.52.6/license-mit) |
| windows_x86_64_gnullvm | 0.52.6 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows_x86_64_gnullvm-0.52.6/license-apache-2.0), [license-mit](licenses/windows_x86_64_gnullvm-0.52.6/license-mit) |
| windows_x86_64_msvc | 0.52.6 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows_x86_64_msvc-0.52.6/license-apache-2.0), [license-mit](licenses/windows_x86_64_msvc-0.52.6/license-mit) |
| winit | 0.30.13 | `Apache-2.0` | [LICENSE](licenses/winit-0.30.13/LICENSE) |
| winnow | 1.0.5 | `MIT` | [LICENSE-MIT](licenses/winnow-1.0.5/LICENSE-MIT) |
| wit-bindgen | 0.57.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/wit-bindgen-0.57.1/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/wit-bindgen-0.57.1/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/wit-bindgen-0.57.1/LICENSE-MIT) |
| wl-clipboard-rs | 0.9.5 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wl-clipboard-rs-0.9.5/LICENSE-APACHE), [LICENSE-MIT](licenses/wl-clipboard-rs-0.9.5/LICENSE-MIT) |
| writeable | 0.6.4 | `Unicode-3.0` | [LICENSE](licenses/writeable-0.6.4/LICENSE) |
| x11-dl | 2.21.0 | `MIT` | [LICENSE-MIT](licenses/x11-dl-2.21.0/LICENSE-MIT) |
| x11rb | 0.13.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/x11rb-0.13.2/LICENSE-APACHE), [LICENSE-MIT](licenses/x11rb-0.13.2/LICENSE-MIT) |
| x11rb-protocol | 0.13.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/x11rb-protocol-0.13.2/LICENSE-APACHE), [LICENSE-MIT](licenses/x11rb-protocol-0.13.2/LICENSE-MIT) |
| xcursor | 0.3.11 | `MIT` | [LICENSE](licenses/xcursor-0.3.11/LICENSE) |
| xkbcommon-dl | 0.4.2 | `MIT` | [LICENSE](licenses/xkbcommon-dl-0.4.2/LICENSE) |
| xkeysym | 0.2.1 | `MIT OR Apache-2.0 OR Zlib` | [LICENSE-APACHE](licenses/xkeysym-0.2.1/LICENSE-APACHE), [LICENSE-MIT](licenses/xkeysym-0.2.1/LICENSE-MIT), [LICENSE-ZLIB](licenses/xkeysym-0.2.1/LICENSE-ZLIB) |
| xml-rs | 0.8.29 | `MIT` | [LICENSE](licenses/xml-rs-0.8.29/LICENSE) |
| yeslogic-fontconfig-sys | 6.0.1 | `MIT` | [LICENSE](licenses/yeslogic-fontconfig-sys-6.0.1/LICENSE) |
| yoke | 0.8.4 | `Unicode-3.0` | [LICENSE](licenses/yoke-0.8.4/LICENSE) |
| yoke-derive | 0.8.4 | `Unicode-3.0` | [LICENSE](licenses/yoke-derive-0.8.4/LICENSE) |
| zbus | 5.19.0 | `MIT` | [LICENSE](licenses/zbus-5.19.0/LICENSE) |
| zbus-lockstep | 0.5.2 | `MIT` | [LICENSE-MIT](licenses/zbus-lockstep-0.5.2/LICENSE-MIT) |
| zbus-lockstep-macros | 0.5.2 | `MIT` | [LICENSE-MIT](licenses/zbus-lockstep-macros-0.5.2/LICENSE-MIT) |
| zbus_macros | 5.19.0 | `MIT` | [LICENSE](licenses/zbus_macros-5.19.0/LICENSE) |
| zbus_names | 4.3.5 | `MIT` | [LICENSE](licenses/zbus_names-4.3.5/LICENSE) |
| zbus_xml | 5.2.1 | `MIT` | [LICENSE](licenses/zbus_xml-5.2.1/LICENSE) |
| zcheapstr | 1.2.0 | `MIT` | [LICENSE](licenses/zcheapstr-1.2.0/LICENSE) |
| zerocopy | 0.8.63 | `BSD-2-Clause OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/zerocopy-0.8.63/LICENSE-APACHE), [LICENSE-BSD](licenses/zerocopy-0.8.63/LICENSE-BSD), [LICENSE-MIT](licenses/zerocopy-0.8.63/LICENSE-MIT) |
| zerocopy-derive | 0.8.63 | `BSD-2-Clause OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/zerocopy-derive-0.8.63/LICENSE-APACHE), [LICENSE-BSD](licenses/zerocopy-derive-0.8.63/LICENSE-BSD), [LICENSE-MIT](licenses/zerocopy-derive-0.8.63/LICENSE-MIT) |
| zerofrom | 0.1.8 | `Unicode-3.0` | [LICENSE](licenses/zerofrom-0.1.8/LICENSE) |
| zerofrom-derive | 0.1.8 | `Unicode-3.0` | [LICENSE](licenses/zerofrom-derive-0.1.8/LICENSE) |
| zerotrie | 0.2.5 | `Unicode-3.0` | [LICENSE](licenses/zerotrie-0.2.5/LICENSE) |
| zerovec | 0.11.9 | `Unicode-3.0` | [LICENSE](licenses/zerovec-0.11.9/LICENSE) |
| zerovec-derive | 0.11.7 | `Unicode-3.0` | [LICENSE](licenses/zerovec-derive-0.11.7/LICENSE) |
| zlib-rs | 0.6.8 | `Zlib` | [LICENSE](licenses/zlib-rs-0.6.8/LICENSE) |
| zmij | 1.0.23 | `MIT` | [LICENSE-MIT](licenses/zmij-1.0.23/LICENSE-MIT) |
| zune-core | 0.5.3 | `MIT OR Apache-2.0 OR Zlib` | [LICENSE-APACHE](licenses/zune-core-0.5.3/LICENSE-APACHE), [LICENSE-MIT](licenses/zune-core-0.5.3/LICENSE-MIT), [LICENSE-ZLIB](licenses/zune-core-0.5.3/LICENSE-ZLIB) |
| zune-jpeg | 0.5.15 | `MIT OR Apache-2.0 OR Zlib` | [LICENSE-APACHE](licenses/zune-jpeg-0.5.15/LICENSE-APACHE), [LICENSE-MIT](licenses/zune-jpeg-0.5.15/LICENSE-MIT), [LICENSE-ZLIB](licenses/zune-jpeg-0.5.15/LICENSE-ZLIB) |
| zvariant | 5.15.1 | `MIT` | [LICENSE](licenses/zvariant-5.15.1/LICENSE) |
| zvariant_derive | 5.15.1 | `MIT` | [LICENSE](licenses/zvariant_derive-5.15.1/LICENSE) |
| zvariant_utils | 4.2.0 | `MIT` | [LICENSE](licenses/zvariant_utils-4.2.0/LICENSE) |
