/*
 * HP2 end-of-frame capture for the vendored Vulkan driver.
 *
 * HP2-owned file; mirrors the XOpenGLDrv end-of-present capture block
 * (ThirdParty/XOpenGLDrv/Src/XOpenGL.cpp) so the renderer matrix compares the
 * same frame_meta.json schema from both drivers. Vendored-mod against
 * upstream pin a29e9ac0df1c60ad302d91bc3a51ab026c1a307c.
 */

#include "Precomp.h"
#include "UVulkanRenderDevice.h"
#include "Hp2FrameCapture.h"

#ifdef WIN32

void Hp2CaptureFrame(UVulkanRenderDevice* /*Renderer*/)
{
	// HP2 capture hook is implemented for the mac port only, mirroring the
	// XOpenGLDrv hook's non-Windows guard.
}

#else

#include <sys/stat.h>
#include <cstdlib>
#include <cstdio>
#include <string>
#include <vector>

#ifndef WIN32
#include <SDL2/SDL_vulkan.h>
#endif

// HP2: PNG encoding via the vendored single-header stb_image_write shared
// with XOpenGLDrv. The implementation TU is XOpenGL.cpp (linked into the same
// hp2_game image), so this TU includes the header in declarations-only mode
// to keep exactly one copy of the encoders per binary. The vendored stb
// header zero-initializes its contexts field-wise, tripping
// -Wmissing-field-initializers under the repo warning policy.
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wmissing-field-initializers"
#include "../../../XOpenGLDrv/ThirdParty/stb/stb_image_write.h"
#pragma clang diagnostic pop

/*-----------------------------------------------------------------------------
	HP2 end-of-frame capture. When HP2_CAPTURE_FRAMES names a directory, every
	presented frame is read back through UVulkanRenderDevice::ReadPixels and
	written as frame_<6d>.png plus a frame_meta.json sidecar (width/height,
	HiDPI backing scale factor, renderer, requested ticks, map). The harness
	may refine the metadata through HP2_CAPTURE_MAP and HP2_CAPTURE_TICKS.
	With HP2_CAPTURE_FRAMES unset the hook costs a single getenv for the whole
	process lifetime.
-----------------------------------------------------------------------------*/
namespace
{
	struct FFrameCaptureState
	{
		bool Initialized = false;
		bool Enabled = false;
		INT FrameIndex = 0;
		INT RequestedTicks = -1;
		std::string Directory;
		std::string MapName;
		std::vector<std::string> MetaEntries;
	};

	FFrameCaptureState GFrameCapture;

	const char* HPCaptureEnv(const char* Name)
	{
		const char* Value = getenv(Name);
		return (Value && Value[0]) ? Value : NULL;
	}

	void HPAppendJsonString(std::string& Out, const char* Text)
	{
		Out.push_back('"');
		for (const char* Cursor = Text ? Text : ""; *Cursor; ++Cursor)
		{
			const unsigned char Character = (unsigned char)*Cursor;
			if (Character == '"' || Character == '\\')
			{
				Out.push_back('\\');
				Out.push_back((char)Character);
			}
			else if (Character < 0x20)
			{
				char Escape[8];
				snprintf(Escape, sizeof(Escape), "\\u%04x", (unsigned)Character);
				Out += Escape;
			}
			else
			{
				Out.push_back((char)Character);
			}
		}
		Out.push_back('"');
	}

	void HPRewriteFrameMeta()
	{
		std::string Path = GFrameCapture.Directory + "/frame_meta.json";
		std::string Json = "{\"captures\":[";
		for (size_t Index = 0; Index < GFrameCapture.MetaEntries.size(); ++Index)
		{
			if (Index)
				Json += ",";
			Json += GFrameCapture.MetaEntries[Index];
		}
		Json += "]}\n";
		if (FILE* Stream = fopen(Path.c_str(), "wb"))
		{
			fwrite(Json.data(), 1, Json.size(), Stream);
			fclose(Stream);
		}
	}

	void HPInitFrameCapture()
	{
		GFrameCapture.Initialized = true;
		const char* Directory = HPCaptureEnv("HP2_CAPTURE_FRAMES");
		if (!Directory)
			return;
		GFrameCapture.Directory = Directory;
		if (const char* MapName = HPCaptureEnv("HP2_CAPTURE_MAP"))
			GFrameCapture.MapName = MapName;
		if (const char* Ticks = HPCaptureEnv("HP2_CAPTURE_TICKS"))
			GFrameCapture.RequestedTicks = atoi(Ticks);
		// Best effort; the harness normally pre-creates the directory.
		mkdir(GFrameCapture.Directory.c_str(), 0775);
		GFrameCapture.Enabled = true;
		debugf(NAME_Init, TEXT("VulkanDrv: frame capture enabled in %ls"), appFromAnsi(GFrameCapture.Directory.c_str()));
	}
}

void Hp2CaptureFrame(UVulkanRenderDevice* Renderer)
{
	if (!GFrameCapture.Initialized)
		HPInitFrameCapture();
	if (!GFrameCapture.Enabled || !Renderer || !Renderer->Viewport)
		return;

	const INT Width = Renderer->Viewport->SizeX;
	const INT Height = Renderer->Viewport->SizeY;
	if (Width <= 0 || Height <= 0)
		return;

	// HP2: capture the GRADED present output. ReadPixels copies the raw scene
	// PPImage when GammaCorrectScreenshots is off, which diverges from the
	// graded present-shader output the window actually shows (observed as a
	// cool-vs-warm mismatch against the GL leg and the live window). Force the
	// ScreenshotPipeline path so the capture matches the screen. Vendored-mod
	// context: upstream pin a29e9ac0df1c60ad302d91bc3a51ab026c1a307c.
	const UBOOL PrevGammaCorrect = Renderer->GammaCorrectScreenshots;
	Renderer->GammaCorrectScreenshots = 1;
	std::vector<FColor> Pixels((SIZE_T)Width * (SIZE_T)Height);
	Renderer->ReadPixels(Pixels.data());
	Renderer->GammaCorrectScreenshots = PrevGammaCorrect;

	const SIZE_T Stride = (SIZE_T)Width * 3;
	std::vector<unsigned char> TopDown(Stride * (SIZE_T)Height);
	const unsigned char* Source = reinterpret_cast<const unsigned char*>(Pixels.data());
	for (INT Row = 0; Row < Height; ++Row)
	{
		unsigned char* Dest = &TopDown[Stride * (SIZE_T)Row];
		const unsigned char* Line = Source + (SIZE_T)Width * 4 * (SIZE_T)Row;
		for (INT Column = 0; Column < Width; ++Column)
		{
			Dest[Column * 3 + 0] = Line[Column * 4 + 2];
			Dest[Column * 3 + 1] = Line[Column * 4 + 1];
			Dest[Column * 3 + 2] = Line[Column * 4 + 0];
		}
	}

	// HiDPI backing scale: drawable pixels per logical window pixel.
	int LogicalWidth = 0, LogicalHeight = 0, DrawableWidth = 0, DrawableHeight = 0;
	SDL_Window* Window = (SDL_Window*)Renderer->Viewport->GetWindow();
	double BackingScale = 1.0;
	if (Window)
	{
		SDL_GetWindowSize(Window, &LogicalWidth, &LogicalHeight);
		SDL_GL_GetDrawableSize(Window, &DrawableWidth, &DrawableHeight);
		BackingScale = LogicalHeight > 0 ? (double)DrawableHeight / (double)LogicalHeight : 1.0;
	}

	char Path[1024];
	snprintf(Path, sizeof(Path), "%s/frame_%06d.png", GFrameCapture.Directory.c_str(), GFrameCapture.FrameIndex);
	if (!stbi_write_png(Path, Width, Height, 3, TopDown.data(), (int)Stride))
	{
		debugf(NAME_Warning, TEXT("VulkanDrv: frame capture write failed for %ls; disabling capture"), appFromAnsi(Path));
		GFrameCapture.Enabled = false;
		return;
	}

	char EntryHead[256];
	snprintf(EntryHead, sizeof(EntryHead),
		"{\"width_px\":%i,\"height_px\":%i,\"backing_scale_factor\":%.6f,\"renderer\":",
		Width, Height, BackingScale);
	std::string Entry(EntryHead);
	HPAppendJsonString(Entry, Renderer->Device->PhysicalDevice.Properties.Properties.deviceName);
	Entry += ",\"ticks\":";
	if (GFrameCapture.RequestedTicks >= 0)
	{
		char TickText[32];
		snprintf(TickText, sizeof(TickText), "%d", GFrameCapture.RequestedTicks);
		Entry += TickText;
	}
	else
	{
		Entry += "null";
	}
	Entry += ",\"map\":";
	HPAppendJsonString(Entry, GFrameCapture.MapName.c_str());
	Entry += "}";
	GFrameCapture.MetaEntries.push_back(Entry);
	HPRewriteFrameMeta();

	debugf(NAME_Log, TEXT("VulkanDrv: captured frame_%06i.png %ix%i backing_scale=%.3f"), GFrameCapture.FrameIndex, Width, Height, BackingScale);
	++GFrameCapture.FrameIndex;
}

#endif
