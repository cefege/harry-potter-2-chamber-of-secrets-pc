/*=============================================================================
	HP2Launcher.cpp: SDL2 + Dear ImGui launcher shared by desktop and web.

	The flow is built around getting a player into the game: with no game
	files the only thing on screen is the import call to action; once the
	files are installed the screen leads with one big Continue / Start
	playing button. Settings and data-source details sit one click away.
=============================================================================*/
#include "HP2Launcher.h"

#include <ctime>
#include "HP2DataImport.h"
#include "HP2LaunchPolicy.h"
#include "HP2LauncherPlatform.h"
#include "HP2LauncherStore.h"
#include "HP2Paths.h"

#include <SDL.h>
#include <imgui.h>
#include <backends/imgui_impl_sdl2.h>
#include <backends/imgui_impl_sdlrenderer2.h>

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <memory>
#include <mutex>
#include <thread>
#include <unordered_map>
#include <utility>

#if defined(__EMSCRIPTEN__)
#include <emscripten.h>
#include <emscripten/html5.h>
#endif

namespace HP2Launcher
{
namespace
{
constexpr float ColumnWidth = 640.0f;
constexpr float PrimaryHeight = 72.0f;
constexpr float SecondaryHeight = 48.0f;

const ImVec4 ColorBackground(0.07f, 0.08f, 0.11f, 1.0f);
const ImVec4 ColorText(0.94f, 0.92f, 0.87f, 1.0f);
const ImVec4 ColorMuted(0.63f, 0.65f, 0.73f, 1.0f);
const ImVec4 ColorAccent(0.84f, 0.66f, 0.23f, 1.0f);
const ImVec4 ColorAccentHover(0.92f, 0.74f, 0.30f, 1.0f);
const ImVec4 ColorAccentActive(0.74f, 0.57f, 0.17f, 1.0f);
const ImVec4 ColorOnAccent(0.10f, 0.08f, 0.04f, 1.0f);
const ImVec4 ColorSurface(0.16f, 0.18f, 0.24f, 1.0f);
const ImVec4 ColorSurfaceHover(0.22f, 0.24f, 0.32f, 1.0f);
const ImVec4 ColorSurfaceActive(0.27f, 0.30f, 0.40f, 1.0f);
const ImVec4 ColorSuccess(0.47f, 0.82f, 0.52f, 1.0f);
const ImVec4 ColorWarning(0.96f, 0.58f, 0.42f, 1.0f);

std::string FormatDate(std::int64_t seconds)
{
	if (seconds <= 0) return "Date unavailable";
	const std::time_t value = static_cast<std::time_t>(seconds);
	std::tm tmValue{};
#if defined(_WIN32)
	localtime_s(&tmValue, &value);
#else
	localtime_r(&value, &tmValue);
#endif
	char text[64]{};
	if (std::strftime(text, sizeof(text), "%x %H:%M", &tmValue) == 0) return "Date unavailable";
	return text;
}

const DataSourceOption* FindOption(const std::vector<DataSourceOption>& options, DataSource source)
{
	for (const DataSourceOption& option : options) if (option.source == source) return &option;
	return nullptr;
}

std::string ResolutionText(const DisplayResolution& resolution)
{
	return resolution.label.empty()
		? std::to_string(resolution.width) + " x " + std::to_string(resolution.height)
		: resolution.label;
}

std::string LocateFonts()
{
	char* base = SDL_GetBasePath();
	std::filesystem::path executable = base ? std::filesystem::u8path(base) : std::filesystem::current_path();
	if (base) SDL_free(base);
#if defined(__EMSCRIPTEN__)
	const std::filesystem::path browserFonts("/hp2fonts");
	if (std::filesystem::exists(browserFonts / "LiberationSans-Regular.ttf"))
		return browserFonts.u8string();
#endif
	const std::filesystem::path candidates[] = {
		executable / "fonts",
		executable / ".." / "Resources" / "fonts",
		executable / "../share/harry-potter-2/fonts"
	};
	for (const auto& path : candidates)
		if (std::filesystem::exists(path / "LiberationSans-Regular.ttf")) return path.u8string();
	return candidates[0].u8string();
}

// The big call-to-action button: gold, full column width.
bool PrimaryButton(const char* label, bool enabled = true)
{
	ImGui::PushStyleColor(ImGuiCol_Button, ColorAccent);
	ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ColorAccentHover);
	ImGui::PushStyleColor(ImGuiCol_ButtonActive, ColorAccentActive);
	ImGui::PushStyleColor(ImGuiCol_Text, ColorOnAccent);
	if (!enabled) ImGui::BeginDisabled();
	const bool pressed = ImGui::Button(label, ImVec2(-1.0f, PrimaryHeight));
	if (!enabled) ImGui::EndDisabled();
	ImGui::PopStyleColor(4);
	return pressed && enabled;
}

bool SecondaryButton(const char* label, float width = -1.0f, bool enabled = true)
{
	if (!enabled) ImGui::BeginDisabled();
	const bool pressed = ImGui::Button(label, ImVec2(width, SecondaryHeight));
	if (!enabled) ImGui::EndDisabled();
	return pressed && enabled;
}

void ColoredWrapped(const ImVec4& color, const std::string& text)
{
	ImGui::PushStyleColor(ImGuiCol_Text, color);
	ImGui::TextWrapped("%s", text.c_str());
	ImGui::PopStyleColor();
}

void Gap(float height = 12.0f) { ImGui::Dummy(ImVec2(0.0f, height)); }

template <typename T, std::size_t N>
bool EnumCombo(const char* label, T& value, const char* const (&labels)[N])
{
	int selected = static_cast<int>(value);
	ImGui::SetNextItemWidth(-220.0f);
	if (!ImGui::Combo(label, &selected, labels, static_cast<int>(N))) return false;
	value = static_cast<T>(selected);
	return true;
}

void ApplyStyle()
{
	ImGui::StyleColorsDark();
	ImGuiStyle& style = ImGui::GetStyle();
	style.WindowPadding = ImVec2(24.0f, 24.0f);
	style.FramePadding = ImVec2(16.0f, 12.0f);
	style.ItemSpacing = ImVec2(10.0f, 12.0f);
	style.TouchExtraPadding = ImVec2(4.0f, 4.0f);
	style.FrameRounding = 10.0f;
	style.GrabRounding = 10.0f;
	style.ChildRounding = 12.0f;
	style.PopupRounding = 10.0f;
	style.ScrollbarRounding = 10.0f;
	style.WindowBorderSize = 0.0f;
	style.ChildBorderSize = 0.0f;
	style.FrameBorderSize = 0.0f;
	ImVec4* colors = style.Colors;
	colors[ImGuiCol_WindowBg] = ColorBackground;
	colors[ImGuiCol_ChildBg] = ImVec4(0, 0, 0, 0);
	colors[ImGuiCol_PopupBg] = ImVec4(0.11f, 0.12f, 0.17f, 1.0f);
	colors[ImGuiCol_Text] = ColorText;
	colors[ImGuiCol_TextDisabled] = ColorMuted;
	colors[ImGuiCol_Button] = ColorSurface;
	colors[ImGuiCol_ButtonHovered] = ColorSurfaceHover;
	colors[ImGuiCol_ButtonActive] = ColorSurfaceActive;
	colors[ImGuiCol_FrameBg] = ColorSurface;
	colors[ImGuiCol_FrameBgHovered] = ColorSurfaceHover;
	colors[ImGuiCol_FrameBgActive] = ColorSurfaceActive;
	colors[ImGuiCol_Header] = ColorSurface;
	colors[ImGuiCol_HeaderHovered] = ColorSurfaceHover;
	colors[ImGuiCol_HeaderActive] = ColorSurfaceActive;
	colors[ImGuiCol_CheckMark] = ColorAccent;
	colors[ImGuiCol_SliderGrab] = ColorAccent;
	colors[ImGuiCol_SliderGrabActive] = ColorAccentHover;
	colors[ImGuiCol_PlotHistogram] = ColorAccent;
	colors[ImGuiCol_Separator] = ColorSurface;
}
}

#if defined(__EMSCRIPTEN__)
namespace
{
std::mutex WebMutex;
std::unordered_map<int, std::pair<bool, std::string>> WebResults;
int NextWebRequest = 1;
}
extern "C" EMSCRIPTEN_KEEPALIVE void hp2_web_bridge_done(int requestId, int ok, const char* payload)
{
	std::lock_guard<std::mutex> lock(WebMutex);
	WebResults[requestId] = {ok != 0, payload ? payload : ""};
}
EM_JS(void, HP2WebCall, (int requestId, int operation, const char* argument), {
	const api = Module.hp2Web;
	if (!api) return;
	// MEMORY64: the pointer arrives as a BigInt; UTF8ToString needs a Number.
	const arg = UTF8ToString(Number(argument));
	switch (operation) {
	case 0: api.dataSourceStatus(requestId); break;
	case 1: api.pickFolder(requestId, arg); break;
	case 2: api.persistImport(requestId, '/hp2import/out', 'retail'); break;
	case 3: api.stageDataSource(requestId, arg); break;
	case 4: api.syncUser(requestId); break;
	}
});
#endif

enum class Screen
{
	Home,
	Settings,
	GameFiles
};

struct FLauncherSession::Impl
{
	LauncherRequest request;
	LauncherCapabilities caps;
	LauncherResult result;
	SDL_Window* window = nullptr;
	SDL_Renderer* renderer = nullptr;
	bool imguiContext = false;
	bool sdlBackend = false;
	bool rendererBackend = false;
	bool ownsVideo = false;
	bool opened = false;
	bool finished = false;
	Screen screen = Screen::Home;
	int selectedSave = -1;
	std::string validationError;
	// Problems with the game files (failed import, cancelled picker).
	std::string dataMessage;
	// Shown once after a successful import.
	std::string installedNotice;
	std::vector<DataSourceOption> options;
	SDL_Texture* thumbnail = nullptr;
	std::string thumbnailPath;
	std::thread importer;
	HP2Import::Progress importProgress;
	std::atomic<bool> importDone{false};
	bool importSucceeded = false;
	std::string importOutput;
	std::string importError;
	bool importing = false;
	int smokeFrames = -1;
	int frameCount = 0;
	std::string fontsDirectory;
	ImFont* titleFont = nullptr;
	ImFont* smallFont = nullptr;
#if defined(__EMSCRIPTEN__)
	int webStatusRequest = 0;
	int webPickRequest = 0;
	int webStageRequest = 0;
	int webPersistRequest = 0;
	std::string webPendingKind;
	std::string webStagedKind;
	bool webRefreshAfterStage = false;
	// The browser answers the data-source status asynchronously; until then
	// the launcher cannot tell a first visit from a returning player.
	bool webStatusKnown = false;
	// A first visit opens the folder prompt once, without a click.
	bool webPromptedImport = false;
	LaunchAction pendingLaunchAction = LaunchAction::Error;
#endif

	Impl(const LauncherRequest& source, const LauncherCapabilities& capabilities)
		: request(source), caps(capabilities), options(source.dataSourceOptions)
	{
		result.settings = source.settings;
		result.dataSources = source.dataSources;
		result.selection.action = LaunchAction::Quit;
		for (std::size_t i = 0; i < request.saves.size(); ++i)
			if (request.saves[i].saveIndex >= 0) { selectedSave = static_cast<int>(i); break; }
		if (const char* smoke = std::getenv("HP2_LAUNCHER_SMOKE_FRAMES"))
		{
			char* end = nullptr;
			long parsed = std::strtol(smoke, &end, 10);
			if (end != smoke && *end == '\0' && parsed > 0 && parsed < 100000) smokeFrames = static_cast<int>(parsed);
		}
	}

	~Impl()
	{
		if (importing) importProgress.cancelRequested.store(true, std::memory_order_relaxed);
		if (importer.joinable()) importer.join();
		if (thumbnail) SDL_DestroyTexture(thumbnail);
		if (rendererBackend) ImGui_ImplSDLRenderer2_Shutdown();
		if (sdlBackend) ImGui_ImplSDL2_Shutdown();
		if (imguiContext) ImGui::DestroyContext();
#if defined(__EMSCRIPTEN__)
		// The browser has one WebGL context, shared with the game that starts
		// next. ImGui leaves the scissor test on with its last clip rectangle
		// (the launcher's column), and the game's renderer never sets a
		// scissor, so it would only ever draw inside that column. A clear with
		// no clip rectangle makes SDL switch the scissor test off.
		if (renderer)
		{
			SDL_RenderSetClipRect(renderer, nullptr);
			SDL_RenderClear(renderer);
			SDL_RenderFlush(renderer);
		}
#endif
		if (renderer) SDL_DestroyRenderer(renderer);
		if (window) SDL_DestroyWindow(window);
		if (ownsVideo) SDL_QuitSubSystem(SDL_INIT_VIDEO);
	}

	void RefreshThumbnail()
	{
		const std::string path = selectedSave >= 0 && selectedSave < static_cast<int>(request.saves.size())
			? request.saves[static_cast<std::size_t>(selectedSave)].thumbnailPath : std::string();
		if (path == thumbnailPath) return;
		if (thumbnail) { SDL_DestroyTexture(thumbnail); thumbnail = nullptr; }
		thumbnailPath = path;
		if (!path.empty())
		{
			SDL_Surface* surface = SDL_LoadBMP(path.c_str());
			if (surface)
			{
				thumbnail = SDL_CreateTextureFromSurface(renderer, surface);
				SDL_FreeSurface(surface);
			}
		}
	}

	bool HasSave() const { return selectedSave >= 0 && selectedSave < static_cast<int>(request.saves.size()); }

	bool DataReady() const
	{
		const DataSourceOption* option = FindOption(options, request.dataSources.selected);
		return option && option->available;
	}

	bool Installing() const
	{
		if (importing) return true;
#if defined(__EMSCRIPTEN__)
		if (webPersistRequest || webRefreshAfterStage) return true;
#endif
		return false;
	}

	bool Launching() const
	{
#if defined(__EMSCRIPTEN__)
		return pendingLaunchAction != LaunchAction::Error;
#else
		return false;
#endif
	}

	void Complete(LaunchAction action)
	{
		if (action != LaunchAction::Quit)
		{
			if (!ValidateLauncherSettings(result.settings, validationError)) return;
#if defined(__EMSCRIPTEN__)
			const char* kind=request.dataSources.selected==DataSource::Retail?"retail":"prototype";
			if(webStagedKind!=kind)
			{
				pendingLaunchAction=action;
				if(!webStageRequest)RequestWeb(3,kind);
				return;
			}
#endif
		}
		else validationError.clear();
		result.selection = LaunchSelection{};
		result.selection.action = action;
		if (action == LaunchAction::Continue && HasSave())
		{
			result.selection.hasSave = true;
			result.selection.save = request.saves[static_cast<std::size_t>(selectedSave)];
		}
		result.dataSources = request.dataSources;
		finished = true;
		SDL_Log("HP2Launcher: closing (%s)", action == LaunchAction::Continue ? "continue"
			: action == LaunchAction::NewGame ? "new game" : action == LaunchAction::Quit ? "quit" : "other");
	}

	void ChooseGameFolder()
	{
		dataMessage.clear();
#if defined(__EMSCRIPTEN__)
		RequestWeb(1,"retail");
#else
		std::string folder;
		if(HP2LauncherPlatform::PickFolder(folder))StartImport(folder);
#endif
	}

	void StartImport(const std::string& source)
	{
		if (importing || source.empty()) return;
		std::string destination = request.retailDataRoot;
#if defined(__EMSCRIPTEN__)
		destination = "/hp2import/out";
#else
		if (destination.empty() && !HP2DataImport::GetConventionalRetailDestination(destination))
		{
			dataMessage = "Could not determine the retail data destination.";
			return;
		}
#endif
		request.retailDataRoot = destination;
		importProgress.filesDone.store(0);
		importProgress.filesTotal.store(0);
		importProgress.cancelRequested.store(false);
		importDone.store(false);
		importing = true;
		importer = std::thread([this, source, destination]()
		{
			importSucceeded = HP2DataImport::RunRetailImport(
				source, destination, &importProgress, importOutput, importError);
			importDone.store(true, std::memory_order_release);
		});
	}

	void FinishImportIfReady()
	{
		if (!importing || !importDone.load(std::memory_order_acquire)) return;
		if (importer.joinable()) importer.join();
		importing = false;
		if (!importSucceeded)
		{
			// The importer's reason is for the log; the player gets one clear sentence.
			SDL_Log("HP2Launcher: import failed: %s", importError.empty() ? "unknown error" : importError.c_str());
			dataMessage = "This doesn\u2019t look like the original game folder. Choose the folder the game was "
				"installed to (it contains Maps, System and Textures), not a copy made by another tool.";
			return;
		}
#if defined(__EMSCRIPTEN__)
		if (!webPersistRequest)
		{
			const int id=NextWebRequest++;
			webPersistRequest=id;
			HP2WebCall(id,2,"");
			return;
		}
#endif
		RefreshRetailSource();
	}

	void RefreshRetailSource()
	{
		const std::string root =
#if defined(__EMSCRIPTEN__)
			"/hp2data/retail";
#else
			request.retailDataRoot;
#endif
		std::string canonicalRoot;
		std::string validationError;
		const bool valid=!root.empty()&&ValidateHP2DataRoot(root,canonicalRoot,validationError);
		for(DataSourceOption& option:options)if(option.source==DataSource::Retail)
		{
			option.root=valid?canonicalRoot:root;
			option.available=valid;
			option.error=valid?std::string():validationError;
		}
		request.dataSources.retailRoot=valid?canonicalRoot:root;
		request.dataSources.selected=DataSource::Retail;
		result.dataSources=request.dataSources;
		if(valid)
		{
			installedNotice="Your game is installed.";
			dataMessage.clear();
			screen=Screen::Home;
		}
		else dataMessage=validationError.empty()?"The imported files could not be validated.":validationError;
	}

	void Heading(const char* text)
	{
		if (titleFont) ImGui::PushFont(titleFont);
		ImGui::TextWrapped("%s", text);
		if (titleFont) ImGui::PopFont();
	}

	void SmallMuted(const std::string& text)
	{
		if (smallFont) ImGui::PushFont(smallFont);
		ColoredWrapped(ColorMuted, text);
		if (smallFont) ImGui::PopFont();
	}

	bool BackButton()
	{
		const bool pressed = SecondaryButton("Back", 140.0f);
		if (pressed) screen = Screen::Home;
		return pressed;
	}

	void DrawProgress()
	{
		if (importing)
		{
			const auto done=importProgress.filesDone.load(std::memory_order_relaxed);
			const auto total=importProgress.filesTotal.load(std::memory_order_relaxed);
			if (total > 0)
			{
				const float fraction=std::clamp(static_cast<float>(done)/static_cast<float>(total),0.0f,1.0f);
				const std::string label=std::to_string(done)+" of "+std::to_string(total)+" files";
				ImGui::ProgressBar(fraction,ImVec2(-1,32),label.c_str());
			}
			else ImGui::ProgressBar(-1.0f*static_cast<float>(ImGui::GetTime()),ImVec2(-1,32),"Checking the files…");
			Gap(4.0f);
			if (SecondaryButton("Cancel", 160.0f)) importProgress.cancelRequested.store(true,std::memory_order_relaxed);
			return;
		}
		const char* label = Launching() ? "Loading the game files…" : "Saving the game files in this browser…";
		ImGui::ProgressBar(-1.0f*static_cast<float>(ImGui::GetTime()),ImVec2(-1,32),label);
	}

	void DrawInstalling()
	{
		Heading("Installing your game");
		Gap(4.0f);
		DrawProgress();
		Gap(4.0f);
		SmallMuted("This only happens once. Keep this window open until it finishes.");
	}

	void DrawWelcome()
	{
		ColoredWrapped(ColorText, "To play, choose the folder where the game is installed on this computer.");
		Gap(8.0f);
		if (PrimaryButton("Choose your game folder")) ChooseGameFolder();
		if (!dataMessage.empty()) { Gap(4.0f); ColoredWrapped(ColorWarning, dataMessage); }
		Gap(16.0f);
		SmallMuted("Look for the folder named \u201CHarry Potter and the Chamber of Secrets\u201D. "
			"It contains the folders Maps, System, Textures, Sounds, Music and Help.");
		SmallMuted("Can\u2019t find it? Search your computer for HP2_Master.utx. It is inside the Textures "
			"folder, so choose the folder one level above Textures.");
#if defined(__EMSCRIPTEN__)
		SmallMuted("Your game files stay on this device. Nothing is sent anywhere.");
#endif
	}

	void DrawReady()
	{
		if (!installedNotice.empty()) ColoredWrapped(ColorSuccess, installedNotice);
		if (HasSave())
		{
			const SaveRecord& save = request.saves[static_cast<std::size_t>(selectedSave)];
			const std::string label = "Continue\n" + save.displayName + "  \u00B7  " + FormatDate(save.modifiedSeconds);
			RefreshThumbnail();
			if (thumbnail)
			{
				int w=0,h=0;
				SDL_QueryTexture(thumbnail,nullptr,nullptr,&w,&h);
				if (w > 0 && h > 0)
				{
					const float height = PrimaryHeight;
					ImGui::Image(reinterpret_cast<ImTextureID>(thumbnail),ImVec2(height*static_cast<float>(w)/static_cast<float>(h),height));
					ImGui::SameLine();
				}
			}
			if (PrimaryButton(label.c_str())) Complete(LaunchAction::Continue);
			if (SecondaryButton("New game")) Complete(LaunchAction::NewGame);
			if (request.saves.size() > 1)
			{
				std::vector<std::string> entries;std::vector<const char*> refs;
				entries.reserve(request.saves.size());refs.reserve(request.saves.size());
				for(const SaveRecord& item:request.saves) entries.push_back(item.displayName+"  \u00B7  "+FormatDate(item.modifiedSeconds));
				for(const auto& entry:entries)refs.push_back(entry.c_str());
				Gap(4.0f);
				SmallMuted("Continue from a different save:");
				ImGui::SetNextItemWidth(-1);
				ImGui::Combo("##save",&selectedSave,refs.data(),static_cast<int>(refs.size()));
			}
		}
		else if (PrimaryButton("Start playing")) Complete(LaunchAction::NewGame);
		if (!validationError.empty()) ColoredWrapped(ColorWarning, validationError);
	}

	void DrawFooter()
	{
		Gap(20.0f);
		ImGui::Separator();
		Gap(4.0f);
		if (SecondaryButton("Settings", 160.0f)) screen = Screen::Settings;
		ImGui::SameLine();
		if (SecondaryButton("Game files", 160.0f)) screen = Screen::GameFiles;
#if !defined(__EMSCRIPTEN__)
		ImGui::SameLine();
		if (SecondaryButton("Quit", 160.0f)) Complete(LaunchAction::Quit);
#endif
	}

	void DrawHome()
	{
#if defined(__EMSCRIPTEN__)
		if (!webStatusKnown)
		{
			ImGui::ProgressBar(-1.0f*static_cast<float>(ImGui::GetTime()),ImVec2(-1,32),"Looking for your game files…");
			return;
		}
#endif
		if (Installing() || Launching()) { DrawInstalling(); return; }
		if (!DataReady())
		{
			DrawWelcome();
#if !defined(__EMSCRIPTEN__)
			DrawFooter();
#endif
			return;
		}
		DrawReady();
		DrawFooter();
	}

	void DrawGraphics()
	{
		LauncherSettings& s = result.settings;
		if (caps.rendererChoice)
		{
#ifdef HP2_ENABLE_VULKAN_DRIVER
			const char* backends[] = {"XOpenGL", "Vulkan"};
			int backend = static_cast<int>(s.renderBackend);
			ImGui::SetNextItemWidth(-220.0f); if (ImGui::Combo("Renderer", &backend, backends, 2)) s.renderBackend = static_cast<RenderBackend>(backend);
#else
			ImGui::TextUnformatted("Renderer: XOpenGL");
			s.renderBackend = RenderBackend::XOpenGL;
#endif
		}
		if (caps.screenModes)
		{
			const char* modes[] = {"Windowed", "Fullscreen", "Borderless desktop"};
			int mode = static_cast<int>(s.screenMode); ImGui::SetNextItemWidth(-220.0f);
			if (ImGui::Combo("Screen mode", &mode, modes, 3)) s.screenMode = static_cast<ScreenMode>(mode);
		}
		if (caps.resolutions && !request.displayModes.empty())
		{
			int current = 0;
			for (std::size_t i = 0; i < request.displayModes.size(); ++i)
				if (request.displayModes[i].width == s.resolution.width && request.displayModes[i].height == s.resolution.height) current = static_cast<int>(i);
			std::vector<std::string> labels; std::vector<const char*> refs;
			labels.reserve(request.displayModes.size()); refs.reserve(request.displayModes.size());
			for (const auto& mode : request.displayModes) labels.push_back(ResolutionText(mode));
			for (const auto& label : labels) refs.push_back(label.c_str());
			ImGui::SetNextItemWidth(-220.0f);
			if (ImGui::Combo("Resolution", &current, refs.data(), static_cast<int>(refs.size()))) s.resolution = request.displayModes[static_cast<std::size_t>(current)];
		}
		ImGui::Checkbox("Vertical Sync", &s.verticalSync);
		const char* renderScales[] = {"50%", "67%", "75%", "85%", "100%"};
		int rs = 4; for (int i=0;i<5;i++) if (std::abs(s.renderScale-RenderScaleValues[i])<0.005) rs=i;
		ImGui::SetNextItemWidth(-220.0f); if (ImGui::Combo("Render scale", &rs, renderScales, 5)) s.renderScale=RenderScaleValues[rs];
		const char* uiScales[] = {"75%", "100%", "125%", "150%", "175%", "200%"};
		int us = 1; for (int i=0;i<6;i++) if (std::abs(s.uiScale-UIScaleValues[i])<0.005) us=i;
		ImGui::SetNextItemWidth(-220.0f); if (ImGui::Combo("UI scale", &us, uiScales, 6)) s.uiScale=UIScaleValues[us];
		const char* fpsLabels[] = {"Unlimited", "30", "60", "120", "144"}; int fps=0;
		for (int i=0;i<5;i++) if (s.frameRateLimit==FrameRateLimitValues[i]) fps=i;
		ImGui::SetNextItemWidth(-220.0f); if (ImGui::Combo("FPS cap", &fps, fpsLabels, 5)) s.frameRateLimit=FrameRateLimitValues[fps];
		ImGui::Checkbox("Show FPS", &s.showFPS);
		ImGui::Checkbox("Maintain vertical FOV", &s.maintainVerticalFOV);
		ImGui::Checkbox("Native text", &s.nativeText);
		const char* aaLabels[] = {"Off", "2x", "4x"}; int aa=0; for(int i=0;i<3;i++)if(s.antiAliasingSamples==AntiAliasingSampleValues[i])aa=i;
		ImGui::SetNextItemWidth(-220.0f); if(ImGui::Combo("Anti-aliasing",&aa,aaLabels,3))s.antiAliasingSamples=AntiAliasingSampleValues[aa];
		const char* anisoLabels[] = {"Off", "4x", "8x", "16x"}; int an=0;for(int i=0;i<4;i++)if(s.anisotropy==AnisotropyValues[i])an=i;
		ImGui::SetNextItemWidth(-220.0f);if(ImGui::Combo("Anisotropy",&an,anisoLabels,4))s.anisotropy=AnisotropyValues[an];
		float brightness=static_cast<float>(s.brightness);ImGui::SetNextItemWidth(-220.0f);if(ImGui::SliderFloat("Brightness",&brightness,0.1f,1.0f,"%.2f"))s.brightness=brightness;
		const char* texture[] = {"Low", "Medium", "High"};EnumCombo("Texture detail",s.textureDetail,texture);
		const char* object[] = {"Very low", "Low", "Medium", "High", "Very high"};EnumCombo("Object detail",s.objectDetail,object);
	}

	void DrawAudio()
	{
		LauncherSettings& s=result.settings;
		ImGui::Checkbox("Sound enabled",&s.soundEnabled);
		ImGui::BeginDisabled(!s.soundEnabled);
		int effects=static_cast<int>(std::lround(s.soundVolume*100.0));ImGui::SetNextItemWidth(-220.0f);if(ImGui::SliderInt("Effects volume",&effects,0,100,"%d%%"))s.soundVolume=effects/100.0;
		int music=static_cast<int>(std::lround(s.musicVolume*100.0));ImGui::SetNextItemWidth(-220.0f);if(ImGui::SliderInt("Music volume",&music,0,100,"%d%%"))s.musicVolume=music/100.0;
		ImGui::EndDisabled();
	}

	void DrawGameplay()
	{
		LauncherSettings& s=result.settings;
		const char* difficulties[]={"Easy","Medium","Hard"};EnumCombo("Difficulty",s.difficulty,difficulties);
		float sensitivity=static_cast<float>(s.mouseSensitivity);ImGui::SetNextItemWidth(-220.0f);if(ImGui::SliderFloat("Mouse sensitivity",&sensitivity,0.2f,10.0f,"%.1f"))s.mouseSensitivity=sensitivity;
		ImGui::Checkbox("Invert mouse",&s.invertMouse);
		const char* controlModes[]={"Classic","Modern"};EnumCombo("Controls mode",s.controlMode,controlModes);
		ImGui::Checkbox("Joystick",&s.joystickEnabled);
		ImGui::Checkbox("Auto-center camera",&s.autoCenterCamera);
		ImGui::Checkbox("Move while casting",&s.moveWhileCasting);
		ImGui::Checkbox("Auto-quaff potions",&s.autoQuaff);
		ImGui::Checkbox("Screen flashes",&s.screenFlashes);
	}

	void DrawSettings()
	{
		BackButton();
		Gap(4.0f);
		Heading("Settings");
		SmallMuted("Changes apply when the game starts.");
		Gap(4.0f);
		if (ImGui::CollapsingHeader("Sound", ImGuiTreeNodeFlags_DefaultOpen)) DrawAudio();
		if (ImGui::CollapsingHeader("Controls and gameplay", ImGuiTreeNodeFlags_DefaultOpen)) DrawGameplay();
		if (ImGui::CollapsingHeader("Graphics")) DrawGraphics();
	}

	void DrawGameFiles()
	{
		BackButton();
		Gap(4.0f);
		Heading("Game files");
		if (Installing()) DrawProgress();
		else
		{
			if (DataReady()) ColoredWrapped(ColorSuccess, "Installed and ready to play.");
			else ColoredWrapped(ColorWarning, "Not installed yet.");
			Gap(4.0f);
			if (SecondaryButton(DataReady() ? "Import the game folder again" : "Choose your game folder")) ChooseGameFolder();
		}
		if (!dataMessage.empty()) ColoredWrapped(ColorWarning, dataMessage);
#if !defined(__EMSCRIPTEN__)
		Gap(8.0f);
		if (ImGui::CollapsingHeader("Advanced")) DrawAdvancedData();
#endif
	}

#if !defined(__EMSCRIPTEN__)
	// Desktop developer and diagnostic controls: the prototype data profile,
	// the active roots, and the user/log folders.
	void DrawAdvancedData()
	{
		int selected=static_cast<int>(request.dataSources.selected);
		const char* sources[]={"Retail game data","Prototype data"};
		ImGui::SetNextItemWidth(-220.0f);
		if(ImGui::Combo("Data source",&selected,sources,2))request.dataSources.selected=static_cast<DataSource>(selected);
		const DataSourceOption* option=FindOption(options,request.dataSources.selected);
		if(option)
		{
			ImGui::TextWrapped("Folder: %s",option->root.empty()?"(not configured)":option->root.c_str());
			ColoredWrapped(option->available?ColorSuccess:ColorWarning,option->available?"Available":"Unavailable");
			if(!option->error.empty())ImGui::TextWrapped("%s",option->error.c_str());
		}
		if(caps.nativeFolderPicker&&SecondaryButton("Choose prototype folder…"))
		{
			std::string folder;
			if(HP2LauncherPlatform::PickFolder(folder))
			{
				std::string canonicalRoot,error;
				const bool valid=ValidateHP2DataRoot(folder,canonicalRoot,error);
				request.dataSources.prototypeRoot=valid?canonicalRoot:folder;
				for(auto& item:options)if(item.source==DataSource::Prototype){item.root=valid?canonicalRoot:folder;item.available=valid;item.error=valid?std::string():error;}
			}
		}
		if(caps.revealPaths)
		{
			if(SecondaryButton("Open user folder"))HP2LauncherPlatform::RevealPath(request.userRoot);
			if(SecondaryButton("Show log",-1.0f,!request.logPath.empty()))HP2LauncherPlatform::RevealPath(request.logPath);
		}
	}
#endif

#if defined(__EMSCRIPTEN__)
	void RequestWeb(int operation,const std::string& argument)
	{
		const int id=NextWebRequest++;
		if(operation==1)webPickRequest=id;
		else if(operation==3)webStageRequest=id;
		else if(operation==2)webPersistRequest=id;
		webPendingKind=argument;
		HP2WebCall(id,operation,argument.c_str());
	}
#endif

	void RenderSmoke()
	{
		if(smokeFrames<0||++frameCount<smokeFrames)return;
		const char* artifact=std::getenv("HP2_ARTIFACT_DIR");
		if(artifact&&*artifact)
		{
			int width=0,height=0;SDL_GetRendererOutputSize(renderer,&width,&height);
			if(width>0&&height>0)
			{
				std::vector<unsigned char> pixels(static_cast<std::size_t>(width)*height*4);
				if(SDL_RenderReadPixels(renderer,nullptr,SDL_PIXELFORMAT_RGBA32,pixels.data(),width*4)==0)
				{
					SDL_Surface* surface=SDL_CreateRGBSurfaceWithFormatFrom(pixels.data(),width,height,32,width*4,SDL_PIXELFORMAT_RGBA32);
					if(surface){std::filesystem::path output=std::filesystem::u8path(artifact)/"launcher.bmp";SDL_SaveBMP(surface,output.u8string().c_str());SDL_FreeSurface(surface);}
				}
			}
		}
		SDL_Log("HP2Launcher: smoke complete");
		Complete(LaunchAction::Quit);
	}
};

FLauncherSession::FLauncherSession(const LauncherRequest& request,const LauncherCapabilities& capabilities)
	:State(new Impl(request,capabilities)){}
FLauncherSession::~FLauncherSession()=default;

bool FLauncherSession::Open(std::string& Error)
{
	Impl& s=*State;
	if(!(SDL_WasInit(SDL_INIT_VIDEO)&SDL_INIT_VIDEO))
	{
		if(SDL_InitSubSystem(SDL_INIT_VIDEO)!=0){Error=SDL_GetError();return false;}
		s.ownsVideo=true;
	}
	s.window=SDL_CreateWindow("Harry Potter 2",SDL_WINDOWPOS_CENTERED,SDL_WINDOWPOS_CENTERED,960,680,SDL_WINDOW_RESIZABLE|SDL_WINDOW_ALLOW_HIGHDPI);
	if(!s.window){Error=SDL_GetError();return false;}
	s.renderer=SDL_CreateRenderer(s.window,-1,SDL_RENDERER_ACCELERATED|SDL_RENDERER_PRESENTVSYNC);
	if(!s.renderer)s.renderer=SDL_CreateRenderer(s.window,-1,SDL_RENDERER_SOFTWARE);
	if(!s.renderer){Error=SDL_GetError();return false;}
	IMGUI_CHECKVERSION();
	ImGui::CreateContext();
	s.imguiContext=true;
	ApplyStyle();
	if(!ImGui_ImplSDL2_InitForSDLRenderer(s.window,s.renderer))
	{
		Error="Could not initialize Dear ImGui SDL input.";
		ImGui::DestroyContext();s.imguiContext=false;
		return false;
	}
	s.sdlBackend=true;
	if(!ImGui_ImplSDLRenderer2_Init(s.renderer))
	{
		Error="Could not initialize Dear ImGui SDL renderer.";
		ImGui_ImplSDL2_Shutdown();s.sdlBackend=false;
		ImGui::DestroyContext();s.imguiContext=false;
		return false;
	}
	s.rendererBackend=true;
	s.fontsDirectory=LocateFonts();
	const std::filesystem::path fonts=std::filesystem::u8path(s.fontsDirectory);
	const std::filesystem::path latin=fonts/"LiberationSans-Regular.ttf";
	if(std::filesystem::exists(latin))
	{
		ImFontAtlas* atlas=ImGui::GetIO().Fonts;
		constexpr float bodySize=20.0f;
		ImFont* base=atlas->AddFontFromFileTTF(latin.u8string().c_str(),bodySize);
		const std::filesystem::path cjk=fonts/"NotoSansSC-Regular.otf";
		if(base&&std::filesystem::exists(cjk)){ImFontConfig merge;merge.MergeMode=true;atlas->AddFontFromFileTTF(cjk.u8string().c_str(),bodySize,&merge,atlas->GetGlyphRangesChineseFull());}
		const std::filesystem::path bold=fonts/"LiberationSans-Bold.ttf";
		s.titleFont=atlas->AddFontFromFileTTF((std::filesystem::exists(bold)?bold:latin).u8string().c_str(),34.0f);
		s.smallFont=atlas->AddFontFromFileTTF(latin.u8string().c_str(),16.0f);
		atlas->Build();
	}
	else SDL_Log("HP2Launcher: bundled font missing, using default");
	s.opened=true;
#if defined(__EMSCRIPTEN__)
	{
		const int id=NextWebRequest++;
		s.webStatusRequest=id;
		HP2WebCall(id,0,"");
	}
#endif
	Error.clear();
	return true;
}

bool FLauncherSession::Frame()
{
	Impl& s=*State;if(!s.opened||s.finished)return false;
#if defined(__EMSCRIPTEN__)
	{
		std::lock_guard<std::mutex> lock(WebMutex);
		auto consume=[&](int& requestId,int type)
		{
			if(!requestId)return;
			auto found=WebResults.find(requestId);
			if(found==WebResults.end())return;
			const auto answer=std::move(found->second);
			WebResults.erase(found);
			requestId=0;
			if(!answer.first)
			{
				s.dataMessage=answer.second;
				if(type==0)s.webStatusKnown=true;
				if(type==2||type==3){s.webRefreshAfterStage=false;s.pendingLaunchAction=LaunchAction::Error;}
				return;
			}
			if(type==0)
			{
				for(auto& option:s.options)
				{
					if(option.source==DataSource::Retail){option.root="/hp2data/retail";option.available=answer.second.find("\"retail\":true")!=std::string::npos;option.error.clear();}
					else {option.root="/hp2data/prototype";option.available=answer.second.find("\"prototype\":true")!=std::string::npos;option.error.clear();}
				}
				s.request.dataSources.retailRoot="/hp2data/retail";
				s.request.dataSources.prototypeRoot="/hp2data/prototype";
				if(!s.DataReady())
				{
					// Fall back to whichever source the browser does hold.
					for(const auto& option:s.options)if(option.available){s.request.dataSources.selected=option.source;break;}
				}
				s.webStatusKnown=true;
				if(!s.DataReady()&&!s.webPromptedImport&&s.smokeFrames<0)
				{
					s.webPromptedImport=true;
					s.RequestWeb(1,"retail");
				}
			}
			else if(type==1 && s.webPendingKind=="retail" && !answer.second.empty())s.StartImport(answer.second);
			else if(type==2)
			{
				s.webRefreshAfterStage=true;
				s.RequestWeb(3,"retail");
			}
			else if(type==3)
			{
				s.webStagedKind=s.webPendingKind;
				if(s.webRefreshAfterStage)
				{
					s.webRefreshAfterStage=false;
					s.RefreshRetailSource();
				}
				if(s.pendingLaunchAction!=LaunchAction::Error)
				{
					const LaunchAction action=s.pendingLaunchAction;
					s.pendingLaunchAction=LaunchAction::Error;
					s.Complete(action);
				}
			}
		};
		consume(s.webStatusRequest,0);
		consume(s.webPickRequest,1);
		consume(s.webPersistRequest,2);
		consume(s.webStageRequest,3);
	}
#endif
	SDL_Event event;
	while(SDL_PollEvent(&event)){ImGui_ImplSDL2_ProcessEvent(&event);if(event.type==SDL_QUIT)s.Complete(LaunchAction::Quit);}
	s.FinishImportIfReady();
	ImGui_ImplSDLRenderer2_NewFrame();
	ImGui_ImplSDL2_NewFrame();
	int width = 0, height = 0, outputWidth = 0, outputHeight = 0;
	SDL_GetWindowSize(s.window, &width, &height);
	SDL_GetRendererOutputSize(s.renderer, &outputWidth, &outputHeight);
	if (width > 0 && height > 0 && outputWidth > 0 && outputHeight > 0)
	{
		if (SDL_RenderSetLogicalSize(s.renderer, width, height) != 0)
			SDL_Log("HP2Launcher: could not set logical renderer size: %s", SDL_GetError());
		ImGui::GetIO().DisplayFramebufferScale = ImVec2(1.0f, 1.0f);
	}
	ImGui::NewFrame();
	ImGui::SetNextWindowPos(ImVec2(0, 0));
	ImGui::SetNextWindowSize(ImVec2(static_cast<float>(width), static_cast<float>(height)));
	ImGui::Begin("##launcher",nullptr,ImGuiWindowFlags_NoDecoration|ImGuiWindowFlags_NoMove|ImGuiWindowFlags_NoResize|ImGuiWindowFlags_NoSavedSettings);
	// One centred column; narrow windows use the full width.
	const float column = std::min(ColumnWidth, std::max(0.0f, static_cast<float>(width) - 48.0f));
	const float top = height > 700 ? 56.0f : 24.0f;
	ImGui::SetCursorPos(ImVec2(std::max(24.0f, (static_cast<float>(width) - column) * 0.5f), top));
	ImGui::BeginChild("column", ImVec2(column, static_cast<float>(height) - top - 24.0f));
	s.Heading("Harry Potter and the Chamber of Secrets");
	Gap(8.0f);
	switch(s.screen)
	{
	case Screen::Home: s.DrawHome(); break;
	case Screen::Settings: s.DrawSettings(); break;
	case Screen::GameFiles: s.DrawGameFiles(); break;
	}
	ImGui::EndChild();
	ImGui::End();
	ImGui::Render();
	SDL_SetRenderDrawColor(s.renderer,18,20,28,255);
	SDL_RenderClear(s.renderer);
	ImGui_ImplSDLRenderer2_RenderDrawData(ImGui::GetDrawData(), s.renderer);
	const bool firstFrame = s.frameCount == 0;
	s.RenderSmoke();
	SDL_RenderPresent(s.renderer);
	if(firstFrame) SDL_Log("HP2Launcher: ready");
	if(s.smokeFrames<0) ++s.frameCount;
	return !s.finished;
}

const LauncherResult& FLauncherSession::Result()const{return State->result;}

bool RunLauncher(const LauncherRequest& Request,LauncherResult& Result,std::string& Error)
{
	FLauncherSession session(Request,HP2LauncherPlatform::CurrentCapabilities());
	if(!session.Open(Error))return false;
	while(session.Frame())SDL_Delay(8);
	Result=session.Result();Error.clear();return true;
}
}
