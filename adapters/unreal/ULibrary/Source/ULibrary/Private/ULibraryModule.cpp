#include "ULibraryModule.h"
#include "ULibraryClient.h"

#include "CoreMinimal.h"
#include "HAL/FileManager.h"
#include "HAL/IConsoleManager.h"
#include "HAL/PlatformProcess.h"
#include "Interfaces/IPluginManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Modules/ModuleManager.h"

DEFINE_LOG_CATEGORY_STATIC(LogULibrary, Log, All);

void FULibraryModule::StartupModule()
{
    const auto Plugin = IPluginManager::Get().FindPlugin(TEXT("ULibrary"));
    if (!Plugin.IsValid())
        return;
    const FString Path = FPaths::ConvertRelativePathToFull(FPaths::Combine(
        Plugin->GetBaseDir(), TEXT("Binaries/ThirdParty/Win64/u_library.dll")));
    void* Library = FPlatformProcess::GetDllHandle(*Path);
    if (Library)
    {
        const TCHAR* RequiredExports[] = {
            TEXT("ul_abi_version"), TEXT("ul_file_open"), TEXT("ul_file_read_at"),
            TEXT("ul_file_close"), TEXT("ul_process_abi_version"), TEXT("ul_process_open"),
            TEXT("ul_process_regions"), TEXT("ul_process_read_at"), TEXT("ul_process_close")
        };
        bool bComplete = true;
        for (const TCHAR* Name : RequiredExports)
            bComplete &= FPlatformProcess::GetDllExport(Library, Name) != nullptr;

        const auto FileVersion = reinterpret_cast<decltype(&ul_abi_version)>(
            FPlatformProcess::GetDllExport(Library, TEXT("ul_abi_version")));
        const auto ProcessVersion = reinterpret_cast<decltype(&ul_process_abi_version)>(
            FPlatformProcess::GetDllExport(Library, TEXT("ul_process_abi_version")));
        if (bComplete && FileVersion() == 1 && ProcessVersion() == 1)
        {
            NativeLibrary = Library;
            return;
        }

        FPlatformProcess::FreeDllHandle(Library);
    }
    UE_LOG(LogULibrary, Error, TEXT("Could not load U-Library file and process ABI v1 from %s"), *Path);
}

static void RunULibrarySmoke()
{
    const auto* Module = FModuleManager::GetModulePtr<FULibraryModule>(TEXT("ULibrary"));
    if (!Module || !Module->IsReady())
    {
        UE_LOG(LogULibrary, Error, TEXT("U-Library is unavailable."));
        return;
    }

    const TArray<uint8> Payload{0x55, 0x00, 0xFF, 0x4C};
    const FString Path = FPaths::CreateTempFilename(*FPaths::ProjectSavedDir(), TEXT("u-library-"), TEXT(".bin"));
    const bool bSaved = FFileHelper::SaveArrayToFile(Payload, *Path);
    std::vector<uint8_t> Bytes;
    const UL_Status FileStatus = bSaved
        ? ULibrary::ReadFile(TCHAR_TO_UTF8(*Path), 1, 8, Bytes)
        : UL_IO_ERROR;
    IFileManager::Get().Delete(*Path, false, true);
    if (FileStatus != UL_OK || Bytes != std::vector<uint8_t>{0x00, 0xFF, 0x4C})
    {
        UE_LOG(LogULibrary, Error, TEXT("U-Library file check failed (status %d)."), FileStatus);
        return;
    }

    const uint64 Pid = FPlatformProcess::GetCurrentProcessId();
    const uint64 Address = reinterpret_cast<UPTRINT>(Payload.GetData());
    std::vector<UL_ProcessRegion> Regions;
    const UL_Status MapStatus = ULibrary::GetRegions(Pid, Regions);
    bool bFound = false;
    for (const UL_ProcessRegion& Region : Regions)
        bFound |= Address >= Region.base_address && Address - Region.base_address < Region.size_bytes &&
            (Region.flags & UL_REGION_SAMPLE_READABLE) != 0;
    const UL_Status ReadStatus = ULibrary::ReadProcess(Pid, Address, Payload.Num(), Bytes);
    if (MapStatus != UL_OK || !bFound || ReadStatus != UL_OK ||
        Bytes != std::vector<uint8_t>{0x55, 0x00, 0xFF, 0x4C})
    {
        UE_LOG(LogULibrary, Error, TEXT("U-Library process check failed (map %d, read %d)."), MapStatus, ReadStatus);
        return;
    }

    UE_LOG(LogULibrary, Display, TEXT("U-Library OK. Regions: %llu. Bytes: 55 00 FF 4C"),
        static_cast<unsigned long long>(Regions.size()));
}

static FAutoConsoleCommand ULibrarySmokeCommand(
    TEXT("ULibrary.Smoke"), TEXT("Read a temporary file and sample the current process."),
    FConsoleCommandDelegate::CreateStatic(&RunULibrarySmoke));

IMPLEMENT_MODULE(FULibraryModule, ULibrary)
