# Адаптеры U-Library для игровых движков

Инструкции рассчитаны на Windows x64. Все адаптеры используют файловый C ABI v1
и Windows-расширение Process ABI v1 из [u_library.h](../include/u_library.h).
Файловые вызовы принимают обычный путь ОС в UTF-8, смещение и длину до 1 МиБ.
Доступны короткое чтение и пустой результат на EOF. Вызовы памяти принимают
PID и возвращают карту регионов или читают 1–512 байт из одного читаемого региона.
Операции синхронные. Дескриптор закрывается в пределах каждого вызова.
Если между получением карты и чтением процесс завершится или карта изменится,
обработайте статус ABI и при необходимости запросите карту заново.

Соберите DLL из корня U-Library в PowerShell для разработчиков Visual Studio x64.

```powershell
cargo build --locked --release --target x86_64-pc-windows-msvc -p u-library-ffi
```

DLL и импортная библиотека находятся в `target/x86_64-pc-windows-msvc/release`.
Берите DLL и `u_library.dll.lib` из одной сборки.

## Unity

1. Скопируйте [ULibraryClient.cs](dotnet/ULibraryClient.cs) в `Assets/ULibrary`,
   а `u_library.dll` в `Assets/Plugins/x86_64`. В Plugin Inspector выберите
   Windows x86_64 для Editor и Windows Standalone. [Документация Unity](https://docs.unity3d.com/2022.3/Documentation/Manual/PluginInspector.html).
2. Для проверки добавьте в `Assets/ULibrary` файлы [ULibrarySmoke.cs](dotnet/ULibrarySmoke.cs)
   и [ULibraryUnitySmoke.cs](unity/ULibraryUnitySmoke.cs). Прикрепите
   `ULibraryUnitySmoke` к активному GameObject и запустите сцену.
3. Проверьте сообщение `U-Library OK. Regions: … Bytes: 55 00 FF 4C` в Console.
   Повторите запуск в Windows x64 Player, включая IL2CPP, если проект его использует.

## Godot 4 .NET

1. Откройте C#-проект в .NET-редакторе Godot с соответствующим .NET SDK.
   [Документация Godot](https://docs.godotengine.org/en/stable/tutorials/scripting/c_sharp/index.html).
2. Скопируйте [ULibraryClient.cs](dotnet/ULibraryClient.cs) в проект и DLL
   в `native/win-x64/u_library.dll` относительно `.csproj`. Добавьте в `.csproj`
   группу, чтобы DLL попадала рядом с управляемой сборкой.

   ```xml
   <ItemGroup>
     <None Update="native/win-x64/u_library.dll"
           TargetPath="u_library.dll"
           CopyToOutputDirectory="PreserveNewest"
           CopyToPublishDirectory="PreserveNewest" />
   </ItemGroup>
   ```

3. Добавьте [ULibrarySmoke.cs](dotnet/ULibrarySmoke.cs) и
   [ULibraryGodotSmoke.cs](godot/ULibraryGodotSmoke.cs). Прикрепите
   `ULibraryGodotSmoke.cs` к узлу `Node` и запустите сцену. В Output должно
   появиться `U-Library OK. Regions: … Bytes: 55 00 FF 4C`.
4. При экспорте Windows x64 проверьте, что DLL лежит рядом с управляемой сборкой
   в экспортированных данных .NET, и запустите ту же сцену в экспортированной игре.

В обоих C#-проектах вызовы идут через `ULibrary.Client.ReadFile`,
`GetRegions` и `ReadProcess`. Ошибки ABI доступны в
`ULibrary.NativeException.Status`. `Client.ProcessAbiVersion` возвращает 0
при отсутствии расширения, в том числе в старой DLL без символа версии процесса.
В Unity пути из `StreamingAssets`, а в Godot виртуальные пути `res://` и `user://`
должны указывать на доступные ОС файлы. Для ресурсов внутри архива сначала
извлеките файл в доступный каталог.

## Unreal Engine 5

1. Скопируйте каталог [ULibrary](unreal/ULibrary) в `<Project>/Plugins/ULibrary`.
   Внутри плагина разместите файлы по следующим путям.

   | Файл | Путь от корня плагина |
   | --- | --- |
   | Публичный заголовок | `Source/ThirdParty/include/u_library.h` |
   | Импортная библиотека | `Source/ThirdParty/lib/Win64/u_library.dll.lib` |
   | DLL | `Source/ThirdParty/bin/Win64/u_library.dll` |

2. Включите плагин, соберите проект для Win64 и откройте редактор. UnrealBuildTool
   копирует DLL в `Binaries/ThirdParty/Win64` плагина и включает её в пакет.
   [Документация Epic](https://dev.epicgames.com/documentation/unreal-engine/integrating-third-party-libraries-into-unreal-engine).
3. В C++ добавьте зависимость `ULibrary` в `.Build.cs`. Перед вызовом функций
   из `ULibraryClient.h` загрузите `FULibraryModule` через `FModuleManager`
   и проверьте `IsReady()`. `ULibrary::ReadFile`, `GetRegions` и `ReadProcess`
   возвращают `UL_Status` и записывают результат в переданный вектор.
   После замены DLL перезапустите редактор.
4. Выполните `ULibrary.Smoke` в Output Log. Проверьте
   `U-Library OK. Regions: … Bytes: 55 00 FF 4C`. Повторите в упакованной
   Development-сборке через игровую консоль.

