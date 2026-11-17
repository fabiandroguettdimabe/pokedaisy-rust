# Proveniencia de cores locales de Vyntra Desktop

Auditoría puntual realizada el 2026-11-17. Se compararon los SHA-256 de `desktop-rust/cores/` con artefactos servidos por los canales oficiales; la versión que un core declara por libretro, por sí sola, no acredita de dónde salió el binario.

## mGBA

- `mgba_libretro.dll`, `pokedaisy_gbc_libretro.dll` y `vyntra_gbc_libretro.dll` tienen SHA-256 `D775ECEF6DA1DA5489FF2E0A37C13826B510C73711F89E78CB8D757C6F8C1426`.
- Ese hash coincide byte por byte con `mgba_libretro.dll` extraído de [`mgba_libretro.dll.zip` del buildbot oficial Libretro para Windows x86_64](https://buildbot.libretro.com/nightly/windows/x86_64/latest/mgba_libretro.dll.zip), descargado durante esta auditoría. El ZIP descargado tuvo SHA-256 `A7AE453B739D3F9117537E8CA7275ECEBD923D9035B52435F556470BF58A268F`; el índice oficial mostraba la compilación fechada 2026-11-16 07:19 UTC.
- `retro_get_system_info` declara `mGBA`, versión `0.11-212-7a12d6d` y extensiones `gba|gb|gbc|sgb`. El sufijo corresponde al commit [`libretro/mgba@7a12d6d4b9acb14c0ae62c9166b6a2f3d08007f6`](https://github.com/libretro/mgba/commit/7a12d6d4b9acb14c0ae62c9166b6a2f3d08007f6).
- El core local pasó el probe, cargó la ROM homebrew `PDTE` con `open_game`, produjo video, completó serialize/unserialize y guardó/restauró SRAM mediante manifiesto y bundle Vault. La licencia publicada por upstream mGBA está en [`LICENSE`](https://github.com/mgba-emu/mgba/blob/master/LICENSE); conserva también los avisos distribuidos con el artefacto.

Los dos nombres GBC son copias exactas del mismo core mGBA; el nombre del archivo no indica un adaptador personalizado.

## melonDS DS

- `melondsds_libretro.dll`, `pokedaisy_ds_libretro.dll` y `vyntra_ds_libretro.dll` tienen SHA-256 `6BE427125CAE551DA75680F85609CFB1CBAAFAFEF1CEBEBCC5E0E55A85E0FF41`.
- El core local declara `melonDS DS` versión `1.4.0`. Esa versión existe en el [release oficial v1.4.0](https://github.com/JesseTG/melonds-ds/releases/tag/v1.4.0), commit `f394adb`.
- El binario local **no** coincide con las DLL de ese release para Windows x86_64: Release `C7E6B58C667075A2A9868F2C3BA7A98E24200410A6DA3051A7C555F1F0EEF8EF`, Debug `F1813A96092545B239528749582AD7BDE759C5334893C87545B6B25CDAF40C44` y RelWithDebInfo `EA103FE0527BB589B402D822D12A9C7837C1A11EE4426557C8FDE3E7BD1B1603`. Tampoco coincide con el core del buildbot nightly descargado durante la auditoría (`3DB74A454A7CA3BA5E6D4C3954D72ECBF34DC9DED7DDA9B14F25272F3531C770`).
- El ZIP oficial Release publicado por GitHub (`melondsds_libretro-win32-x86_64-Release.zip`) tuvo SHA-256 `F03AF0318FEE91A83880F438B369A1368FEA6B5C1524A09DDE849CA8FA68603F`; su DLL oficial sí pasó `open_game`, produjo video y completó serialize/unserialize con la ROM homebrew `DSPT`. Ese resultado valida el artefacto oficial probado, pero no atribuye el binario local de hash `6BE4…` al mismo build. El ZIP incluye `melondsds-LICENSE.txt`; la composición y los avisos deben conservarse si se distribuye el artefacto oficial.

## Decisión de distribución

El core mGBA local está identificado por coincidencia exacta con el artefacto del buildbot y el hash queda fijado arriba. La versión declarada del melonDS local coincide con una versión upstream, pero su binario no coincide con las compilaciones oficiales comparadas; su procedencia sigue **sin verificar**. No incluir ese binario local ni tratar sus tres nombres como cores distintos en paquetes publicados. Se puede usar el artefacto oficial verificado de v1.4.0 en validaciones separadas, sujeto a revisar y conservar sus avisos/licencias.
