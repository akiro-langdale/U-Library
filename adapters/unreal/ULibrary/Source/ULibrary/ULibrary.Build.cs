using System.IO;
using UnrealBuildTool;

public class ULibrary : ModuleRules
{
    public ULibrary(ReadOnlyTargetRules Target) : base(Target)
    {
        PCHUsage = PCHUsageMode.UseExplicitOrSharedPCHs;
        PublicDependencyModuleNames.Add("Core");
        PrivateDependencyModuleNames.Add("Projects");

        string native = Path.Combine(PluginDirectory, "Source", "ThirdParty");
        PublicSystemIncludePaths.Add(Path.Combine(native, "include"));
        PublicAdditionalLibraries.Add(Path.Combine(native, "lib", "Win64", "u_library.dll.lib"));
        PublicDelayLoadDLLs.Add("u_library.dll");
        RuntimeDependencies.Add("$(PluginDir)/Binaries/ThirdParty/Win64/u_library.dll",
            Path.Combine(native, "bin", "Win64", "u_library.dll"), StagedFileType.NonUFS);
    }
}
