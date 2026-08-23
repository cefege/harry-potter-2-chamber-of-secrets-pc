set(HP2_UNREAL_ROOT "${PROJECT_SOURCE_DIR}/HarryPotter2/Unreal" CACHE PATH "Engine data root")
set(HP2_THIRD_PARTY_ROOT "${PROJECT_SOURCE_DIR}/ThirdParty")

# Core.dsp runtime units plus UnCoreNative.cpp for the static native lookup
# table. UnPSX2.cpp, UnVcWin32.cpp, legacy UnGUID.cpp, and the unused
# AudioMutex.cpp unit are intentionally excluded from this arm64 graph.
set(HP2_CORE_SOURCES
    "${HP2_UNREAL_ROOT}/Core/Src/Core.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UExporter.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UFactory.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnAnsi.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnBits.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnCache.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnClass.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnCoreNative.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnCoreNet.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnCorSc.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnLinker.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnMath.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnMem.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnMisc.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnName.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnObj.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnProp.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnUnix.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/UnFileStream.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/FEAXABlockDecoder.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/HP2CrashReporter.cpp"
)

# Engine.dsp runtime units, excluding the Windows-only winstuff.cpp unit.
set(HP2_ENGINE_SOURCES
    "${HP2_UNREAL_ROOT}/Engine/Src/AStatLog.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/Engine.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/palette.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/ULodMesh.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnActCol.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnActor.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnAudio.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnCamera.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnCamMgr.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnCanvas.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnCon.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnDynBsp.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnEngine.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnEngineNative.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnFont.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnFPoly.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnGame.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnGesture.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnIn.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnLevAct.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnLevel.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnLevTic.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnMesh.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnModel.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnMover.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnParams.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnParticle.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnParticleFX.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnParticleList.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnPath.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnPawn.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnPhysic.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnPlayer.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnPrim.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnReach.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnRenderIterator.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnReplay.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnRoute.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnScript.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnScrTex.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnSkeletalMesh.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnTex.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnTrace.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnURL.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnWind.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnBunch.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnChan.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnConn.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnDemoPenLev.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnDemoRec.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnDownload.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnNetDrv.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/UnPenLev.cpp"
    "${HP2_UNREAL_ROOT}/Engine/MPeg/scompmpg.cpp"
    "${HP2_UNREAL_ROOT}/Engine/MPeg/TbFile.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/S3tcCompat.cpp"
)

set(HP2_RENDER_SOURCES
    "${HP2_UNREAL_ROOT}/Render/Src/Render.cpp"
    "${HP2_UNREAL_ROOT}/Render/Src/UnLight.cpp"
    "${HP2_UNREAL_ROOT}/Render/Src/UnMeshRn.cpp"
    "${HP2_UNREAL_ROOT}/Render/Src/UnParticleRn.cpp"
    "${HP2_UNREAL_ROOT}/Render/Src/UnRandom.cpp"
    "${HP2_UNREAL_ROOT}/Render/Src/UnRender.cpp"
    "${HP2_UNREAL_ROOT}/Render/Src/UnSoftLn.cpp"
    "${HP2_UNREAL_ROOT}/Render/Src/UnSpan.cpp"
    "${HP2_UNREAL_ROOT}/Render/Src/UnSprite.cpp"
    "${HP2_UNREAL_ROOT}/Render/Src/UnTest.cpp"
)

# Fire's x86 assembly units are not part of the arm64 graph.
set(HP2_FIRE_SOURCES
    "${HP2_UNREAL_ROOT}/Fire/Src/UnFractal.cpp"
)

# Stock game packages import Editor.Transactor and Editor.TransBuffer. Keep the
# native runtime subset limited to the real transaction implementation plus
# package/global storage; no editor UI or tooling units belong in this graph.
set(HP2_EDITOR_RUNTIME_SOURCES
    "${HP2_UNREAL_ROOT}/Editor/Src/UnEdTran.cpp"
    "${HP2_UNREAL_ROOT}/Launch/Src/HP2EditorRuntime.cpp"
)

set(HP2_ALAUDIO_SOURCES
    "${HP2_UNREAL_ROOT}/ALAudio/Src/ALAudio.cpp"
    "${HP2_UNREAL_ROOT}/ALAudio/Src/ALAudioSubsystem.cpp"
)

set(HP2_SDLDRV_SOURCES
    "${HP2_THIRD_PARTY_ROOT}/UT469eSDK/SDLDrv/Src/SDLClient.cpp"
    "${HP2_THIRD_PARTY_ROOT}/UT469eSDK/SDLDrv/Src/SDLDrv.cpp"
    "${HP2_THIRD_PARTY_ROOT}/UT469eSDK/SDLDrv/Src/SDLViewport.cpp"
)

# This allowlist is the complete pinned XOpenGL runtime subset. The vendored
# GLM implementation translation unit and unrelated donor trees are excluded.
set(HP2_XOPENGLDRV_SOURCES
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/XOpenGLDrv.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/XOpenGL.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/glad.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/CheckExtensions.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DebugLog.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DrawComplex.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DrawComplex_GLSL.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DrawGouraud.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DrawGouraud_GLSL.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DrawPostProcess.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DrawSimple.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DrawSimple_GLSL.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DrawTile.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/DrawTile_GLSL.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/EditorHit.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/SetTexture.cpp"
    "${HP2_THIRD_PARTY_ROOT}/XOpenGLDrv/Src/ShaderProgram.cpp"
)

set(HP2_LAUNCHER_CORE_SOURCES
    "${HP2_UNREAL_ROOT}/SDLLaunch/Src/HP2LaunchPolicy.cpp"
    "${HP2_UNREAL_ROOT}/SDLLaunch/Src/HP2LauncherStore.cpp"
)
set(HP2_NATIVE_LAUNCHER_SOURCE
    "${HP2_UNREAL_ROOT}/SDLLaunch/Src/HP2MacLauncher.mm"
)
set(HP2_SDL_LAUNCH_SOURCES
    "${HP2_UNREAL_ROOT}/SDLLaunch/Src/SDLLaunch.cpp"
    ${HP2_LAUNCHER_CORE_SOURCES}
    "${HP2_NATIVE_LAUNCHER_SOURCE}"
)
set(HP2_UCC_SOURCES
    "${HP2_UNREAL_ROOT}/UCC/Src/UCC.cpp"
)

set(HP2_STATIC_PACKAGE_SOURCE
    "${HP2_UNREAL_ROOT}/Launch/Src/HP2StaticPackages.cpp"
)
set(HP2_PATH_SOURCE
    "${HP2_UNREAL_ROOT}/Launch/Src/HP2Paths.cpp"
)
set(HP2_PACKAGE_AUDIT_SOURCE "${PROJECT_SOURCE_DIR}/Tests/PackageAudit.cpp")
set(HP2_ABI_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/AbiTests.cpp")
set(HP2_SPELL_RUNTIME_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/SpellRuntimeTests.cpp")
set(HP2_EAXA_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/EaxaTests.cpp")
set(HP2_AUDIO_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/AudioTests.cpp")
set(HP2_DXT1_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/Dxt1Tests.cpp")
set(HP2_LAUNCHER_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/LauncherTests.cpp")
set(HP2_NATIVE_TYPOGRAPHY_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/NativeTypographyTests.cpp")
set(HP2_CANVAS_COMPATIBILITY_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/CanvasCompatibilityTests.cpp")
set(HP2_CONFIG_INI_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/ConfigIniTests.cpp")
set(HP2_REPLAY_ROUNDTRIP_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/ReplayRoundTripTests.cpp")
set(HP2_INPUT_CONTRACT_TEST_SOURCE "${PROJECT_SOURCE_DIR}/Tests/InputContractTests.cpp")

# Every source below is required by the completed native test graph. Keep the
# list explicit so HP2_REQUIRE_IMPLEMENTED_SOURCES reports a focused error at
# configure time rather than treating missing implementation as generated.
set(HP2_PLANNED_IMPLEMENTATION_SOURCES
    "${HP2_UNREAL_ROOT}/Core/Src/UnFileStream.cpp"
    "${HP2_UNREAL_ROOT}/Core/Src/FEAXABlockDecoder.cpp"
    "${HP2_UNREAL_ROOT}/Engine/Src/S3tcCompat.cpp"
    ${HP2_EDITOR_RUNTIME_SOURCES}
    "${HP2_STATIC_PACKAGE_SOURCE}"
    "${HP2_PATH_SOURCE}"
    "${HP2_PACKAGE_AUDIT_SOURCE}"
    "${HP2_ABI_TEST_SOURCE}"
    "${HP2_SPELL_RUNTIME_TEST_SOURCE}"
    "${HP2_EAXA_TEST_SOURCE}"
    "${HP2_AUDIO_TEST_SOURCE}"
    "${HP2_DXT1_TEST_SOURCE}"
    ${HP2_LAUNCHER_CORE_SOURCES}
    "${HP2_NATIVE_LAUNCHER_SOURCE}"
    "${HP2_LAUNCHER_TEST_SOURCE}"
    "${HP2_NATIVE_TYPOGRAPHY_TEST_SOURCE}"
)

set(_hp2_missing_implementation_sources)
foreach(_hp2_source IN LISTS HP2_PLANNED_IMPLEMENTATION_SOURCES)
    if(NOT EXISTS "${_hp2_source}")
        list(APPEND _hp2_missing_implementation_sources "${_hp2_source}")
    endif()
endforeach()

if(HP2_REQUIRE_IMPLEMENTED_SOURCES AND _hp2_missing_implementation_sources)
    list(JOIN _hp2_missing_implementation_sources "\n  " _hp2_missing_text)
    message(FATAL_ERROR
        "Required HP2 implementation sources are missing:\n  ${_hp2_missing_text}")
endif()

unset(_hp2_source)
unset(_hp2_missing_text)
unset(_hp2_missing_implementation_sources)
