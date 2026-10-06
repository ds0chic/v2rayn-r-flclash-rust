"""SP-26 helper: scan flutter_windows.dll for renderer switch strings (read-only)."""
import sys

DLL = (sys.argv[1] if len(sys.argv) > 1 else
       r"C:\Users\Colby\toolchains\flutter\bin\cache\artifacts\engine\windows-x64-release\flutter_windows.dll")

data = open(DLL, "rb").read()
print(f"dll_bytes={len(data)}")
for token in [b"--enable-impeller", b"--enable-impeller=true", b"--enable-impeller=false",
              b"enable-software-rendering", b"--impeller-use-sdfs",
              b"--enable-flutter-gpu", b"FlutterDesktopEngineGetGraphicsAdapter",
              b"LowPowerPreference", b"HighPerformancePreference"]:
    print(f"{token.decode()} count={data.count(token)}")
i = data.find(b"enable-software-rendering")
print("software_rendering_help=" + repr(data[i - 60:i + 160]))
