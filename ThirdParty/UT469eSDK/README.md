# Unreal Tournament Public Source Distribution

This is OldUnreal's updated version of the original Unreal Tournament Public
Source Distribution for Unreal Tournament v432 (commonly refered to as
PubSrc432).

## What is included in this SDK?

* UnrealScript source code for the following original UPackages: Botpack, Core,
  Engine, Fire, IpDrv, IpServer, MultiMesh, Relics, RelicsBindings, UBrowser,
  UMenu, UTBrowser, UTMenu, UTServerAdmin, UWeb, UWindow, UnrealI,
  UnrealShare. This source code is included merely for convenience as it can
  also be exported directly from the corresponding .u files.

* C++ headers and Windows .lib/resource files for native packages that were
  included in PubSrc432: Core, Engine

* C++ headers and Windows .lib/resource files for native packages that were
  released by Ion Storm: Editor, IpDrv, Fire

* C++ headers and Windows .lib files required to build other packages:
  Render

* Full C++ source code for video renderers that were included in PubSrc432:
  D3DDrv, OpenGLDrv (Daniel Vogel's version)

* Full C++ source code for some of the audio drivers and video renderers
  developed for or contributed to the OldUnreal Community Patches: ALAudio
  (OldUnreal version), D3D9Drv (OldUnreal version), OpenGLDrv (OldUnreal
  version), XOpenGLDrv. Note that OldUnreal's D3D9Drv is based on Chris Dohnal's
  UTGLR Renderer. OldUnreal's OpenGLDrv was originally also based on the UTGLR
  renderer but has been almost fully rewritten since then. XOpenGLDrv was
  written from scratch by members of the OldUnreal developer team.

* Full C++ source code for native packages that were included in PubSrc432:
  Setup, Launch, UCC

* Full C++ source code for native packages that were included in OpenUT: SDLDrv
  (OldUnreal/UTPG version), SDLLaunch (OldUnreal/UTPG version)

* Full C++ source code for packages OldUnreal got permission to release:
  UnrealEd, Window, WinDrv

## What is not included in this SDK?

* C++ source code for core engine packages: Core, Engine, Editor, Fire, Render,
  IpDrv, UWeb

* C++ source code for older audio drivers and renderers: Galaxy, SoftDrv,
  GlideDrv, MetalDrv, ...

* C++ source code for OldUnreal packages that are not ready to be released yet:
  Cluster

## Building C++ packages included in the SDK

Before building, you should copy the contents of this repository into your
Unreal Tournament root folder. To build on Windows, you will need a fairly
recent version of Visual Studio (we recommend version 2022) with the CMake tools
installed. On Linux, you will need clang and CMake.

#### Windows/x86

This SDK works for Windows 7 and up. To build the packages included in the SDK,
you should open a Visual Studio command prompt and set up a CMake build folder
as follows:

```
cd /path/to/UnrealTournament/
mkdir cmake-build
cd cmake-build
cmake -A Win32 -DCMAKE_INSTALL_PREFIX=C:\Path\To\UnrealTournament\System\ -DCMAKE_TOOLCHAIN_FILE=C:\Path\To\UnrealTournament\cmake\MSVC.cmake ..
```

Note: Our CMakeLists.txt automatically looks for 3rd party build dependencies in
the build folder that is included in the SDK. If you want to manually specify the
dependencies path, you should add the following option to your cmake configure
command:
```
-DOLDUNREAL_DEPENDENCIES_PATH=C:\Path\To\UnrealTournament\Dependencies\
```

After configuring the CMake build folder, you can build as follows:
```
cmake --build . --config Release --parallel 8 --target install
```

#### Linux

On Linux, you can set up a CMake build folder as follows:
```
cd /path/to/UnrealTournament/
mkdir cmake-build
cd cmake-build
cmake -DCMAKE_C_COMPILER=clang -DCMAKE_CXX_COMPILER=clang++ -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX=/path/to/UnrealTournament/<System> -DOLDUNREAL_DEPENDENCIES_PATH=/path/to/UnrealTournament/build/Linux/<arch>/Release/ ..
```

In the cmake command above, you should replace <System> with the name of the System folder on your platform. This means:
* System for x86/i386
* System64 for amd64/x86_64
* SystemARM64 for aarch64/arm64

You should also replace <arch> with the CPU architecture of your platform. Valid options are x86, amd64, and arm64.

You can then build as follows:
```
cmake --build . --config Release -j 8 --target install
```

## Building UnrealScript packages included in the SDK

The UnrealScript packages included in this SDK are core packages. They should
not be rebuilt, as doing so **WILL** break network compatibility.

## License/Copyright

* Unless stated otherwise, all files contained here are the property of [Epic
Games, Inc](https://www.epicgames.com). They are provided without warranty, and
under the same terms as the Unreal retail license agreement: You may use them
for your personal, non-profit enjoyment, but you may not sell or otherwise
commercially exploit the source or things you created based on the source.

* The licenses for third-party components included in this SDK can be found in
  LICENSE.md.