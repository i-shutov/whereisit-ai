# Уведомления о сторонних компонентах

Этот перечень составлен по зафиксированным registry-зависимостям в `Cargo.lock`; он включает зависимости для сборки, тестов и разных платформ, поэтому не каждый пакет обязательно присутствует в каждом бинарном файле. Версии и выражения лицензий приведены по локальным пакетам Cargo без изменения их исходной записи. Лицензии с `OR` предоставляют указанные альтернативы, а `AND` требует выполнения обеих частей.

Лицензионные файлы и файлы NOTICE, включённые в локальные пакеты, сохранены по папкам `licenses/<имя>-<версия>/`. Для трёх пакетов исходный архив в локальном реестре не содержит отдельного файла лицензии: r-efi 5.3.0 и 6.0.0, thrift 0.17.0; выражения ниже взяты из их Cargo.toml. Apache-2.0 и MIT тексты доступны в других папках настоящего комплекта. Для окончательного распространяемого дистрибутива следует повторно сверить набор и уведомления при обновлении lockfile или подключении других компонентов.

SHA-256 Cargo.lock: `b94d11ada91ea0eb664195c62fed82171d50971d5dcdbe3d8fe78008f27e5b28`.

## Встроенный SQLite

Сборка с `rusqlite` и `libsqlite3-sys` использует встроенный SQLite 3.46.0. В составе локального исходника его авторы отказываются от авторского права; сохранён соответствующий заголовок в `licenses/SQLite-3.46.0/COPYRIGHT.txt`. Лицензия Rust-обёртки `libsqlite3-sys` приведена отдельно.

## Зафиксированные registry-пакеты

| Пакет | Версия | Лицензия из Cargo.toml | Файлы |
|---|---|---|---|
| ahash | 0.8.12 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/ahash-0.8.12/LICENSE-APACHE), [LICENSE-MIT](licenses/ahash-0.8.12/LICENSE-MIT) |
| android_system_properties | 0.1.6 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/android_system_properties-0.1.6/LICENSE-APACHE), [LICENSE-MIT](licenses/android_system_properties-0.1.6/LICENSE-MIT) |
| autocfg | 1.5.1 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/autocfg-1.5.1/LICENSE-APACHE), [LICENSE-MIT](licenses/autocfg-1.5.1/LICENSE-MIT) |
| bitflags | 2.13.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/bitflags-2.13.2/LICENSE-APACHE), [LICENSE-MIT](licenses/bitflags-2.13.2/LICENSE-MIT) |
| block-buffer | 0.10.4 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/block-buffer-0.10.4/LICENSE-APACHE), [LICENSE-MIT](licenses/block-buffer-0.10.4/LICENSE-MIT) |
| bumpalo | 3.20.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/bumpalo-3.20.3/LICENSE-APACHE), [LICENSE-MIT](licenses/bumpalo-3.20.3/LICENSE-MIT) |
| byteorder | 1.5.0 | `Unlicense OR MIT` | [COPYING](licenses/byteorder-1.5.0/COPYING), [LICENSE-MIT](licenses/byteorder-1.5.0/LICENSE-MIT), [UNLICENSE](licenses/byteorder-1.5.0/UNLICENSE) |
| bytes | 1.12.1 | `MIT` | [LICENSE](licenses/bytes-1.12.1/LICENSE) |
| cc | 1.10.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/cc-1.10.0/LICENSE-APACHE), [LICENSE-MIT](licenses/cc-1.10.0/LICENSE-MIT) |
| cfg-if | 1.0.5 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/cfg-if-1.0.5/LICENSE-APACHE), [LICENSE-MIT](licenses/cfg-if-1.0.5/LICENSE-MIT) |
| chrono | 0.4.45 | `MIT OR Apache-2.0` | [LICENSE.txt](licenses/chrono-0.4.45/LICENSE.txt) |
| const-random | 0.1.18 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/const-random-0.1.18/LICENSE-APACHE), [LICENSE-MIT](licenses/const-random-0.1.18/LICENSE-MIT) |
| const-random-macro | 0.1.16 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/const-random-macro-0.1.16/LICENSE-APACHE), [LICENSE-MIT](licenses/const-random-macro-0.1.16/LICENSE-MIT) |
| core-foundation-sys | 0.8.7 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/core-foundation-sys-0.8.7/LICENSE-APACHE), [LICENSE-MIT](licenses/core-foundation-sys-0.8.7/LICENSE-MIT) |
| cpufeatures | 0.2.17 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/cpufeatures-0.2.17/LICENSE-APACHE), [LICENSE-MIT](licenses/cpufeatures-0.2.17/LICENSE-MIT) |
| crunchy | 0.2.4 | `MIT` | [LICENSE](licenses/crunchy-0.2.4/LICENSE) |
| crypto-common | 0.1.7 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/crypto-common-0.1.7/LICENSE-APACHE), [LICENSE-MIT](licenses/crypto-common-0.1.7/LICENSE-MIT) |
| csv | 1.4.0 | `Unlicense/MIT` | [COPYING](licenses/csv-1.4.0/COPYING), [LICENSE-MIT](licenses/csv-1.4.0/LICENSE-MIT), [UNLICENSE](licenses/csv-1.4.0/UNLICENSE) |
| csv-core | 0.1.13 | `Unlicense/MIT` | [COPYING](licenses/csv-core-0.1.13/COPYING), [LICENSE-MIT](licenses/csv-core-0.1.13/LICENSE-MIT), [UNLICENSE](licenses/csv-core-0.1.13/UNLICENSE) |
| digest | 0.10.7 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/digest-0.10.7/LICENSE-APACHE), [LICENSE-MIT](licenses/digest-0.10.7/LICENSE-MIT) |
| errno | 0.3.14 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/errno-0.3.14/LICENSE-APACHE), [LICENSE-MIT](licenses/errno-0.3.14/LICENSE-MIT) |
| fallible-iterator | 0.3.0 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/fallible-iterator-0.3.0/LICENSE-APACHE), [LICENSE-MIT](licenses/fallible-iterator-0.3.0/LICENSE-MIT) |
| fallible-streaming-iterator | 0.1.9 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/fallible-streaming-iterator-0.1.9/LICENSE-APACHE), [LICENSE-MIT](licenses/fallible-streaming-iterator-0.1.9/LICENSE-MIT) |
| fastrand | 2.5.0 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/fastrand-2.5.0/LICENSE-APACHE), [LICENSE-MIT](licenses/fastrand-2.5.0/LICENSE-MIT) |
| find-msvc-tools | 0.1.15 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/find-msvc-tools-0.1.15/LICENSE-APACHE), [LICENSE-MIT](licenses/find-msvc-tools-0.1.15/LICENSE-MIT) |
| futures-core | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/futures-core-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/futures-core-0.3.34/LICENSE-MIT) |
| futures-task | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/futures-task-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/futures-task-0.3.34/LICENSE-MIT) |
| futures-util | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/futures-util-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/futures-util-0.3.34/LICENSE-MIT) |
| generic-array | 0.14.7 | `MIT` | [LICENSE](licenses/generic-array-0.14.7/LICENSE) |
| getrandom | 0.2.17 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/getrandom-0.2.17/LICENSE-APACHE), [LICENSE-MIT](licenses/getrandom-0.2.17/LICENSE-MIT) |
| getrandom | 0.3.4 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/getrandom-0.3.4/LICENSE-APACHE), [LICENSE-MIT](licenses/getrandom-0.3.4/LICENSE-MIT) |
| getrandom | 0.4.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/getrandom-0.4.3/LICENSE-APACHE), [LICENSE-MIT](licenses/getrandom-0.4.3/LICENSE-MIT) |
| half | 2.7.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/half-2.7.1/LICENSE-APACHE), [LICENSE-MIT](licenses/half-2.7.1/LICENSE-MIT) |
| hashbrown | 0.14.5 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hashbrown-0.14.5/LICENSE-APACHE), [LICENSE-MIT](licenses/hashbrown-0.14.5/LICENSE-MIT) |
| hashbrown | 0.16.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hashbrown-0.16.1/LICENSE-APACHE), [LICENSE-MIT](licenses/hashbrown-0.16.1/LICENSE-MIT) |
| hashlink | 0.9.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/hashlink-0.9.1/LICENSE-APACHE), [LICENSE-MIT](licenses/hashlink-0.9.1/LICENSE-MIT) |
| iana-time-zone | 0.1.65 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/iana-time-zone-0.1.65/LICENSE-APACHE), [LICENSE-MIT](licenses/iana-time-zone-0.1.65/LICENSE-MIT) |
| iana-time-zone-haiku | 0.1.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/iana-time-zone-haiku-0.1.2/LICENSE-APACHE), [LICENSE-MIT](licenses/iana-time-zone-haiku-0.1.2/LICENSE-MIT) |
| integer-encoding | 3.0.4 | `MIT` | [LICENSE](licenses/integer-encoding-3.0.4/LICENSE) |
| itoa | 1.0.18 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/itoa-1.0.18/LICENSE-APACHE), [LICENSE-MIT](licenses/itoa-1.0.18/LICENSE-MIT) |
| js-sys | 0.3.106 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/js-sys-0.3.106/LICENSE-APACHE), [LICENSE-MIT](licenses/js-sys-0.3.106/LICENSE-MIT) |
| libc | 0.2.191 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/libc-0.2.191/LICENSE-APACHE), [LICENSE-MIT](licenses/libc-0.2.191/LICENSE-MIT) |
| libm | 0.2.16 | `MIT` | [LICENSE.txt](licenses/libm-0.2.16/LICENSE.txt) |
| libsqlite3-sys | 0.30.1 | `MIT` | [LICENSE](licenses/libsqlite3-sys-0.30.1/LICENSE) |
| linux-raw-sys | 0.12.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/linux-raw-sys-0.12.1/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/linux-raw-sys-0.12.1/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/linux-raw-sys-0.12.1/LICENSE-MIT) |
| log | 0.4.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/log-0.4.34/LICENSE-APACHE), [LICENSE-MIT](licenses/log-0.4.34/LICENSE-MIT) |
| memchr | 2.8.3 | `Unlicense OR MIT` | [COPYING](licenses/memchr-2.8.3/COPYING), [LICENSE-MIT](licenses/memchr-2.8.3/LICENSE-MIT), [UNLICENSE](licenses/memchr-2.8.3/UNLICENSE) |
| num | 0.4.3 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-0.4.3/LICENSE-APACHE), [LICENSE-MIT](licenses/num-0.4.3/LICENSE-MIT) |
| num-bigint | 0.4.8 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-bigint-0.4.8/LICENSE-APACHE), [LICENSE-MIT](licenses/num-bigint-0.4.8/LICENSE-MIT) |
| num-complex | 0.4.6 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-complex-0.4.6/LICENSE-APACHE), [LICENSE-MIT](licenses/num-complex-0.4.6/LICENSE-MIT) |
| num-integer | 0.1.47 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-integer-0.1.47/LICENSE-APACHE), [LICENSE-MIT](licenses/num-integer-0.1.47/LICENSE-MIT) |
| num-iter | 0.1.46 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-iter-0.1.46/LICENSE-APACHE), [LICENSE-MIT](licenses/num-iter-0.1.46/LICENSE-MIT) |
| num-rational | 0.4.2 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-rational-0.4.2/LICENSE-APACHE), [LICENSE-MIT](licenses/num-rational-0.4.2/LICENSE-MIT) |
| num-traits | 0.2.19 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/num-traits-0.2.19/LICENSE-APACHE), [LICENSE-MIT](licenses/num-traits-0.2.19/LICENSE-MIT) |
| once_cell | 1.21.4 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/once_cell-1.21.4/LICENSE-APACHE), [LICENSE-MIT](licenses/once_cell-1.21.4/LICENSE-MIT) |
| ordered-float | 2.10.1 | `MIT` | [LICENSE-MIT](licenses/ordered-float-2.10.1/LICENSE-MIT) |
| parquet | 56.2.1 | `Apache-2.0` | [LICENSE.txt](licenses/parquet-56.2.1/LICENSE.txt), [NOTICE.txt](licenses/parquet-56.2.1/NOTICE.txt) |
| paste | 1.0.15 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/paste-1.0.15/LICENSE-APACHE), [LICENSE-MIT](licenses/paste-1.0.15/LICENSE-MIT) |
| pin-project-lite | 0.2.17 | `Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/pin-project-lite-0.2.17/LICENSE-APACHE), [LICENSE-MIT](licenses/pin-project-lite-0.2.17/LICENSE-MIT) |
| pkg-config | 0.3.34 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/pkg-config-0.3.34/LICENSE-APACHE), [LICENSE-MIT](licenses/pkg-config-0.3.34/LICENSE-MIT) |
| proc-macro2 | 1.0.107 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/proc-macro2-1.0.107/LICENSE-APACHE), [LICENSE-MIT](licenses/proc-macro2-1.0.107/LICENSE-MIT) |
| quote | 1.0.47 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/quote-1.0.47/LICENSE-APACHE), [LICENSE-MIT](licenses/quote-1.0.47/LICENSE-MIT) |
| r-efi | 5.3.0 | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | В архиве нет отдельного файла |
| r-efi | 6.0.0 | `MIT OR Apache-2.0 OR LGPL-2.1-or-later` | В архиве нет отдельного файла |
| rusqlite | 0.32.1 | `MIT` | [LICENSE](licenses/rusqlite-0.32.1/LICENSE) |
| rustix | 1.1.6 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/rustix-1.1.6/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/rustix-1.1.6/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/rustix-1.1.6/LICENSE-MIT) |
| rustversion | 1.0.23 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/rustversion-1.0.23/LICENSE-APACHE), [LICENSE-MIT](licenses/rustversion-1.0.23/LICENSE-MIT) |
| ryu | 1.0.23 | `Apache-2.0 OR BSL-1.0` | [LICENSE-APACHE](licenses/ryu-1.0.23/LICENSE-APACHE), [LICENSE-BOOST](licenses/ryu-1.0.23/LICENSE-BOOST) |
| same-file | 1.0.6 | `Unlicense/MIT` | [COPYING](licenses/same-file-1.0.6/COPYING), [LICENSE-MIT](licenses/same-file-1.0.6/LICENSE-MIT), [UNLICENSE](licenses/same-file-1.0.6/UNLICENSE) |
| seq-macro | 0.3.6 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/seq-macro-0.3.6/LICENSE-APACHE), [LICENSE-MIT](licenses/seq-macro-0.3.6/LICENSE-MIT) |
| serde | 1.0.229 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/serde-1.0.229/LICENSE-APACHE), [LICENSE-MIT](licenses/serde-1.0.229/LICENSE-MIT) |
| serde_core | 1.0.229 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/serde_core-1.0.229/LICENSE-APACHE), [LICENSE-MIT](licenses/serde_core-1.0.229/LICENSE-MIT) |
| serde_derive | 1.0.229 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/serde_derive-1.0.229/LICENSE-APACHE), [LICENSE-MIT](licenses/serde_derive-1.0.229/LICENSE-MIT) |
| serde_json | 1.0.154 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/serde_json-1.0.154/LICENSE-APACHE), [LICENSE-MIT](licenses/serde_json-1.0.154/LICENSE-MIT) |
| sha2 | 0.10.9 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/sha2-0.10.9/LICENSE-APACHE), [LICENSE-MIT](licenses/sha2-0.10.9/LICENSE-MIT) |
| shlex | 2.0.1 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/shlex-2.0.1/LICENSE-APACHE), [LICENSE-MIT](licenses/shlex-2.0.1/LICENSE-MIT) |
| slab | 0.4.12 | `MIT` | [LICENSE](licenses/slab-0.4.12/LICENSE) |
| smallvec | 1.16.4 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/smallvec-1.16.4/LICENSE-APACHE), [LICENSE-MIT](licenses/smallvec-1.16.4/LICENSE-MIT) |
| snap | 1.1.2 | `BSD-3-Clause` | [COPYING](licenses/snap-1.1.2/COPYING) |
| syn | 2.0.119 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/syn-2.0.119/LICENSE-APACHE), [LICENSE-MIT](licenses/syn-2.0.119/LICENSE-MIT) |
| syn | 3.0.9 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/syn-3.0.9/LICENSE-APACHE), [LICENSE-MIT](licenses/syn-3.0.9/LICENSE-MIT) |
| tempfile | 3.27.0 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/tempfile-3.27.0/LICENSE-APACHE), [LICENSE-MIT](licenses/tempfile-3.27.0/LICENSE-MIT) |
| thiserror | 2.0.21 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/thiserror-2.0.21/LICENSE-APACHE), [LICENSE-MIT](licenses/thiserror-2.0.21/LICENSE-MIT) |
| thiserror-impl | 2.0.21 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/thiserror-impl-2.0.21/LICENSE-APACHE), [LICENSE-MIT](licenses/thiserror-impl-2.0.21/LICENSE-MIT) |
| thrift | 0.17.0 | `Apache-2.0` | В архиве нет отдельного файла |
| tiny-keccak | 2.0.2 | `CC0-1.0` | [LICENSE](licenses/tiny-keccak-2.0.2/LICENSE) |
| twox-hash | 2.1.5 | `MIT` | [LICENSE.txt](licenses/twox-hash-2.1.5/LICENSE.txt) |
| typenum | 1.20.1 | `MIT OR Apache-2.0` | [LICENSE](licenses/typenum-1.20.1/LICENSE), [LICENSE-APACHE](licenses/typenum-1.20.1/LICENSE-APACHE), [LICENSE-MIT](licenses/typenum-1.20.1/LICENSE-MIT) |
| unicode-ident | 1.0.27 | `(MIT OR Apache-2.0) AND Unicode-3.0` | [LICENSE-APACHE](licenses/unicode-ident-1.0.27/LICENSE-APACHE), [LICENSE-MIT](licenses/unicode-ident-1.0.27/LICENSE-MIT), [LICENSE-UNICODE](licenses/unicode-ident-1.0.27/LICENSE-UNICODE) |
| vcpkg | 0.2.15 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/vcpkg-0.2.15/LICENSE-APACHE), [LICENSE-MIT](licenses/vcpkg-0.2.15/LICENSE-MIT) |
| version_check | 0.9.5 | `MIT/Apache-2.0` | [LICENSE-APACHE](licenses/version_check-0.9.5/LICENSE-APACHE), [LICENSE-MIT](licenses/version_check-0.9.5/LICENSE-MIT) |
| walkdir | 2.5.0 | `Unlicense/MIT` | [COPYING](licenses/walkdir-2.5.0/COPYING), [LICENSE-MIT](licenses/walkdir-2.5.0/LICENSE-MIT), [UNLICENSE](licenses/walkdir-2.5.0/UNLICENSE) |
| wasi | 0.11.1+wasi-snapshot-preview1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/wasi-0.11.1+wasi-snapshot-preview1/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/wasi-0.11.1+wasi-snapshot-preview1/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/wasi-0.11.1+wasi-snapshot-preview1/LICENSE-MIT) |
| wasip2 | 1.0.4+wasi-0.2.12 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/wasip2-1.0.4+wasi-0.2.12/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/wasip2-1.0.4+wasi-0.2.12/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/wasip2-1.0.4+wasi-0.2.12/LICENSE-MIT) |
| wasm-bindgen | 0.2.129 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wasm-bindgen-0.2.129/LICENSE-APACHE), [LICENSE-MIT](licenses/wasm-bindgen-0.2.129/LICENSE-MIT) |
| wasm-bindgen-macro | 0.2.129 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wasm-bindgen-macro-0.2.129/LICENSE-APACHE), [LICENSE-MIT](licenses/wasm-bindgen-macro-0.2.129/LICENSE-MIT) |
| wasm-bindgen-macro-support | 0.2.129 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wasm-bindgen-macro-support-0.2.129/LICENSE-APACHE), [LICENSE-MIT](licenses/wasm-bindgen-macro-support-0.2.129/LICENSE-MIT) |
| wasm-bindgen-shared | 0.2.129 | `MIT OR Apache-2.0` | [LICENSE-APACHE](licenses/wasm-bindgen-shared-0.2.129/LICENSE-APACHE), [LICENSE-MIT](licenses/wasm-bindgen-shared-0.2.129/LICENSE-MIT) |
| winapi-util | 0.1.11 | `Unlicense OR MIT` | [COPYING](licenses/winapi-util-0.1.11/COPYING), [LICENSE-MIT](licenses/winapi-util-0.1.11/LICENSE-MIT), [UNLICENSE](licenses/winapi-util-0.1.11/UNLICENSE) |
| windows-core | 0.62.2 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-core-0.62.2/license-apache-2.0), [license-mit](licenses/windows-core-0.62.2/license-mit) |
| windows-implement | 0.60.2 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-implement-0.60.2/license-apache-2.0), [license-mit](licenses/windows-implement-0.60.2/license-mit) |
| windows-interface | 0.59.3 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-interface-0.59.3/license-apache-2.0), [license-mit](licenses/windows-interface-0.59.3/license-mit) |
| windows-link | 0.2.1 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-link-0.2.1/license-apache-2.0), [license-mit](licenses/windows-link-0.2.1/license-mit) |
| windows-result | 0.4.1 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-result-0.4.1/license-apache-2.0), [license-mit](licenses/windows-result-0.4.1/license-mit) |
| windows-strings | 0.5.1 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-strings-0.5.1/license-apache-2.0), [license-mit](licenses/windows-strings-0.5.1/license-mit) |
| windows-sys | 0.61.2 | `MIT OR Apache-2.0` | [license-apache-2.0](licenses/windows-sys-0.61.2/license-apache-2.0), [license-mit](licenses/windows-sys-0.61.2/license-mit) |
| wit-bindgen | 0.57.1 | `Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/wit-bindgen-0.57.1/LICENSE-APACHE), [LICENSE-Apache-2.0_WITH_LLVM-exception](licenses/wit-bindgen-0.57.1/LICENSE-Apache-2.0_WITH_LLVM-exception), [LICENSE-MIT](licenses/wit-bindgen-0.57.1/LICENSE-MIT) |
| zerocopy | 0.8.63 | `BSD-2-Clause OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/zerocopy-0.8.63/LICENSE-APACHE), [LICENSE-BSD](licenses/zerocopy-0.8.63/LICENSE-BSD), [LICENSE-MIT](licenses/zerocopy-0.8.63/LICENSE-MIT) |
| zerocopy-derive | 0.8.63 | `BSD-2-Clause OR Apache-2.0 OR MIT` | [LICENSE-APACHE](licenses/zerocopy-derive-0.8.63/LICENSE-APACHE), [LICENSE-BSD](licenses/zerocopy-derive-0.8.63/LICENSE-BSD), [LICENSE-MIT](licenses/zerocopy-derive-0.8.63/LICENSE-MIT) |
| zmij | 1.0.23 | `MIT` | [LICENSE-MIT](licenses/zmij-1.0.23/LICENSE-MIT) |
