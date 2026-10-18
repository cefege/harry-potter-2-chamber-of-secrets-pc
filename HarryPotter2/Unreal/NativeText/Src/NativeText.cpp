/*=============================================================================
	NativeText.cpp: Driver-neutral native Canvas text backend factory. Layout
	creation, measurement, and teardown are delegated to the shared core;
	rasterization and atlas upload belong to the consuming render device.
=============================================================================*/

#include "Engine.h"
#include "NativeText.h"
#include "NativeTextShared.h"

#if !HP2_HAS_NATIVE_TEXT_BACKEND
#error NativeText.cpp must only be compiled when HP2_HAS_NATIVE_TEXT_BACKEND is enabled.
#endif

namespace
{
	class FNativeTextPlatformBackendImpl final : public FNativeTextPlatformBackend
	{
	public:
		UBOOL CreateLayout(const FCanvasTextLayoutRequest& Request, FCanvasTextLayout*& OutLayout) override
		{
			OutLayout = Hp2NativeText::CreateLayout(Request);
			return OutLayout ? 1 : 0;
		}

		void DestroyLayout(FCanvasTextLayout* Layout) override
		{
			Hp2NativeText::DestroyLayout(Layout);
		}

		UBOOL MeasureLayout(FCanvasTextLayout* Layout, INT& OutWidth, INT& OutHeight) override
		{
			return Hp2NativeText::MeasureLayout(Layout, OutWidth, OutHeight) ? 1 : 0;
		}
	};
}

FNativeTextPlatformBackend* CreateNativeTextPlatformBackend(FNativeTextBackendStatus& OutStatus)
{
	const char* ReasonCode = "text.internal_error";
	if (!Hp2NativeText::ProbeAvailability(ReasonCode))
	{
		// Unavailable platform text stack: hand back no backend at all so every
		// per-draw call site keeps its existing Boolean fallback path.
		OutStatus.Available = false;
		OutStatus.ReasonCode = ReasonCode;
		return NULL;
	}

	OutStatus.Available = true;
	OutStatus.ReasonCode = ReasonCode;
	return new FNativeTextPlatformBackendImpl;
}

FNativeTextPlatformBackend* CreateNativeTextPlatformBackend()
{
	FNativeTextBackendStatus Status;
	return CreateNativeTextPlatformBackend(Status);
}
