#pragma once

#include "Modules/ModuleInterface.h"

class ULIBRARY_API FULibraryModule : public IModuleInterface
{
public:
    void StartupModule() override;
    bool IsReady() const { return NativeLibrary != nullptr; }
    bool SupportsDynamicReloading() override { return false; }

private:
    // Delay-loaded imports remain bound for the life of the host process.
    void* NativeLibrary = nullptr;
};
