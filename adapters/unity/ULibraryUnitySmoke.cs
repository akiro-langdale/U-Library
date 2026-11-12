using UnityEngine;

public sealed class ULibraryUnitySmoke : MonoBehaviour
{
    private void Start()
    {
        Debug.Log(ULibrary.Smoke.Run(Application.temporaryCachePath));
    }
}
