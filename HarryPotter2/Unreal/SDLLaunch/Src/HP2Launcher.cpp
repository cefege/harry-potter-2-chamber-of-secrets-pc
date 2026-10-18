/*=============================================================================
	HP2Launcher.cpp: SDL2 + Dear ImGui launcher shared by desktop and web.
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
constexpr float ItemHeight = 44.0f;
constexpr const char* TabNames[] = {"Play", "Graphics", "Audio", "Gameplay", "Game Data"};
constexpr const char* ArchiveExtensions[] = {"zip", "tar", "tar.gz", "tgz", "tar.bz2", "tbz2", "tar.xz", "txz", "7z"};

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

bool UiButton(const char* label, bool enabled = true)
{
	ImGui::PushStyleVar(ImGuiStyleVar_FramePadding, ImVec2(12.0f, 13.0f));
	if (!enabled) ImGui::BeginDisabled();
	const bool pressed = ImGui::Button(label, ImVec2(-1.0f, ItemHeight));
	if (!enabled) ImGui::EndDisabled();
	ImGui::PopStyleVar();
	return pressed && enabled;
}

void SetComboHeight()
{
	ImGui::SetNextItemWidth(-1.0f);
}

template <typename T, std::size_t N>
bool EnumCombo(const char* label, T& value, const char* const (&labels)[N])
{
	int selected = static_cast<int>(value);
	SetComboHeight();
	if (!ImGui::Combo(label, &selected, labels, static_cast<int>(N))) return false;
	value = static_cast<T>(selected);
	return true;
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
	const arg = UTF8ToString(argument);
	switch (operation) {
	case 0: api.dataSourceStatus(requestId); break;
	case 1: api.pickFolder(requestId, arg); break;
	case 2: api.persistImport(requestId, '/hp2import/out', 'retail'); break;
	case 3: api.stageDataSource(requestId, arg); break;
	case 4: api.syncUser(requestId); break;
	}
});
#endif

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
	int tab = 0;
	int selectedSave = -1;
	std::string validationError;
	std::string dataMessage;
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
#if defined(__EMSCRIPTEN__)
	int webStatusRequest = 0;
	int webPickRequest = 0;
	int webStageRequest = 0;
	int webPersistRequest = 0;
	std::string webPendingKind;
	std::string webStagedKind;
	bool webRefreshAfterStage = false;
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
			dataMessage = importError.empty() ? "Import failed." : importError;
			return;
		}
#if defined(__EMSCRIPTEN__)
		if (!webPersistRequest)
		{
			const int id=NextWebRequest++;
			webPersistRequest=id;
			HP2WebCall(id,2,"");
			dataMessage="Saving imported game data to browser storage…";
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
		dataMessage=valid?"Retail game data imported successfully.":(validationError.empty()?"Imported data could not be validated.":validationError);
	}


	void DrawGraphics()
	{
		LauncherSettings& s = result.settings;
		if (caps.screenModes)
		{
			const char* modes[] = {"Windowed", "Fullscreen", "Borderless desktop"};
			int mode = static_cast<int>(s.screenMode); ImGui::SetNextItemWidth(-1);
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
			ImGui::SetNextItemWidth(-1);
			if (ImGui::Combo("Resolution", &current, refs.data(), static_cast<int>(refs.size()))) s.resolution = request.displayModes[static_cast<std::size_t>(current)];
		}
		ImGui::Checkbox("Vertical Sync", &s.verticalSync);
		const char* renderScales[] = {"50%", "67%", "75%", "85%", "100%"};
		int rs = 4; for (int i=0;i<5;i++) if (std::abs(s.renderScale-RenderScaleValues[i])<0.005) rs=i;
		ImGui::SetNextItemWidth(-1); if (ImGui::Combo("Render scale", &rs, renderScales, 5)) s.renderScale=RenderScaleValues[rs];
		const char* uiScales[] = {"75%", "100%", "125%", "150%", "175%", "200%"};
		int us = 1; for (int i=0;i<6;i++) if (std::abs(s.uiScale-UIScaleValues[i])<0.005) us=i;
		ImGui::SetNextItemWidth(-1); if (ImGui::Combo("UI scale", &us, uiScales, 6)) s.uiScale=UIScaleValues[us];
		const char* fpsLabels[] = {"Unlimited", "30", "60", "120", "144"}; int fps=0;
		for (int i=0;i<5;i++) if (s.frameRateLimit==FrameRateLimitValues[i]) fps=i;
		ImGui::SetNextItemWidth(-1); if (ImGui::Combo("FPS cap", &fps, fpsLabels, 5)) s.frameRateLimit=FrameRateLimitValues[fps];
		ImGui::Checkbox("Show FPS", &s.showFPS);
		ImGui::Checkbox("Maintain vertical FOV", &s.maintainVerticalFOV);
		ImGui::Checkbox("Native text", &s.nativeText);
		const char* aaLabels[] = {"Off", "2x", "4x"}; int aa=0; for(int i=0;i<3;i++)if(s.antiAliasingSamples==AntiAliasingSampleValues[i])aa=i;
		ImGui::SetNextItemWidth(-1); if(ImGui::Combo("Anti-aliasing",&aa,aaLabels,3))s.antiAliasingSamples=AntiAliasingSampleValues[aa];
		const char* anisoLabels[] = {"Off", "4x", "8x", "16x"}; int an=0;for(int i=0;i<4;i++)if(s.anisotropy==AnisotropyValues[i])an=i;
		ImGui::SetNextItemWidth(-1);if(ImGui::Combo("Anisotropy",&an,anisoLabels,4))s.anisotropy=AnisotropyValues[an];
		float brightness=static_cast<float>(s.brightness);if(ImGui::SliderFloat("Brightness",&brightness,0.1f,1.0f,"%.2f"))s.brightness=brightness;
		const char* texture[] = {"Low", "Medium", "High"};EnumCombo("Texture detail",s.textureDetail,texture);
		const char* object[] = {"Very low", "Low", "Medium", "High", "Very high"};EnumCombo("Object detail",s.objectDetail,object);
	}

	void DrawAudio()
	{
		LauncherSettings& s=result.settings;
		ImGui::Checkbox("Sound enabled",&s.soundEnabled);
		ImGui::BeginDisabled(!s.soundEnabled);
		float effects=static_cast<float>(s.soundVolume);if(ImGui::SliderFloat("Effects volume",&effects,0.0f,1.0f,"%.0f%%",100.0f))s.soundVolume=effects;
		float music=static_cast<float>(s.musicVolume);if(ImGui::SliderFloat("Music volume",&music,0.0f,1.0f,"%.0f%%",100.0f))s.musicVolume=music;
		ImGui::EndDisabled();
	}

	void DrawGameplay()
	{
		LauncherSettings& s=result.settings;
		float sensitivity=static_cast<float>(s.mouseSensitivity);if(ImGui::SliderFloat("Mouse sensitivity",&sensitivity,0.2f,10.0f,"%.1f"))s.mouseSensitivity=sensitivity;
		ImGui::Checkbox("Invert mouse",&s.invertMouse);
		const char* controlModes[]={"Classic","Modern"};EnumCombo("Controls mode",s.controlMode,controlModes);
		ImGui::Checkbox("Joystick",&s.joystickEnabled);
		ImGui::Checkbox("Auto-center camera",&s.autoCenterCamera);
		ImGui::Checkbox("Move while casting",&s.moveWhileCasting);
		ImGui::Checkbox("Auto-quaff potions",&s.autoQuaff);
		ImGui::Checkbox("Screen flashes",&s.screenFlashes);
		const char* difficulties[]={"Easy","Medium","Hard"};EnumCombo("Difficulty",s.difficulty,difficulties);
	}

	void DrawPlay()
	{
		ImGui::TextUnformatted("Saved games");
		if (request.saves.empty()) ImGui::TextDisabled("No saved games found.");
		else
		{
			std::vector<std::string> entries;std::vector<const char*> refs;entries.reserve(request.saves.size());refs.reserve(request.saves.size());
			for(const SaveRecord& save:request.saves)
			{
				entries.push_back(save.displayName+"  —  "+FormatDate(save.modifiedSeconds)+"  —  "+std::to_string(save.size/1024)+" KB");
			}
			for(const auto& entry:entries)refs.push_back(entry.c_str());
			if(selectedSave<0)selectedSave=0;
			ImGui::SetNextItemWidth(-1);
			ImGui::Combo("Save",&selectedSave,refs.data(),static_cast<int>(refs.size()));
			RefreshThumbnail();
			if(thumbnail)
			{
				int w=0,h=0;
				SDL_QueryTexture(thumbnail,nullptr,nullptr,&w,&h);
				ImGui::Image(reinterpret_cast<ImTextureID>(thumbnail),ImVec2(static_cast<float>(w),static_cast<float>(h)));
			}
			if(HasSave()) ImGui::Text("%s",FormatDate(request.saves[static_cast<std::size_t>(selectedSave)].modifiedSeconds).c_str());
		}
		if(!validationError.empty())ImGui::TextWrapped("%s",validationError.c_str());
		if(UiButton("Continue",HasSave()))Complete(LaunchAction::Continue);
		if(UiButton("New Game"))Complete(LaunchAction::NewGame);
		if(UiButton("Quit"))Complete(LaunchAction::Quit);
	}

	void DrawGameData()
	{
		int selected=static_cast<int>(request.dataSources.selected);
		const char* sources[]={"Retail game data","Prototype data"};
		ImGui::SetNextItemWidth(-1);
		if(ImGui::Combo("Data source",&selected,sources,2))
		{
			request.dataSources.selected=static_cast<DataSource>(selected);
#if defined(__EMSCRIPTEN__)
			const char* kind=selected==static_cast<int>(DataSource::Retail)?"retail":"prototype";
			if(webStagedKind!=kind)RequestWeb(3,kind);
#endif
		}
		const DataSourceOption* option=FindOption(options,request.dataSources.selected);
		if(option)
		{
			ImGui::TextWrapped("Folder: %s",option->root.empty()?"(not configured)":option->root.c_str());
			ImGui::TextColored(option->available?ImVec4(0.2f,0.85f,0.3f,1):ImVec4(1,0.55f,0.3f,1),"%s",option->available?"Available":"Unavailable");
			if(!option->error.empty())ImGui::TextWrapped("%s",option->error.c_str());
		}
		if(caps.nativeFolderPicker)
		{
			if(UiButton("Choose prototype folder…"))
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
			if(UiButton("Import retail game…"))
			{
				std::string folder;
				if(HP2LauncherPlatform::PickFolder(folder))StartImport(folder);
			}
			if(UiButton("Import retail archive…"))
			{
				std::string file;
				std::vector<std::string> extensions(ArchiveExtensions,ArchiveExtensions+9);
				if(HP2LauncherPlatform::PickFile(file,extensions))StartImport(file);
			}
		}
#if defined(__EMSCRIPTEN__)
		else
		{
			if(UiButton("Choose prototype folder…"))RequestWeb(1,"prototype");
			if(UiButton("Import retail game…"))RequestWeb(1,"retail");
		}
#endif
		if(caps.revealPaths)
		{
			if(UiButton("Open user folder"))HP2LauncherPlatform::RevealPath(request.userRoot);
			if(UiButton("Show log",!request.logPath.empty()))HP2LauncherPlatform::RevealPath(request.logPath);
		}
		if(importing)
		{
			const auto done=importProgress.filesDone.load(std::memory_order_relaxed);
			const auto total=importProgress.filesTotal.load(std::memory_order_relaxed);
			const float fraction=total>0?std::clamp(static_cast<float>(done)/static_cast<float>(total),0.0f,1.0f):0.0f;
			ImGui::ProgressBar(fraction,ImVec2(-1,24),total>0?"Importing files":"Preparing import…");
			if(UiButton("Cancel import"))importProgress.cancelRequested.store(true,std::memory_order_relaxed);
		}
		if(!dataMessage.empty())ImGui::TextWrapped("%s",dataMessage.c_str());
	}

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
	s.window=SDL_CreateWindow("Harry Potter 2",SDL_WINDOWPOS_CENTERED,SDL_WINDOWPOS_CENTERED,960,640,SDL_WINDOW_RESIZABLE|SDL_WINDOW_ALLOW_HIGHDPI);
	if(!s.window){Error=SDL_GetError();return false;}
	s.renderer=SDL_CreateRenderer(s.window,-1,SDL_RENDERER_ACCELERATED|SDL_RENDERER_PRESENTVSYNC);
	if(!s.renderer)s.renderer=SDL_CreateRenderer(s.window,-1,SDL_RENDERER_SOFTWARE);
	if(!s.renderer){Error=SDL_GetError();return false;}
	IMGUI_CHECKVERSION();
	ImGui::CreateContext();
	s.imguiContext=true;
	// This launcher's layout is fixed (one window, no dockable panes
	// worth persisting), so there is nothing worth saving across runs.
	// Left at ImGui's own default, `io.IniFilename` is the relative
	// path "imgui.ini", written into whatever the process's current
	// working directory happens to be -- the repo root for every
	// ctest-invoked run and most manual launches from a checkout,
	// which is exactly the ad-hoc-artifacts-outside-HP2_ARTIFACT_DIR
	// pattern AGENTS.md's Non-Negotiable 2 rules out.
	ImGui::GetIO().IniFilename=nullptr;
	ImGui::StyleColorsDark();
	ImGuiStyle& style=ImGui::GetStyle();
	style.FramePadding.y=std::max(style.FramePadding.y,16.0f);
	style.TouchExtraPadding=ImVec2(4,4);
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
	s.fontsDirectory=LocateFonts();	ImFontConfig config;config.SizePixels=18.0f;
	const std::filesystem::path latin=std::filesystem::u8path(s.fontsDirectory)/"LiberationSans-Regular.ttf";	if(std::filesystem::exists(latin))
	{
		ImFont* base=ImGui::GetIO().Fonts->AddFontFromFileTTF(latin.u8string().c_str(),config.SizePixels,&config);
		const std::filesystem::path cjk=std::filesystem::u8path(s.fontsDirectory)/"NotoSansSC-Regular.otf";
		if(base&&std::filesystem::exists(cjk)){ImFontConfig merge;merge.MergeMode=true;merge.SizePixels=config.SizePixels;ImGui::GetIO().Fonts->AddFontFromFileTTF(cjk.u8string().c_str(),config.SizePixels,&merge,ImGui::GetIO().Fonts->GetGlyphRangesChineseFull());}
		ImGui::GetIO().Fonts->Build();
	}
	else SDL_Log("HP2Launcher: bundled font missing, using default");
	s.opened=true;
	if(s.request.errorMessage.size())s.dataMessage=s.request.errorMessage;
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
			if(!answer.first){s.dataMessage=answer.second;return;}
			if(type==0)
			{
				for(auto& option:s.options)
				{
					if(option.source==DataSource::Retail){option.root="/hp2data/retail";option.available=answer.second.find("\"retail\":true")!=std::string::npos;option.error.clear();}
					else {option.root="/hp2data/prototype";option.available=answer.second.find("\"prototype\":true")!=std::string::npos;option.error.clear();}
				}
				s.request.dataSources.retailRoot="/hp2data/retail";
				s.request.dataSources.prototypeRoot="/hp2data/prototype";
			}
			else if(type==1 && s.webPendingKind=="retail" && !answer.second.empty())s.StartImport(answer.second);
			else if(type==1 && s.webPendingKind=="prototype")
			{
				s.request.dataSources.prototypeRoot="/hp2data/prototype";
				s.request.dataSources.selected=DataSource::Prototype;
				for(auto& option:s.options)if(option.source==DataSource::Prototype){option.root="/hp2data/prototype";option.available=true;option.error.clear();}
				const int id=NextWebRequest++;s.webStageRequest=id;s.webPendingKind="prototype";HP2WebCall(id,3,"prototype");
			}
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
	ImGui::TextUnformatted("Harry Potter 2");ImGui::Separator();
	if (width >= 800)
	{
		const float bodyY = ImGui::GetCursorPosY();
		for (int i = 0; i < 5; ++i)
		{
			ImGui::SetCursorPos(ImVec2(0.0f, bodyY + i * ItemHeight));
			if (ImGui::Selectable(TabNames[i], s.tab == i, 0, ImVec2(150.0f, ItemHeight))) s.tab = i;
		}
		ImGui::SetCursorPos(ImVec2(160.0f, bodyY));
		ImGui::BeginChild("content", ImVec2(0, -1), false);
	}
	else
	{
		ImGui::BeginTabBar("launcher-tabs");
		for(int i=0;i<5;i++)if(ImGui::BeginTabItem(TabNames[i],nullptr,s.tab==i?ImGuiTabItemFlags_SetSelected:0)){s.tab=i;ImGui::EndTabItem();}
		ImGui::EndTabBar();ImGui::BeginChild("content",ImVec2(0,-1),false);
	}
	switch(s.tab){case 0:s.DrawPlay();break;case 1:s.DrawGraphics();break;case 2:s.DrawAudio();break;case 3:s.DrawGameplay();break;case 4:s.DrawGameData();break;default:break;}
	ImGui::EndChild();
	ImGui::End();
	ImGui::Render();
	SDL_SetRenderDrawColor(s.renderer,18,20,26,255);
	SDL_RenderClear(s.renderer);
	ImGui_ImplSDLRenderer2_RenderDrawData(ImGui::GetDrawData(), s.renderer);
	const bool firstFrame = s.frameCount == 0;
	s.RenderSmoke();
	SDL_RenderPresent(s.renderer);
	if(firstFrame) SDL_Log("HP2Launcher: ready");
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
