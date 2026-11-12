using Godot;

public partial class ULibraryGodotSmoke : Node
{
    public override void _Ready()
    {
        GD.Print(ULibrary.Smoke.Run(ProjectSettings.GlobalizePath("user://")));
    }
}
