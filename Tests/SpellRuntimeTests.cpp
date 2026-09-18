/*=============================================================================
	SpellRuntimeTests.cpp: stock Core/Engine/HGame package-load boundary.
=============================================================================*/

#include <stdlib.h>

#include "Engine.h"
#include "HP2Paths.h"
#include "HP2StaticPackages.h"
#include "FMallocAnsi.h"
#include "FFileManagerUnix.h"
#include "FFeedbackContextAnsi.h"
#include "FConfigCacheIni.h"

#include <cstdarg>
#include <cstdio>
#include <cstring>

extern "C" { TCHAR GPackage[64] = TEXT("SpellRuntimeTests"); }
INT GFilesOpen = 0;
INT GFilesOpened = 0;

namespace
{
	class FSilentLog : public FOutputDevice
	{
	public:
		void Serialize(const TCHAR*, EName) override {}
	};

	class FCapturingError : public FOutputDeviceError
	{
	public:
		FCapturingError() { Message[0] = 0; }
		void Serialize(const TCHAR* Text, EName) override
		{
			if (!Message[0])
				appStrncpy(Message, Text, ARRAY_COUNT(Message));
			throw 1;
		}
		void HandleError() override {}
		TCHAR Message[1024];
	};

	FSilentLog RuntimeLog;
	FCapturingError RuntimeError;
	FFeedbackContextAnsi RuntimeWarn;
	FFileManagerUnix RuntimeFileManager;
	FMallocAnsi RuntimeMalloc;
	const char* TestStage = "bootstrap";

	int Fail(const char* Format, ...)
	{
		std::fprintf(stderr, "spell_runtime[%s]: ", TestStage);
		va_list Args;
		va_start(Args, Format);
		std::vfprintf(stderr, Format, Args);
		va_end(Args);
		std::fputc('\n', stderr);
		return 1;
	}

	int Run(int ArgC, char** ArgV)
	{
		if (!PrepareHP2Paths(ArgC, ArgV))
			return Fail("path bootstrap failed");
#if !_MSC_VER
		__Context::StaticInit();
		std::strncpy(GModule, ArgV[0], sizeof(GModule) - 1);
		GModule[sizeof(GModule) - 1] = 0;
#endif
		TCHAR CmdLine[2048];
		CmdLine[0] = 0;
		for (INT Index = 1; Index < ArgC; ++Index)
		{
			const TCHAR* Argument = ANSI_TO_TCHAR(ArgV[Index]);
			if (appStrlen(CmdLine) + appStrlen(Argument) + 2 >= ARRAY_COUNT(CmdLine))
				return Fail("runtime command line is too long");
			if (CmdLine[0]) appStrcat(CmdLine, TEXT(" "));
			appStrcat(CmdLine, Argument);
		}

		InstallHP2NativeLookups();
		GIsStarted = 1;
		GIsGuarded = 1;
		appInit(TEXT("SpellRuntimeTests"), CmdLine, &RuntimeMalloc, &RuntimeLog, &RuntimeError, &RuntimeWarn,
			&RuntimeFileManager, FConfigCacheIni::Factory, 1);
		RegisterHP2RuntimeClasses();
		TestStage = "Engine.u-load";
		UObject* EnginePackage = UObject::LoadPackage(NULL, TEXT("Engine.u"), LOAD_NoFail);
		if (!Cast<UPackage>(EnginePackage))
			return Fail("Engine package load failed");
		TestStage = "HGame.u-load";
		UObject* HGamePackage = UObject::LoadPackage(NULL, TEXT("HGame.u"), LOAD_NoFail);
		if (!Cast<UPackage>(HGamePackage))
			return Fail("HGame package load failed");
		if (!UObject::StaticFindObject(UClass::StaticClass(), ANY_PACKAGE, TEXT("Engine.Mover"), 1))
			return Fail("Engine.Mover class is unavailable after package load");
		return 0;
	}
}

int main(int ArgC, char** ArgV)
{
	int Result = 1;
	try
	{
		Result = Run(ArgC, ArgV);
	}
	catch (...)
	{
		Result = RuntimeError.Message[0]
			? Fail("runtime failure: %ls", RuntimeError.Message)
			: Fail("runtime threw without an engine diagnostic");
	}
	GIsScriptable = 0;
	GIsGuarded = 0;
	if (GIsStarted)
		appExit();
	GIsStarted = 0;
	if (!Result)
		std::puts("spell runtime contracts passed");
	return Result;
}
