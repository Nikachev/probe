# Руководство по HIL-тестированию (Hardware-in-the-Loop) отладчика rusty-probe-nicenano

В этом документе содержатся подробные инструкции по настройке, сборке и запуску автоматизированных аппаратных тестов для прошивки CMSIS-DAP отладчика **rusty-probe-nicenano** с использованием двух плат **nice!nano v2** (nRF52840).

---

## 1. Топология и Схема Подключения

Для проведения тестов используются две одинаковые платы nice!nano v2:
- **Плата A (Probe):** Прошита основной прошивкой `rusty-probe-nicenano` и выполняет роль отладчика CMSIS-DAP.
- **Плата B (Target):** Выполняет роль отлаживаемого устройства (Target MCU, nRF52840 Cortex-M4F).

### Распиновка подключения SWD

| Сигнал SWD | Pin платы A (Probe) | Pin/Pad платы B (Target) | Описание линии |
|---|---|---|---|
| **SWDCLK** | **`P0.17`** (`017`) | **`SWDCLK`** / `P0.17` | Тактовый сигнал SWD (Push-Pull выход отладчика) |
| **SWDIO** | **`P0.20`** (`020`) | **`SWDIO`** / `P0.20` | Двунаправленная линия данных SWD |
| **nRESET** | **`P0.22`** (`022`) | **`RESET`** / `P0.18` | Аппаратный сброс (Open-Drain, подтяжка к 3.3V) |
| **VCC** | **`VCC`** (3.3V) | **`VCC`** (3.3V) | Питание 3.3V целевой платы от отладчика |
| **GND** | **`GND`** | **`GND`** | Общий провод заземления (**Обязательно**) |

> ⚠️ **КРИТИЧЕСКОЕ ПРЕДУПРЕЖДЕНИЕ ПО БЕЗОПАСНОСТИ:**
> Питание целевой платы (Плата B) подается **СТРОГО на вывод `VCC` (3.3 V)**. 
> Ни в коем случае не подключайте вывод **`RAW`** (на нем присутствует 5 V от USB, что приведет к повреждению логики микроконтроллеров!).

---

## 2. Сборка Тестовых Прошивок (Target Binaries)

Перед запуском тестов необходимо скомпилировать специальные прошивки для ведомой платы B. Для этого в репозитории подготовлен автоматический скрипт:

```bash
./tools/build-test-targets.sh
```

Скрипт формирует следующие бинарные файлы в каталоге `tmp/test-targets/`:
1. `target_blinky` (`.elf`, `.bin`, `.uf2`) — прошивка с миганием светодиодом `P0.15` и статической памятью для проверки операций RAM.
2. `target_rtt` (`.elf`, `.bin`, `.uf2`) — прошивка с блоком `_SEGGER_RTT` для тестирования высокоскоростного отладочного вывода RTT.
3. `target_fault` (`.elf`, `.bin`, `.uf2`) — прошивка для отладки исключений (HardFault, Breakpoints) и проверки останова ядра.

---

## 3. Запуск Автоматизированных Тестов (29 Test Cases)

Тестовый ранер `tools/run_hil_tests.py` построен на базе клиентской обертки `ProbeRsClient` и поддерживает выборочный запуск тестов, переопределение параметров подключения и просмотр списка доступных тест-кейсов.

### Варианты запуска:

1. **Запуск всех 29 тестов:**
   ```bash
   ./tools/run_hil_tests.py
   ```

2. **Просмотр списка всех тест-кейсов:**
   ```bash
   ./tools/run_hil_tests.py --list
   ```

3. **Запуск конкретного набора тестов (Suite 1..7):**
   ```bash
   ./tools/run_hil_tests.py --suite 3
   ```

4. **Запуск конкретного теста по ID:**
   ```bash
   ./tools/run_hil_tests.py --test TS-301
   ```

5. **Автоматическая прошивка и запуск HIL-тестов (С программным сбросом):**
   ```bash
   ./tools/flash_now.py
   ```
   *Скрипт посылает 1200-baud DFU touch сигнал по USB CDC Serial, переводит плату в режим загрузчика (`/Volumes/NICENANO`), автоматически прошивает новую сборку `tmp/app.uf2` и запускает HIL-тесты.*

---

### Архитектура Тестового Фреймворка:

- **`ProbeRsClient`**: Модульный клиент-обертка над CLI `probe-rs`, автоматизирующий формирование аргументов (`--chip`, `--probe`), выполнение операций `read`, `write`, `reset`, `erase`, `download` и отслеживание таймингов исполнения.
- **Программный DFU Сброс**: В прошивку добавлена обработка 1200-baud touch и команд `dfu`/`bootloader`/`reset` на USB CDC Serial интерфейсе, автоматически выставляющая `POWER.GPREGRET = 0x57` и вызывающая `SCB::sys_reset()`.
- **Интеграционные тесты Rust**: Для локальной проверки исходного кода прошивки выполняется проверка корректности структур данных и сборки target-бинарников.

---



### Полная таблица проверяемых наборов тестов (Suites 1–7):

| Suite | ID Теста | Описание | Метод проверки |
|---|---|---|---|
| **Suite 1: USB & Identification** | **TS-101** | USB Device Enumeration | Поиск VID:PID `1209:4853` на USB шине |
| | **TS-102** | Unique Serial Number | Проверка 16-символьного hex серийного номера из FICR |
| | **TS-103** | DAP Capabilities Query | Проверка флагов возможностей CMSIS-DAP (SWD mode) |
| | **TS-104** | Target Chip ID | Вычитывание DP IDCODE (`0x2BA01477` для nRF52840) |
| | **TS-105** | CoreSight Discovery | Обнаружение компонентов FPB, DWT, ITM |
| **Suite 2: Bit-Bang SWD & Timing** | **TS-201** | Frequency Scaling | Проверка связи на тактовой частоте 100 кГц и 1000 кГц |
| | **TS-202** | SWDIO Direction Switch | Проверка переключения Push-Pull ⇄ Input при приеме ACK |
| | **TS-203** | ACK & Error Recovery | Попытка невалидного чтения `0xFFFFFFFF` и проверка восстановления |
| | **TS-204** | Line Reset Sequence | Генерация 50+ импульсов SWD Line Reset и JTAG-to-SWD (`0xE79E`) |
| **Suite 3: Memory Operations** | **TS-301** | Single Word RAM R/W | Запись и чтение 32-битного слова по адресу `0x20004000` |
| | **TS-302** | Sub-word RAM Access | Побайтовая запись `0xA5`, `0x5A` и полуслова `0x1234`, проверка `0x12345AA5` |
| | **TS-303** | Bulk Memory Transfer | Передача 1024 байт RAM с замером скорости (**10.36 КБ/с**) |
| | **TS-304** | Flash Read Boundary | Вычитывание векторов прерываний Bootloader (`0x00000000`) |
| **Suite 4: Execution Control** | **TS-401** | CPU Halt & Status | Остановка ядра целевой платы B (`C_HALT = 1`) |
| | **TS-402** | Register Read/Write | Запись и вычитывание состояний памяти и регистров CPU |
| | **TS-403** | Single Step Execution | Пошаговое выполнение инструкций (`C_STEP = 1`) |
| | **TS-404** | Hardware Breakpoints | Проверка блока аппаратных точек останова FPB |
| | **TS-405** | Watchpoints via DWT | Проверка блока точек контроля записи DWT |
| | **TS-406** | CPU Resume | Возобновление работы ядра (`C_HALT = 0`) |
| **Suite 5: Flash Programming** | **TS-501** | Sector Erase | Стирание сектора 4096 байт и проверка заполнения `0xFFFFFFFF` |
| | **TS-502** | Full Binary Flashing | Прошивка `target_blinky.elf` с измерением скорости (**165.98 КБ/с**) |
| | **TS-503** | Flash Verification | Побайтовая верификация памяти с флагом `--verify` |
| | **TS-504** | Mass Erase Protection | Проверка целостности сектора приложений и загрузчика |
| **Suite 6: Reset Control** | **TS-601** | Hardware nRESET | Подача физического импульса сброса по линии `P0.22` |
| | **TS-602** | Software SYSRESETREQ | Инициирование программного сброса через `AIRCR` |
| | **TS-603** | Vector Catch | Перехват вектора сброса `VC_CORERESET` в `DEMCR` |
| **Suite 7: RTT Streaming** | **TS-701** | RTT Buffer Auto-Detect | Автоопределение символа `_SEGGER_RTT` в `target_rtt.elf` |
| | **TS-702** | Up-Buffer Streaming | Высокоскоростное чтение логов с целевой платы |
| | **TS-703** | Down-Buffer Injection | Инжекция команд в RTT Down-Buffer 0 |

---

## 4. Результаты Выполнения HIL-Тестов (29/29 PASS)

```text
==========================================================
 Running Complete Rigorous HIL Test Suite for rusty-probe
==========================================================
[✅ PASS] TS-101: USB Device Enumeration (VID:PID 1209:4853) (0.07s)
[✅ PASS] TS-102: Unique Serial Number Verification (FICR DEVICEID) (0.02s)
[✅ PASS] TS-103: CMSIS-DAP Capabilities Query (SWD Mode) (0.02s)
[✅ PASS] TS-104: Target Chip Detection & IDCODE (nRF52840 0x2BA01477) (0.29s)
[✅ PASS] TS-105: ARM CoreSight Component Discovery (FPB, DWT, ITM) (0.31s)
[✅ PASS] TS-201: SWD Frequency Scaling (100 kHz & 1000 kHz) (0.31s)
[✅ PASS] TS-202: SWDIO Dynamic Direction Switch Verification (0.10s)
[✅ PASS] TS-203: ACK Verification & Negative Error Recovery (0.09s)
[✅ PASS] TS-204: Line Reset & JTAG-to-SWD Sequence (0xE79E) (0.09s)
[✅ PASS] TS-301: Single Word RAM Read/Write (0x20004000) (0.09s)
[✅ PASS] TS-302: Sub-word & Byte Level RAM Masking (0.09s)
[✅ PASS] TS-303: Bulk Memory Transfer & CRC (1024 Bytes) (0.10s) [10.36 KB/s]
[✅ PASS] TS-304: Flash Read Boundary Test (Vector Table 0x00000000) (0.09s)
[✅ PASS] TS-401: CPU Halt & Status Check (DHCSR C_HALT) (0.09s)
[✅ PASS] TS-402: Register Read/Write & Memory State Control (0.09s)
[✅ PASS] TS-403: Single Step Execution (C_STEP) (0.09s)
[✅ PASS] TS-404: Hardware Breakpoints via FPB Component (0.29s)
[✅ PASS] TS-405: Watchpoints via DWT Component (0.32s)
[✅ PASS] TS-406: CPU Resume & Running State Transition (0.11s)
[✅ PASS] TS-501: Sector Erase & Blank Check (4096-byte Page) (0.11s)
[✅ PASS] TS-502: Full Binary Flashing (target_blinky.elf) (0.92s) [165.98 KB/s]
[✅ PASS] TS-503: Flash Verification (--verify Byte-for-Byte) (1.24s)
[✅ PASS] TS-504: Mass Erase Recovery & Bootloader Protection (0.09s)
[✅ PASS] TS-601: Hardware nRESET Line Pulse (Open-Drain P0.22) (0.09s)
[✅ PASS] TS-602: Software SYSRESETREQ (AIRCR Register) (0.09s)
[✅ PASS] TS-603: Reset and Halt / Vector Catch (DEMCR VC_CORERESET) (0.18s)
[✅ PASS] TS-701: RTT Buffer Auto-Detection (_SEGGER_RTT Symbol) (0.80s)
[✅ PASS] TS-702: Up-Buffer High-Speed Streaming (target_rtt.elf) (2.00s)
[✅ PASS] TS-703: Down-Buffer Command Injection & Echo Channel (0.50s)
----------------------------------------------------------
Summary: 29/29 tests passed.
```

---

## 5. Ручная Проверка через `probe-rs`

Вы также можете выполнить любую операцию отладки вручную:

1. **Список найденных отладчиков:**
   ```bash
   probe-rs list
   ```
2. **Информация о целевом процессоре:**
   ```bash
   probe-rs info --chip nRF52840_xxAA --probe 1209:4853
   ```
3. **Прошивка целевой платы:**
   ```bash
   probe-rs download --chip nRF52840_xxAA --probe 1209:4853 tmp/test-targets/target_blinky.elf
   ```
4. **Запуск и RTT логгирование:**
   ```bash
   probe-rs run --chip nRF52840_xxAA --probe 1209:4853 tmp/test-targets/target_rtt.elf --rtt-scan-memory
   ```
