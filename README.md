# U-Library

Библиотека для исследования доступных адресных пространств. U-Monitor — отдельное приложение, которое сможет использовать её публичный API.

Первый источник данных — обычный файл. `FileSource::open` открывает его для чтения, а `read_at` возвращает фактическое число прочитанных байтов, смещение, тип адресного пространства и время завершения чтения. Запрос ограничен 1 МиБ. На конце файла возвращается короткое чтение, включая ноль байтов.

Для вызова из C и C++ подключите [u_library.h](include/u_library.h). Для FFI других языков используйте те же сигнатуры и соглашение о вызовах C. Команда `cargo build -p u-library-ffi --release` создаёт динамическую библиотеку в `target/release`: `u_library.dll` (Windows), `libu_library.so` (Linux) или `libu_library.dylib` (macOS). На Windows для линковки из MSVC используйте `u_library.dll.lib`.

`ul_abi_version()` возвращает версию ABI (сейчас 1). Передайте путь в UTF-8 и его длину в байтах в `ul_file_open`. Читайте в собственный буфер через `ul_file_read_at` и используйте только фактически прочитанные байты. Закройте каждый успешно открытый дескриптор через `ul_file_close` после завершения всех операций с ним. Полный контракт ошибок и указателей приведён в заголовке.

Проверка рабочего пространства: `cargo test --workspace`.

## Поиск по байтам

Rust-функция `u_library_core::find_byte_offsets(bytes, pattern)` возвращает
смещения всех точных совпадений в переданном буфере, включая перекрывающиеся.
Для пустого шаблона итератор пуст.

## Виртуальная память процессов Windows

Модуль Rust `u_library_memory::process_memory` открывает процесс по PID через
`ProcessMemory::open`. Сессия запрашивает только права на чтение и получение
карты. Дескриптор закрывается при освобождении сессии.

`regions` возвращает виртуальные диапазоны с состоянием, типом и защитой.
`read(address, length)` ограничивает длину 512 байтами. Запрос должен целиком
помещаться в один читаемый выделенный диапазон без guard-защиты. При ошибке
или неполном чтении байты не возвращаются. Для текущего процесса доступны
`current_process_regions` и `read_current_process`.

Расширение C ABI для памяти процессов описано в [u_library.h](include/u_library.h).
`ul_process_abi_version()` возвращает 1 на Windows и 0 на платформах,
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

## Адаптеры для игровых движков

Для Windows x64 доступны адаптеры для Unity, Unreal Engine 5 и Godot 4 .NET.
См. [установку и сценарии проверки](adapters/README.md).
