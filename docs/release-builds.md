# TAVRA Preview: сборка и установка

Development preview общего Flutter-приложения с режимами Admin/Client, control API и node management agent. VPN runtime к UI ещё не подключён. Установщик не означает работающий VPN или готовность полного продукта. Отдельный экспериментальный Linux `tavra-link-cli` включён в архив для изолированного TUN-теста; это ещё не multi-user VPN daemon.

[Опубликованный Preview1](https://github.com/DenKedr/tavra-vpn/releases/tag/v0.1.0-preview.1): CI40413857045 прошёл все6jobs. macOS app universal arm64+x86_64, структура codesign/PKG/DMG проверена; Apple DeveloperID/notarization отсутствуют. Windows Setup прошёл тихую установку/удаление. APK integrity/apksigner прошли, установка на Android-устройство пока не проверена. Более новые исходники inventory CRUD не входят в Preview1.

## Проверяемые gates

- `Rust and Flutter checks`: `cargo fmt --all --check`, Clippy с `-D warnings`, `cargo test --workspace --locked` на Rust **1.99.1**. PostgreSQL 16 — одноразовый service container; `TAVRA_TEST_DATABASE_URL` только для тестовой БД с синтетическими данными. Тесты очищают таблицы: никогда не подставлять рабочую БД.
- Flutter **3.47.8**, Dart **3.13.7**: lockfile, analyze, test.
- `Preview installers (manual)` сначала вызывает эти проверки, затем собирает настоящие файлы. Workflow не создаёт GitHub Release и не подключается к внешним серверам.
- Linux: сборка Ubuntu 22.04 amd64; DEB install/remove, синтаксис systemd, отказ запуска без конфигурации — в одноразовых Ubuntu 22.04/24.04/26.04 контейнерах. Это не полноценный тест systemd boot/VPN. arm64 DEB ещё не проверен и не публикуется workflow.
- Windows 2022: Release Flutter, unsigned Inno Setup EXE, silent install/uninstall smoke. Интерактивный UI и Windows 10 требуют отдельной проверки.
- macOS 15 с явным Xcode 16.4 / Swift 6.1: Flutter/Xcode Release, ad-hoc подпись приложения, unsigned PKG/DMG. Это не Developer ID/notarization. Архитектура runner в `BUILD-INPUTS.json`; universal binary/macOS 13 не подтверждены. Gatekeeper может запретить открытие; не отключать защиту глобально.
- Android: arm64 debug APK с development key runner, проверка ZIP и подписи APK; fingerprint в `ANDROID-SIGNER.txt`. Не production signing. Device/emulator install и реальный VPN — отдельные gates.

## Получение артефактов

GitHub `DenKedr/tavra-vpn`: **Actions → Preview installers (manual) → Run workflow** на проверяемом commit. Скачать `TAVRA-Preview-*` из успешного запуска:

```sh
gh workflow run preview-installers.yml --repo DenKedr/tavra-vpn --ref main
gh run list --repo DenKedr/tavra-vpn --workflow preview-installers.yml
gh run download RUN_ID --repo DenKedr/tavra-vpn --dir downloaded-preview
```

Наборы содержат `SHA256SUMS` точных выходных файлов и `BUILD-INPUTS.json` с commit, архитектурой и SHA-256 lockfile/скриптов. Linux также содержит source tar из `git archive HEAD` и CycloneDX 1.5 `dependency-inventory.cdx.json` по Cargo metadata/Dart lock. Это source dependency inventory; он не охватывает ОС, Flutter engine и транзитивные зависимости инструментов упаковки. Полный runtime SBOM остаётся release gate. Manifest не подписан: checksum проверяет целостность, но сам по себе не устанавливает автора.

```sh
cd downloaded-preview/TAVRA-Preview-Linux-amd64-source
sha256sum -c SHA256SUMS
```

Action SHA проверены через GitHub REST API 27.10.2026: checkout v6 `d23441a48e516b6c34aea4fa41551a30e30af803`, upload-artifact v6 `b7c566a772e6b6bfb58ed0dc250532a479d7789f`, flutter-action v2 `1a449444c387b1966244ae4d4f8c696479add0b2`. Образы ОС/PostgreSQL и Inno Setup из Chocolatey обновляются upstream; byte-identical rebuild всего toolchain пока не гарантируется.

## DEB: сборка на Linux

```sh
rustup toolchain install 1.99.1 --profile minimal
cargo +1.99.1 build --release --locked -p tavra-control -p tavra-node
scripts/release-build-deb.sh control 0.1.0 amd64
scripts/release-build-deb.sh node 0.1.0 amd64
scripts/release-verify-deb.sh --containers
python3 scripts/release-provenance.py
python3 scripts/release-manifest.py
```

Нужны `dpkg-deb`, `readelf`, Docker для контейнерного gate. На M1 использовать CI; скрипт проверяет ELF architecture. Пакетные пути: `/opt/tavra`, `/etc/tavra`, `/lib/systemd/system/tavra-*.service`, `/usr/share/doc/tavra-*`; state `/var/lib/tavra`, системный пользователь/группа `tavra`. `owned-files.txt` перечисляет поставляемые файлы. Пакеты не содержат ключей/паролей, не обновляют ОС, не меняют чужие службы/firewall/SSH, не перезагружают сервер.

## Control

Инструкция оператору после обследования целевого сервера. Сначала сохранить конфигурацию, проверенную резервную копию БД и предыдущий DEB для отката.

```sh
sudo apt install ./tavra-control_0.1.0_amd64.deb
sudo install -o root -g tavra -m 0640 /etc/tavra/control.example /etc/tavra/control.env
sudoedit /etc/tavra/control.env
```

Предварительно создать отдельную PostgreSQL БД и владельца с правами на миграции. Внести настоящий `TAVRA_DATABASE_URL`; пример не работает без замены, superuser не использовать. PostgreSQL автоматически не устанавливается. На старте control применяет встроенные SQLx-миграции. Откат бинарника не отменяет схему; восстановление БД планируется отдельно.

Сохранить `TAVRA_BIND=127.0.0.1:8787`. Публичный TLS reverse proxy — отдельная настройка после обследования хоста. Для первоначального admin создать локально случайный секрет >=32 байт, задать `TAVRA_BOOTSTRAP_SECRET`; после bootstrap удалить его и перезапустить службу. Секреты — только в operator-owned envfile, вне Git/артефактов.

```sh
sudo systemd-analyze verify /lib/systemd/system/tavra-control.service
sudo systemctl enable --now tavra-control
sudo systemctl status tavra-control --no-pager
curl --fail http://127.0.0.1:8787/v1/health
```

Установка **не запускает и не включает** службу; конфигурация меняется с явным restart оператора. Логи: `journalctl -u tavra-control`, не публиковать чувствительные данные.

## Node: обязательный mTLS

```sh
sudo apt install ./tavra-node_0.1.0_amd64.deb
sudo install -o root -g tavra -m 0640 /etc/tavra/node.example /etc/tavra/node.json
sudoedit /etc/tavra/node.json
```

Доставить отдельно node server certificate, private key и отдельный management-client CA по путям JSON. Ключ root:tavra `0640`; доступ к каталогу для `tavra`. Management client certificate/key хранятся у доверенного controller/оператора, не в DEB. Node peer CA не заменяет management CA. Default bind `127.0.0.1:9443`. Unit проверяет config перед стартом, работает без root/сетевых capabilities. Агент предоставляет mTLS health/metrics, не устанавливает VPN и не управляет чужими процессами.

```sh
sudo -u tavra /opt/tavra/bin/tavra-node check-config --config /etc/tavra/node.json
sudo systemd-analyze verify /lib/systemd/system/tavra-node.service
sudo systemctl enable --now tavra-node
# Отдельные operator certificates; адрес должен совпадать с SAN.
curl --fail --cacert node-server-ca.crt --cert management-client.crt --key management-client.key https://localhost:9443/healthz
```

Без клиентского сертификата TLS должен отклонить запрос. Существующие VPN-профили/ключи не используются автоматически.

## Удаление и откат

`sudo apt remove tavra-control tavra-node` останавливает только свои службы и удаляет пакетные файлы. Даже purge сохраняет operator-created env/JSON/certificates, БД, `/var/lib/tavra`, системную учётную запись: удаление — отдельное решение оператора. Manifest: `/usr/share/doc/tavra-control/owned-files.txt`. Для отката остановить затронутую службу, установить предыдущий DEB, восстановить совместимые config/БД из проверенной копии, запустить и проверить health. Автообновления/обратная миграция не реализованы.

Источники: [GitHub PostgreSQL services](https://docs.github.com/en/actions/tutorials/use-containerized-services/create-postgresql-service-containers), [Inno Setup reference](https://jrsoftware.org/ishelp/), [Flutter deployment](https://docs.flutter.dev/deployment).
