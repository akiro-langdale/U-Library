# U-Library

Библиотека для исследования доступных адресных пространств. U-Monitor — отдельное приложение, которое сможет использовать её публичный API.

Первый источник данных — обычный файл. `FileSource::open` открывает его для чтения, а `read_at` возвращает фактическое число прочитанных байтов, смещение, тип адресного пространства и время завершения чтения. Запрос ограничен 1 МиБ. На конце файла возвращается короткое чтение, включая ноль байтов.

Для вызова из C и C++ подключите [u_library.h](include/u_library.h). Для FFI других языков используйте те же сигнатуры и соглашение о вызовах C. Команда `cargo build -p u-library-ffi --release` создаёт динамическую библиотеку в `target/release`: `u_library.dll` (Windows), `libu_library.so` (Linux) или `libu_library.dylib` (macOS). На Windows для линковки из MSVC используйте `u_library.dll.lib`.

`ul_abi_version()` возвращает версию ABI (сейчас 1). Передайте путь в UTF-8 и его длину в байтах в `ul_file_open`. Читайте в собственный буфер через `ul_file_read_at` и используйте только фактически прочитанные байты. Для секторного просмотра используйте `ul_file_size` и `ul_file_read_sectors` (размер сектора `0` означает 512 байт). Закройте каждый успешно открытый дескриптор через `ul_file_close` после завершения всех операций с ним. Полный контракт ошибок и указателей приведён в заголовке.

План модулей и границы U-Monitor: [ROADMAP.md](ROADMAP.md).

Проверка рабочего пространства: `cargo test --workspace`.

## Поиск по байтам

Rust-функция `u_library_core::find_byte_offsets(bytes, pattern)` возвращает
смещения всех точных совпадений в переданном буфере, включая перекрывающиеся.
Для пустого шаблона итератор пуст.

## Снимки и сравнение

`MemorySnapshot::capture` сохраняет адресное пространство, базовый адрес и
байты одного чтения. `FileSource::snapshot_at` и `ProcessMemory::snapshot`
собирают снимок сразу после чтения. `compare_snapshots` и `compare_bytes`
возвращают только изменившиеся адреса с прежним и новым значением. Снимки
с разным пространством, базой или длиной отклоняются.

В C ABI сравнение буферов доступно через `ul_bytes_compare`. Передайте
`NULL` и нулевую ёмкость, чтобы узнать число отличий, затем выделите массив
`UL_ByteDiff` и повторите вызов.

## Виртуальная память процессов

Модуль Rust `u_library_memory::process_memory` открывает процесс по PID через
`ProcessMemory::open`. Сессия запрашивает только права на чтение и получение
карты. Дескриптор закрывается при освобождении сессии.

Платформы:
- Windows — `OpenProcess` / `VirtualQueryEx` / `ReadProcessMemory`
- Linux — `/proc/<pid>/maps` и `/proc/<pid>/mem`
- macOS — Mach VM; чужой процесс обычно требует elevated rights (`task_for_pid`)

`regions` возвращает виртуальные диапазоны с состоянием, типом и защитой.
`read(address, length)` ограничивает длину 512 байтами. Запрос должен целиком
помещаться в один читаемый выделенный диапазон без guard-защиты. При ошибке
или неполном чтении байты не возвращаются. Для текущего процесса доступны
`current_process_regions` и `read_current_process`.

Расширение C ABI для памяти процессов описано в [u_library.h](include/u_library.h).
`ul_process_abi_version()` возвращает 1 на Windows, Linux и macOS, и 0 там,
где расширение недоступно. Версия базового файлового ABI по-прежнему 1.

Откройте процесс через `ul_process_open`. Для карты вызовите
`ul_process_regions` с нулевой ёмкостью, выделите массив из указанного числа
записей и повторите запрос. Если карта выросла, увеличьте массив по новому
значению `count` и повторите вызов. `ul_process_read_at` читает от 1 до 512 байт
в буфер вызывающей стороны. Закройте дескриптор через `ul_process_close`.

## Примеры для Windows x64

Примеры открывают собственный процесс, получают карту и проверяют чтение
массива `55 00 FF 4C`. Количество регионов зависит от процесса. Используйте
DLL и компиляторы той же архитектуры, что и запускаемый пример.

В PowerShell для разработчиков Visual Studio с инструментами x64 из корня
U-Library выполните:

```powershell
cargo build -p u-library-ffi
Push-Location target/debug

cl.exe /nologo /std:c11 /I../../include /Fe:example-c.exe ../../examples/process_memory.c u_library.dll.lib
.\example-c.exe

cl.exe /nologo /std:c++17 /I../../include /Fe:example-cpp.exe /Tp../../examples/process_memory.c u_library.dll.lib
.\example-cpp.exe

& "$env:WINDIR/Microsoft.NET/Framework64/v4.0.30319/csc.exe" /nologo /platform:x64 /out:example-cs.exe ../../examples/ProcessMemory.cs
.\example-cs.exe

python ../../examples/process_memory.py u_library.dll
Pop-Location
```

Для Python нужен 64-битный Python 3. Пример использует только стандартную
библиотеку. C# загружает DLL из папки с исполняемым файлом.

## Кэш CPU

Rust API `u_library_cache` обнаруживает иерархию кэша на Windows и Linux
(`discover`) и при необходимости снимает аппаратные счётчики references/misses
через `perf_event_open` на Linux (`sample_counters`). Содержимое кэш-линий не
читается и не симулируется.

C ABI: `ul_cache_abi_version`, `ul_cache_topology` (сначала count-only) и
`ul_cache_counters`. Неизвестные метрики узла — `size_bytes == -1`, нулевые поля
или `UL_CACHE_METRIC_UNAVAILABLE` для счётчиков.

## Регистры CPU

Rust API `u_library_registers` снимает GPR текущего потока на Windows x86-64
(`capture_current_thread`) и регистры другого потока на Windows x86-64 / Linux
(`capture_thread`) после явного согласия вызывающей стороны. На Linux
используется `ptrace`; на неподдерживаемых сочетаниях OS/архитектуры
возвращается `Unsupported`.

C ABI: `ul_registers_abi_version`, `ul_registers_capture_current` и
`ul_registers_capture_thread`. Как и для карты процессов, сначала запросите
число записей с нулевой ёмкостью, затем повторите вызов с массивом
`UL_Register`.

## GPU adapters

Rust API `u_library_gpu::list_devices` перечисляет адаптеры на Windows через DXGI
и сообщает имя, vendor/device id и dedicated/shared bytes, если OS их отдаёт.
Произвольное содержимое VRAM не читается. На других ОС возвращается `Unsupported`.

C ABI: `ul_gpu_abi_version` и `ul_gpu_list` (count-only, затем заполнение
`UL_GpuDevice`; `UINT64_MAX` означает неизвестный объём).

## Адаптеры для игровых движков

Для Windows x64 доступны адаптеры для Unity, Unreal Engine 5 и Godot 4 .NET.
См. [установку и сценарии проверки](adapters/README.md).
