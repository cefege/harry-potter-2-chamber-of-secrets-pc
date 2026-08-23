#include <cstdlib>
#include "Core.h"
#include "FMallocAnsi.h"
#include "HP2LaunchPolicy.h"
#include "HP2Paths.h"
#include "HP2LauncherStore.h"

#include <algorithm>
#include <cmath>
#include <cctype>
#include <cstdio>
#include <fstream>
#include <filesystem>
#include <limits>
#include <iterator>
#include <string>
#include <sys/stat.h>
#include <unistd.h>
#include <vector>

namespace fs = std::filesystem;
using namespace HP2Launcher;

namespace
{
int Failures = 0;
FMallocAnsi RuntimeMalloc;

void Expect(bool condition, const std::string& message)
{
	if (!condition)
	{
		std::fprintf(stderr, "FAIL: %s\n", message.c_str());
		++Failures;
	}
}

struct TemporaryRoots
{
	fs::path root;
	fs::path system;
	fs::path user;

	TemporaryRoots()
	{
		char pattern[] = "/tmp/hp2-launcher-tests.XXXXXX";
		char* created = ::mkdtemp(pattern);
		if (!created)
		{
			std::perror("mkdtemp");
			std::exit(2);
		}
		root = created;
		system = root / "System";
		user = root / "User";
		fs::create_directories(system);
		fs::create_directories(user);
	}

	~TemporaryRoots()
	{
		std::error_code ignored;
		fs::remove_all(root, ignored);
	}

	LauncherPaths paths() const
	{
		return {user.string(), system.string()};
	}
};

void WriteBytes(const fs::path& path, const std::string& bytes)
{
	std::ofstream stream(path, std::ios::binary | std::ios::trunc);
	if (!stream)
	{
		std::fprintf(stderr, "Unable to create test file: %s\n", path.c_str());
		std::exit(2);
	}
	stream.write(bytes.data(), static_cast<std::streamsize>(bytes.size()));
	if (!stream)
	{
		std::fprintf(stderr, "Unable to write test file: %s\n", path.c_str());
		std::exit(2);
	}
}

std::string ReadBytes(const fs::path& path)
{
	std::ifstream stream(path, std::ios::binary);
	return std::string(std::istreambuf_iterator<char>(stream), std::istreambuf_iterator<char>());
}

void WriteDefaultTemplates(const TemporaryRoots& roots)
{
	WriteBytes(roots.system / "Default.ini",
		"; immutable game default\n"
		"[Engine.Engine]\n"
		"ViewportManager=WinDrv.WindowsClient\n"
		"GameRenderDevice=D3DDrv.D3DRenderDevice\n"
		"[Engine.GameEngine]\n"
		"UseSound=True\n"
		"FrameRateLimit=60.000000\n"
		"[SDLDrv.SDLClient]\n"
		"WindowedViewportX=800\nWindowedViewportY=600\n"
		"FullscreenViewportX=800\nFullscreenViewportY=600\n"
		"StartupFullscreen=False\nBrightness=0.4\nUseJoystick=True\n"
		"ShowFPS=False\nMaintainVerticalFOV=True\nNativeText=True\nScreenFlashes=True\n"
		"[XOpenGLDrv.XOpenGLRenderDevice]\n"
		"UseAA=False\nNumAASamples=0\nMaxAnisotropy=4\n"
		"[ALAudio.ALAudioSubsystem]\nMusicVolume=0.53\nSoundVolume=0.9\n");
	WriteBytes(roots.system / "DefUser.ini",
		"; immutable user default\n"
		"[Engine.PlayerPawn]\nbModernThirdPersonControls=False\nDifficulty=DifficultyEasy\n"
		"[HGame.Harry]\nbAutoCenterCamera=True\nbMoveWhileCasting=True\nbAutoQuaff=True\n");
}

struct Arguments
{
	std::vector<std::string> storage;
	std::vector<char*> values;

	Arguments(std::initializer_list<std::string> input) : storage(input)
	{
		for (std::string& value : storage)
			values.push_back(value.data());
	}

	bool run() { return ShouldRunNativeLauncher(static_cast<int>(values.size()), values.data()); }
};

void TestPolicyClassification()
{
	Expect(Arguments({"hp2"}).run(), "empty command line opens launcher");
	Expect(Arguments({"hp2", "-datadir=/Retail"}).run(), "-datadir alone opens launcher");
	Expect(Arguments({"hp2", "-DaTaDiR=/Retail", "-unknown"}).run(), "datadir is case-insensitive and unknown options stay interactive");
	Expect(Arguments({"hp2", "--datadir=/Retail"}).run(), "double-dash datadir remains an unrelated interactive option");
	Expect(Arguments({"hp2", "-unknown"}).run(), "unrelated option stays interactive");
	Expect(!Arguments({"hp2", "PrivetDr.unr"}).run(), "explicit map bypasses launcher");
	Expect(!Arguments({"hp2", "unreal://127.0.0.1/Map"}).run(), "explicit URL bypasses launcher");
	Expect(!Arguments({"hp2", "-datadir=/Retail", "Startup.unr"}).run(), "datadir does not hide explicit map");

	const char* names[] = {"LOAD", "NOFRONTEND", "TESTTICKS", "TESTRENDEV", "TESTNATIVETEXT", "SERVER",
		"REPLAY", "RECORD", "BENCHMARK", "COMMANDLET", "MAKE", "EXEC"};
	for (const char* name : names)
	{
		const std::string upper = std::string("-") + name + "=value";
		Expect(!Arguments({"hp2", upper}).run(), std::string(name) + " bypasses launcher");
		std::string mixed = name;
		for (std::size_t i = 0; i < mixed.size(); i += 2)
			mixed[i] = static_cast<char>(std::tolower(static_cast<unsigned char>(mixed[i])));
		Expect(!Arguments({"hp2", "--" + mixed}).run(), std::string(name) + " matching is case-insensitive with repeated dashes");
	}
	Expect(!Arguments({"hp2", "-datadir=/Retail", "-eXeC=Boot.txt"}).run(), "datadir-only filtering still detects bypass option");
}

void TestSelectedCommands()
{
	std::string command, error;
	LaunchSelection selection;
	selection.action = LaunchAction::Quit;
	Expect(BuildSelectedCommand(selection, command, error) && command.empty(), "Quit produces an empty command");
	selection.action = LaunchAction::NewGame;
	Expect(BuildSelectedCommand(selection, command, error) && command == "PrivetDr.unr", "New Game selects PrivetDr.unr");
	selection.action = LaunchAction::Continue;
	selection.hasSave = true;
	selection.save.saveIndex = 12;
	selection.save.usesSlotDirectory = false;
	selection.save.slot = -1;
	Expect(BuildSelectedCommand(selection, command, error) && command == "Startup.unr -LOAD=12", "flat save command is exact");
	selection.save.usesSlotDirectory = true;
	selection.save.slot = 3;
	Expect(BuildSelectedCommand(selection, command, error) && command == "Startup.unr -LOAD=12 -SAVESLOT=3", "slotted save command is exact");
	selection.hasSave = false;
	Expect(!BuildSelectedCommand(selection, command, error) && command.empty() && !error.empty(), "Continue rejects absent save");
	selection.hasSave = true;
	selection.save.saveIndex = -1;
	Expect(!BuildSelectedCommand(selection, command, error), "Continue rejects negative save index");
	selection.save.saveIndex = 1;
	selection.save.slot = -1;
	Expect(!BuildSelectedCommand(selection, command, error), "slotted Continue rejects negative slot");
	selection.action = LaunchAction::Error;
	Expect(!BuildSelectedCommand(selection, command, error), "error selection does not launch");
}

bool Valid(const LauncherSettings& settings)
{
	std::string error;
	return ValidateLauncherSettings(settings, error);
}

void TestValidationBoundaries()
{
	LauncherSettings settings;
	settings.resolution = {320, 320, ""};
	settings.brightness = 0.1;
	settings.mouseSensitivity = 0.2;
	settings.soundVolume = 0.0;
	settings.musicVolume = 1.0;
	settings.frameRateLimit = 0;
	Expect(Valid(settings), "all lower validation boundaries are accepted");
	settings.resolution = {16384, 16384, ""};
	settings.brightness = 1.0;
	settings.mouseSensitivity = 10.0;
	settings.soundVolume = 1.0;
	settings.musicVolume = 0.0;
	settings.frameRateLimit = 144;
	Expect(Valid(settings), "all upper validation boundaries are accepted");

	auto rejected = [&](const LauncherSettings& candidate, const char* message) { Expect(!Valid(candidate), message); };
	LauncherSettings bad = settings;
	bad.resolution.width = 319; rejected(bad, "width below boundary is rejected");
	bad = settings; bad.resolution.height = 16385; rejected(bad, "height above boundary is rejected");
	bad = settings; bad.brightness = 0.099; rejected(bad, "brightness below boundary is rejected");
	bad = settings; bad.brightness = std::numeric_limits<double>::quiet_NaN(); rejected(bad, "NaN brightness is rejected");
	bad = settings; bad.mouseSensitivity = 10.01; rejected(bad, "sensitivity above boundary is rejected");
	bad = settings; bad.mouseSensitivity = std::numeric_limits<double>::infinity(); rejected(bad, "infinite sensitivity is rejected");
	bad = settings; bad.soundVolume = -0.01; rejected(bad, "negative sound volume is rejected");
	bad = settings; bad.musicVolume = 1.01; rejected(bad, "music volume above one is rejected");
	bad = settings; bad.musicVolume = std::numeric_limits<double>::quiet_NaN(); rejected(bad, "NaN music volume is rejected");
	for (int cap : FrameRateLimitValues) { bad = settings; bad.frameRateLimit = cap; Expect(Valid(bad), "allowed frame cap is accepted"); }
	bad = settings; bad.frameRateLimit = 59; rejected(bad, "unknown frame cap is rejected");
	for (int samples : AntiAliasingSampleValues)
	{
		bad = settings;
		bad.antiAliasingSamples = samples;
		Expect(Valid(bad), "allowed MSAA sample count is accepted");
	}
	for (int anisotropy : AnisotropyValues)
	{
		bad = settings;
		bad.anisotropy = anisotropy;
		Expect(Valid(bad), "allowed anisotropy value is accepted");
	}
	bad = settings; bad.antiAliasingSamples = 1; rejected(bad, "unknown MSAA sample count is rejected");
	bad = settings; bad.antiAliasingSamples = 8; rejected(bad, "unsupported MSAA sample count is rejected");
	bad = settings; bad.anisotropy = 2; rejected(bad, "unknown anisotropy value is rejected");
	bad = settings; bad.anisotropy = 32; rejected(bad, "unsupported anisotropy value is rejected");
	bad = settings; bad.screenMode = static_cast<ScreenMode>(99); rejected(bad, "invalid screen mode is rejected");
	bad = settings; bad.textureDetail = static_cast<TextureDetail>(-1); rejected(bad, "invalid texture detail is rejected");
	bad = settings; bad.objectDetail = static_cast<ObjectDetail>(8); rejected(bad, "invalid object detail is rejected");
	bad = settings; bad.difficulty = static_cast<Difficulty>(7); rejected(bad, "invalid difficulty is rejected");
	bad = settings; bad.controlMode = static_cast<ControlMode>(5); rejected(bad, "invalid control mode is rejected");
}
void TestScaleDefaultsAndValidation()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	LauncherState loaded;
	std::string error;
	Expect(LoadLauncherState(roots.paths(), loaded, error), "scale defaults load when keys are absent");
	Expect(loaded.settings.renderScale == 1.0, "missing RenderScale defaults to 1.0");
	Expect(loaded.settings.uiScale == 1.0, "missing UIScale defaults to 1.0");
	Expect(loaded.settings.nativeText, "available native renderer is enabled by default");
	Expect(!loaded.settings.showFPS, "FPS counter is disabled by default");
	Expect(loaded.settings.controlMode == ControlMode::Classic, "Classic controls are the default");

	LauncherSettings settings;
	for (double value : RenderScaleValues)
	{
		settings.renderScale = value;
		settings.uiScale = 1.0;
		Expect(Valid(settings), "accepted render scale validates");
	}
	for (double value : UIScaleValues)
	{
		settings.renderScale = 1.0;
		settings.uiScale = value;
		Expect(Valid(settings), "accepted UI scale validates");
	}

	for (double value : {-1.0, 0.0, 0.49, 0.51, 0.66, 0.68, 0.74, 0.76, 0.84, 0.86, 1.01,
			std::numeric_limits<double>::infinity(), std::numeric_limits<double>::quiet_NaN()})
	{
		settings = LauncherSettings();
		settings.renderScale = value;
		Expect(!Valid(settings), "render scale outside the discrete choices is rejected");
	}
	for (double value : {-1.0, 0.0, 0.74, 0.76, 0.99, 1.01, 1.24, 1.26, 1.49, 1.51, 1.74, 1.76, 1.99, 2.01,
			std::numeric_limits<double>::infinity(), std::numeric_limits<double>::quiet_NaN()})
	{
		settings = LauncherSettings();
		settings.uiScale = value;
		Expect(!Valid(settings), "UI scale outside the discrete choices is rejected");
	}
}


void TestSaveDiscovery()
{
	TemporaryRoots roots;
	const fs::path save = roots.user / "Save";
	fs::create_directories(save / "Slot2");
	fs::create_directories(save / "slot1");
	fs::create_directories(save / "Slot01");
	WriteBytes(save / "Save10.USA", "ten");
	WriteBytes(save / "save2.usa", "two");
	WriteBytes(save / "save2.BMP", "bitmap");
	WriteBytes(save / "Save0.usa", "");
	fs::create_directories(save / "Save4.usa");
	fs::create_symlink(save / "save2.usa", save / "Save5.usa");
	WriteBytes(save / "Save2147483648.usa", "overflow");
	WriteBytes(save / "Save02.usa", "noncanonical flat");
	fs::create_symlink(save / "save2.BMP", save / "Save10.bmp");
	WriteBytes(save / "Slot2" / "SAVE7.UsA", "slot two");
	WriteBytes(save / "Slot2" / "SAVE7.bmp", "x");
	fs::resize_file(save / "Slot2" / "SAVE7.bmp", 16 * 1024 * 1024 + 1);
	WriteBytes(save / "slot1" / "Save9.usa", "slot one");
	WriteBytes(save / "Slot01" / "Save3.usa", "noncanonical slot");
	fs::create_symlink(save / "Slot2", save / "Slot3");

	std::vector<SaveRecord> saves;
	std::string error;
	Expect(DiscoverSaves(roots.user.string(), saves, error), "save discovery succeeds");
	Expect(saves.size() == 4, "only eligible nonempty regular saves are discovered");
	if (saves.size() == 4)
	{
		Expect(!saves[0].usesSlotDirectory && saves[0].saveIndex == 2, "flat saves sort first by index");
		Expect(!saves[1].usesSlotDirectory && saves[1].saveIndex == 10, "second flat save is ordered");
		Expect(saves[2].usesSlotDirectory && saves[2].slot == 1 && saves[2].saveIndex == 9, "slot saves sort by slot then index");
		Expect(saves[3].usesSlotDirectory && saves[3].slot == 2 && saves[3].saveIndex == 7, "later slot save is ordered");
		Expect(!saves[0].thumbnailPath.empty(), "regular case-insensitive thumbnail is found");
		Expect(saves[1].thumbnailPath.empty(), "symlink thumbnail is rejected");
		Expect(saves[3].thumbnailPath.empty(), "thumbnail over 16 MiB is rejected");
		for (const SaveRecord& record : saves)
		{
			Expect(record.size > 0, "save byte size is populated");
			Expect(record.modifiedSeconds > 0, "save date is populated");
			Expect(!record.displayName.empty(), "save display label is populated");
		}
	}
	Expect(ReadBytes(save / "save2.usa") == "two", "discovery never modifies saves");
}

bool OnlyCrLf(const std::string& text)
{
	for (std::size_t i = 0; i < text.size(); ++i)
		if (text[i] == '\n' && (i == 0 || text[i - 1] != '\r')) return false;
	return true;
}

LauncherSettings DistinctSettings()
{
	LauncherSettings settings;
	settings.screenMode = ScreenMode::BorderlessDesktop;
	settings.resolution = {1440, 900, "1440 x 900"};
	settings.verticalSync = true;
	settings.frameRateLimit = 144;
	settings.showFPS = true;
	settings.maintainVerticalFOV = false;
	settings.nativeText = false;
	settings.antiAliasingSamples = 4;
	settings.anisotropy = 16;
	settings.brightness = 0.75;
	settings.renderScale = 0.85;
	settings.uiScale = 1.75;
	settings.textureDetail = TextureDetail::Low;
	settings.objectDetail = ObjectDetail::VeryHigh;
	settings.soundEnabled = false;
	settings.soundVolume = 0.25;
	settings.musicVolume = 0.5;
	settings.mouseSensitivity = 7.25;
	settings.invertMouse = true;
	settings.controlMode = ControlMode::Modern;
	settings.autoCenterCamera = false;
	settings.moveWhileCasting = false;
	settings.autoQuaff = false;
	settings.screenFlashes = false;
	settings.difficulty = Difficulty::Hard;
	settings.joystickEnabled = false;
	return settings;
}

void TestConfigRoundTripAndBackups()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	const std::string originalGame =
		"; Preserve This Comment\r\n"
		"[engine.engine]\r\nCustomCase=KeepMe\r\n"
		"[ENGINE.GAMEENGINE]\r\nFrameRateLimit=30\r\nfrAMeRateLimit = 60 ; final value\r\nUseSound=True\r\n"
		"malformed line without equals\r\n"
		"[SDLDrv.SDLClient]\r\nBrightness=0.4\r\nUIScale=1.0\r\nUIScale = 1.25 ; final UI scale\r\n"
		"ShowFPS=False\r\nMaintainVerticalFOV=True\r\nNativeText=True\r\nScreenFlashes=True\r\n"
		"[XOpenGLDrv.XOpenGLRenderDevice]\r\nDriverNote=PreserveRendererSetting\r\n"
		"UseAA=Off\r\nNumAASamples=0\r\nMaxAnisotropy=4.000000\r\n"
		"RenderScale=0.67\r\nRenderScale = 0.75 ; final render scale\r\n"
		"[Unrelated.Section]\r\nMiXeDKey=MiXeDValue\r\n";
	const std::string originalUser =
		"# user comment\n[Engine.PlayerPawn]\nDifficulty=DifficultyEasy\nDifficulty = DifficultyMedium ; final\n"
		"[HGame.Harry]\nbAutoQuaff=True\n[Other]\nCaseKey=CaseValue\n";
	WriteBytes(roots.user / "Game.ini", originalGame);
	WriteBytes(roots.user / "User.ini", originalUser);
	::chmod((roots.user / "Game.ini").c_str(), 0640);
	::chmod((roots.user / "User.ini").c_str(), 0604);

	std::string error;
	LauncherSettings selected = DistinctSettings();
	SetLauncherPublishFailureForTesting(2);
	Expect(!CommitLauncherSettings(roots.paths(), selected, error), "second-file publication failure is reported");
	SetLauncherPublishFailureForTesting(0);
	Expect(error.find("Injected failure") != std::string::npos, "publication failure preserves its diagnostic");
	Expect(ReadBytes(roots.user / "Game.ini") == originalGame, "failed User.ini publication rolls Game.ini back exactly");
	Expect(ReadBytes(roots.user / "User.ini") == originalUser, "failed User.ini publication rolls User.ini back exactly");
	Expect(CommitLauncherSettings(roots.paths(), selected, error), "settings commit succeeds: " + error);
	const std::string game = ReadBytes(roots.user / "Game.ini");
	const std::string user = ReadBytes(roots.user / "User.ini");
	Expect(game.find("; Preserve This Comment") != std::string::npos, "game comment is preserved");
	Expect(game.find("CustomCase=KeepMe") != std::string::npos, "unrelated game key is preserved");
	Expect(game.find("MiXeDKey=MiXeDValue") != std::string::npos, "unrelated key case is preserved");
	Expect(game.find("malformed line without equals") != std::string::npos, "malformed line is preserved");
	Expect(game.find("FrameRateLimit=30") != std::string::npos, "earlier repeated owned key remains untouched");
	Expect(game.find("frAMeRateLimit = 144 ; final value") != std::string::npos, "only final repeated owned key value is replaced while case/comment remain");
	Expect(user.find("Difficulty=DifficultyEasy") != std::string::npos, "earlier repeated user key remains untouched");
	Expect(user.find("Difficulty = DifficultyHard ; final") != std::string::npos, "final user key is replaced in place");
	Expect(user.find("bModernThirdPersonControls=True") != std::string::npos,
		"Modern control mode persists under Engine.PlayerPawn");
	Expect(game.find("UIScale=1.0") != std::string::npos, "earlier repeated UI scale remains untouched");
	Expect(game.find("UIScale = 1.75 ; final UI scale") != std::string::npos, "final UI scale is replaced in place");
	Expect(game.find("RenderScale=0.67") != std::string::npos, "earlier repeated render scale remains untouched");
	Expect(game.find("RenderScale = 0.85 ; final render scale") != std::string::npos, "final render scale is replaced in place");
	Expect(game.find("DriverNote=PreserveRendererSetting") != std::string::npos, "unrelated renderer setting is preserved");
	Expect(game.find("\r\nShowFPS=True\r\n") != std::string::npos &&
		game.find("MaintainVerticalFOV=False") != std::string::npos &&
		game.find("NativeText=False") != std::string::npos &&
		game.find("ScreenFlashes=False") != std::string::npos,
		"FPS, widescreen, text rendering, and flash preferences persist under SDLDrv.SDLClient");
	Expect(game.find("UseAA=True") != std::string::npos &&
		game.find("NumAASamples=4") != std::string::npos &&
		game.find("MaxAnisotropy=16") != std::string::npos,
		"MSAA and anisotropy persist under XOpenGLDrv");
	Expect(user.find("ShowFPS") == std::string::npos &&
		user.find("MaintainVerticalFOV") == std::string::npos &&
		user.find("ScreenFlashes") == std::string::npos &&
		user.find("NumAASamples") == std::string::npos,
		"modern video settings do not leak into User.ini");
	Expect(OnlyCrLf(game), "Game.ini CRLF newline style is preserved");
	Expect(user.find('\r') == std::string::npos, "User.ini LF newline style is preserved");
	Expect(ReadBytes(roots.user / "Game.ini.bak") == originalGame, "first Game.ini backup is exact");
	Expect(ReadBytes(roots.user / "User.ini.bak") == originalUser, "first User.ini backup is exact");
	struct stat gameMode, userMode;
	::stat((roots.user / "Game.ini").c_str(), &gameMode);
	::stat((roots.user / "User.ini").c_str(), &userMode);
	Expect((gameMode.st_mode & 0777) == 0640, "Game.ini destination mode is retained");
	Expect((userMode.st_mode & 0777) == 0604, "User.ini destination mode is retained");

	LauncherState loaded;
	Expect(LoadLauncherState(roots.paths(), loaded, error), "committed settings reload");
	Expect(loaded.settings.screenMode == selected.screenMode, "screen mode round-trips");
	Expect(loaded.settings.resolution.width == 1440 && loaded.settings.resolution.height == 900, "resolution round-trips");
	Expect(loaded.settings.verticalSync && loaded.settings.frameRateLimit == 144, "VSync and frame cap round-trip");
	Expect(loaded.settings.showFPS, "FPS counter preference round-trips");
	Expect(!loaded.settings.maintainVerticalFOV && !loaded.settings.nativeText &&
		loaded.settings.antiAliasingSamples == 4 &&
		loaded.settings.anisotropy == 16,
		"widescreen, text rendering, MSAA, and anisotropy round-trip");
	Expect(loaded.settings.renderScale == 0.85 && loaded.settings.uiScale == 1.75, "render and UI scales round-trip");
	Expect(loaded.settings.textureDetail == TextureDetail::Low && loaded.settings.objectDetail == ObjectDetail::VeryHigh, "detail levels round-trip");
	Expect(!loaded.settings.soundEnabled && loaded.settings.soundVolume == 0.25 && loaded.settings.musicVolume == 0.5, "audio settings round-trip");
	Expect(loaded.settings.mouseSensitivity == 7.25 && loaded.settings.invertMouse, "mouse settings round-trip");
	Expect(loaded.settings.controlMode == ControlMode::Modern, "Modern control mode round-trips");
	Expect(loaded.settings.difficulty == Difficulty::Hard, "difficulty round-trips");
	Expect(!loaded.settings.autoCenterCamera && !loaded.settings.moveWhileCasting && !loaded.settings.autoQuaff, "gameplay toggles round-trip");
	Expect(!loaded.settings.screenFlashes, "screen flash preference round-trips");

	selected.frameRateLimit = 30;
	selected.controlMode = ControlMode::Classic;
	Expect(CommitLauncherSettings(roots.paths(), selected, error), "second settings commit succeeds");
	Expect(ReadBytes(roots.user / "User.ini").find("bModernThirdPersonControls=False") != std::string::npos,
		"Classic control mode rewrites the engine preference to False");
	Expect(ReadBytes(roots.user / "Game.ini.bak") == originalGame, "Game.ini backup is never overwritten");
	Expect(ReadBytes(roots.user / "User.ini.bak") == originalUser, "User.ini backup is never overwritten");
}
void TestEveryScaleRoundTrip()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	std::string error;
	for (double value : RenderScaleValues)
	{
		LauncherSettings settings;
		settings.renderScale = value;
		Expect(CommitLauncherSettings(roots.paths(), settings, error), "accepted render scale commits");
		LauncherState loaded;
		Expect(LoadLauncherState(roots.paths(), loaded, error), "accepted render scale reloads");
		Expect(loaded.settings.renderScale == value, "accepted render scale round-trips exactly");
	}
	for (double value : UIScaleValues)
	{
		LauncherSettings settings;
		settings.uiScale = value;
		Expect(CommitLauncherSettings(roots.paths(), settings, error), "accepted UI scale commits");
		LauncherState loaded;
		Expect(LoadLauncherState(roots.paths(), loaded, error), "accepted UI scale reloads");
		Expect(loaded.settings.uiScale == value, "accepted UI scale round-trips exactly");
	}
}
void TestModernOptionRoundTrips()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	std::string error;

	for (int samples : AntiAliasingSampleValues)
	{
		LauncherSettings settings;
		settings.antiAliasingSamples = samples;
		Expect(CommitLauncherSettings(roots.paths(), settings, error), "accepted MSAA setting commits");
		LauncherState loaded;
		Expect(LoadLauncherState(roots.paths(), loaded, error), "accepted MSAA setting reloads");
		Expect(loaded.settings.antiAliasingSamples == samples, "accepted MSAA setting round-trips exactly");
	}
	for (int anisotropy : AnisotropyValues)
	{
		LauncherSettings settings;
		settings.anisotropy = anisotropy;
		Expect(CommitLauncherSettings(roots.paths(), settings, error), "accepted anisotropy setting commits");
		LauncherState loaded;
		Expect(LoadLauncherState(roots.paths(), loaded, error), "accepted anisotropy setting reloads");
		Expect(loaded.settings.anisotropy == anisotropy, "accepted anisotropy setting round-trips exactly");
	}
	for (bool enabled : {false, true})
	{
		LauncherSettings settings;
		settings.showFPS = enabled;
		settings.maintainVerticalFOV = enabled;
		settings.screenFlashes = enabled;
		settings.nativeText = enabled;
		Expect(CommitLauncherSettings(roots.paths(), settings, error), "modern boolean settings commit");
		LauncherState loaded;
		Expect(LoadLauncherState(roots.paths(), loaded, error), "modern boolean settings reload");
		Expect(loaded.settings.maintainVerticalFOV == enabled, "widescreen preference round-trips exactly");
		Expect(loaded.settings.showFPS == enabled, "FPS counter preference round-trips exactly");
		Expect(loaded.settings.screenFlashes == enabled, "screen flash preference round-trips exactly");
		Expect(loaded.settings.nativeText == enabled, "native text preference round-trips exactly");
	}

	WriteBytes(roots.user / "Game.ini",
		"[SDLDrv.SDLClient]\nShowFPS=True\nMaintainVerticalFOV=True\nNativeText=True\nScreenFlashes=True\n"
		"[XOpenGLDrv.XOpenGLRenderDevice]\n"
		"UseAA=False\nNumAASamples=4\nMaxAnisotropy=8.000000\n");
	LauncherState loaded;
	Expect(LoadLauncherState(roots.paths(), loaded, error), "runtime-formatted modern options load");
	Expect(loaded.settings.antiAliasingSamples == 0, "disabled UseAA takes precedence over a stale sample count");
	Expect(loaded.settings.anisotropy == 8, "floating-point renderer anisotropy text loads as a discrete value");
	Expect(loaded.settings.nativeText, "enabled native text preference loads from Game.ini");
	Expect(loaded.settings.showFPS, "enabled FPS counter preference loads from Game.ini");

	WriteBytes(roots.user / "Game.ini",
		"[SDLDrv.SDLClient]\nMaintainVerticalFOV=True\nScreenFlashes=True\n");
	Expect(LoadLauncherState(roots.paths(), loaded, error), "modern options load without a NativeText key");
	Expect(loaded.settings.nativeText, "missing NativeText uses the available renderer default");
	Expect(!loaded.settings.showFPS, "missing ShowFPS uses the disabled model default");
}



void TestReadOnlyCancellationAndMaterialization()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	const std::string defaultBefore = ReadBytes(roots.system / "Default.ini");
	const std::string userDefaultBefore = ReadBytes(roots.system / "DefUser.ini");
	LauncherState state;
	std::string error;
	Expect(LoadLauncherState(roots.paths(), state, error), "load from immutable defaults succeeds");
	LaunchSelection quit;
	quit.action = LaunchAction::Quit;
	std::string command;
	Expect(BuildSelectedCommand(quit, command, error), "cancel selection is accepted");
	Expect(!fs::exists(roots.user / "Game.ini") && !fs::exists(roots.user / "User.ini"), "load/cancel path creates no writable INIs");
	Expect(ReadBytes(roots.system / "Default.ini") == defaultBefore && ReadBytes(roots.system / "DefUser.ini") == userDefaultBefore,
		"load/cancel path never changes immutable defaults");

	SetLauncherPublishFailureForTesting(2);
	Expect(!CommitLauncherSettings(roots.paths(), state.settings, error), "failed first-write publication is reported");
	SetLauncherPublishFailureForTesting(0);
	Expect(!fs::exists(roots.user / "Game.ini") && !fs::exists(roots.user / "User.ini"),
		"failed first-write publication restores both files to absence");

	Expect(CommitLauncherSettings(roots.paths(), state.settings, error), "first commit materializes missing writable INIs");
	Expect(fs::exists(roots.user / "Game.ini") && fs::exists(roots.user / "User.ini"), "commit seeds both writable INIs");
	Expect(!fs::exists(roots.user / "Game.ini.bak") && !fs::exists(roots.user / "User.ini.bak"), "materializing absent INIs does not invent backups");
	struct stat gameMode;
	::stat((roots.user / "Game.ini").c_str(), &gameMode);
	Expect((gameMode.st_mode & 0777) == 0600, "new writable INI is mode 0600");
}

void TestMalformedRecovery()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	const std::string malformedGame =
		"; remains intact\n[Engine.GameEngine\nFrameRateLimit=120\n"
		"not=a truncation marker\n[Engine.GameEngine]\nFrameRateLimit=invalid\n"
		"[SDLDrv.SDLClient]\nBrightness=nan\nUIScale=1.1\nWindowedViewportX=12\nWindowedViewportY=999999\n"
		"ShowFPS=perhaps\nMaintainVerticalFOV=maybe\nNativeText=not-a-boolean\nScreenFlashes=sometimes\n"
		"[XOpenGLDrv.XOpenGLRenderDevice]\nRenderScale=0.8\nUseAA=True\nNumAASamples=8\nMaxAnisotropy=2\n";
	const std::string malformedUser =
		"[Engine.PlayerPawn\nDifficulty=DifficultyHard\n"
		"[Engine.PlayerPawn]\nDifficulty=Unknown\nMouseSensitivity=not-a-number\nbModernThirdPersonControls=maybe\n"
		"[HGame.Harry]\nbAutoQuaff=maybe\n";
	WriteBytes(roots.user / "Game.ini", malformedGame);
	WriteBytes(roots.user / "User.ini", malformedUser);
	LauncherState state;
	std::string error;
	Expect(LoadLauncherState(roots.paths(), state, error), "malformed INIs recover without truncation failure");
	const LauncherSettings defaults;
	Expect(state.settings.frameRateLimit == defaults.frameRateLimit, "invalid final frame cap falls back to model default");
	Expect(state.settings.brightness == defaults.brightness, "invalid brightness falls back to model default");
	Expect(state.settings.resolution.width == defaults.resolution.width && state.settings.resolution.height == defaults.resolution.height,
		"invalid resolution falls back to model defaults");
	Expect(state.settings.renderScale == defaults.renderScale && state.settings.uiScale == defaults.uiScale,
		"invalid discrete scale values fall back to model defaults");
	Expect(!state.settings.showFPS &&
		state.settings.maintainVerticalFOV == defaults.maintainVerticalFOV &&
		state.settings.nativeText == defaults.nativeText &&
		state.settings.screenFlashes == defaults.screenFlashes,
		"invalid modern booleans fall back to model defaults");
	Expect(state.settings.antiAliasingSamples == defaults.antiAliasingSamples &&
		state.settings.anisotropy == defaults.anisotropy,
		"invalid MSAA and anisotropy values fall back to model defaults");
	Expect(state.settings.difficulty == defaults.difficulty && state.settings.mouseSensitivity == defaults.mouseSensitivity,
		"invalid user values fall back to model defaults");
	Expect(state.settings.controlMode == ControlMode::Classic, "malformed control mode falls back to Classic");
	Expect(state.settings.autoQuaff == defaults.autoQuaff, "invalid boolean falls back to model default");
	Expect(CommitLauncherSettings(roots.paths(), state.settings, error), "malformed but decodable INIs remain writable");
	Expect(ReadBytes(roots.user / "Game.ini").find("[Engine.GameEngine") != std::string::npos, "malformed section header is preserved on commit");
	Expect(ReadBytes(roots.user / "Game.ini.bak") == malformedGame, "malformed original is backed up exactly");
}

std::string Utf16Ascii(const std::string& text, bool little)
{
	std::string bytes;
	bytes.push_back(static_cast<char>(little ? 0xff : 0xfe));
	bytes.push_back(static_cast<char>(little ? 0xfe : 0xff));
	for (unsigned char c : text)
	{
		bytes.push_back(static_cast<char>(little ? c : 0));
		bytes.push_back(static_cast<char>(little ? 0 : c));
	}
	return bytes;
}

std::string DecodeUtf16Ascii(const std::string& bytes, bool little)
{
	std::string text;
	for (std::size_t i = 2; i + 1 < bytes.size(); i += 2)
		text.push_back(little ? bytes[i] : bytes[i + 1]);
	return text;
}

void TestUtf16AndInvalidEncodingRecovery()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	const std::string gameText = "; UTF16 game comment\r\n[Engine.GameEngine]\r\nFrameRateLimit=60.000000\r\n";
	const std::string userText = "; UTF16 user comment\n[Engine.PlayerPawn]\nDifficulty=DifficultyEasy\n";
	WriteBytes(roots.user / "Game.ini", Utf16Ascii(gameText, true));
	WriteBytes(roots.user / "User.ini", Utf16Ascii(userText, false));
	std::string error;
	Expect(CommitLauncherSettings(roots.paths(), DistinctSettings(), error), "UTF-16 LE/BE settings commit succeeds");
	const std::string game = ReadBytes(roots.user / "Game.ini");
	const std::string user = ReadBytes(roots.user / "User.ini");
	Expect(game.size() >= 2 && static_cast<unsigned char>(game[0]) == 0xff && static_cast<unsigned char>(game[1]) == 0xfe,
		"UTF-16 LE encoding is preserved");
	Expect(user.size() >= 2 && static_cast<unsigned char>(user[0]) == 0xfe && static_cast<unsigned char>(user[1]) == 0xff,
		"UTF-16 BE encoding is preserved");
	Expect(DecodeUtf16Ascii(game, true).find("; UTF16 game comment") != std::string::npos, "UTF-16 comment survives round-trip");
	Expect(DecodeUtf16Ascii(user, false).find("Difficulty=DifficultyHard") != std::string::npos, "UTF-16 owned user value is updated");

	TemporaryRoots invalid;
	WriteDefaultTemplates(invalid);
	const std::string invalidUtf8 = std::string("[Engine.GameEngine]\nFrameRateLimit=") + static_cast<char>(0xff) + "\n";
	WriteBytes(invalid.user / "Game.ini", invalidUtf8);
	const std::string invalidUtf16("\xff\xfe\x00", 3);
	WriteBytes(invalid.user / "User.ini", invalidUtf16);
	LauncherState recovered;
	Expect(LoadLauncherState(invalid.paths(), recovered, error), "invalid writable encoding falls back to immutable defaults in memory");
	Expect(ReadBytes(invalid.user / "Game.ini") == invalidUtf8, "encoding recovery during load is read-only");
	Expect(CommitLauncherSettings(invalid.paths(), recovered.settings, error), "commit materializes recovered default documents");
	Expect(ReadBytes(invalid.user / "Game.ini.bak") == invalidUtf8, "invalid UTF-8 original is backed up exactly");
	Expect(ReadBytes(invalid.user / "User.ini.bak") == invalidUtf16, "invalid UTF-16 original is backed up exactly");
	Expect(ReadBytes(invalid.user / "Game.ini").find("FrameRateLimit=60") != std::string::npos, "recovered Game.ini is valid baseline plus owned settings");
}

void TestUnsafeBackupRejected()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	WriteBytes(roots.user / "Game.ini", "[Engine.GameEngine]\nFrameRateLimit=60\n");
	WriteBytes(roots.user / "User.ini", "[Engine.PlayerPawn]\nDifficulty=DifficultyEasy\n");
	WriteBytes(roots.user / "elsewhere", "do not replace");
	fs::create_symlink(roots.user / "elsewhere", roots.user / "Game.ini.bak");
	std::string error;
	Expect(!CommitLauncherSettings(roots.paths(), LauncherSettings(), error), "symlink backup path is rejected");
	Expect(ReadBytes(roots.user / "elsewhere") == "do not replace", "unsafe backup target is untouched");
}

void TestConventionalDataRoots()
{
	TemporaryRoots roots;
	const char* priorHomeValue = std::getenv("HOME");
	const bool hadHome = priorHomeValue != nullptr;
	const std::string priorHome = hadHome ? priorHomeValue : "";
	::setenv("HOME", roots.root.c_str(), 1);

	const fs::path dataRoot = fs::canonical(roots.root) / "Library" / "Application Support" /
		"Harry Potter 2" / "Data";
	const fs::path expectedRetail = dataRoot / "Retail";
	const fs::path expectedPrototype = dataRoot / "Prototype";
	Expect(!fs::exists(expectedRetail) && !fs::exists(expectedPrototype),
		"conventional data folders are not created or required before first launch");

	std::string retail;
	std::string prototype;
	Expect(GetHP2ConventionalRetailDataRoot(retail),
		"Retail conventional root resolves without existing data");
	Expect(GetHP2ConventionalPrototypeDataRoot(prototype),
		"Prototype conventional root resolves without existing data");
	Expect(fs::path(retail).is_absolute() && fs::path(prototype).is_absolute() &&
		fs::path(retail) == expectedRetail && fs::path(prototype) == expectedPrototype &&
		retail != prototype,
		"conventional roots are distinct canonical external Data/Retail and Data/Prototype paths");

	const std::string checkout = fs::canonical(fs::current_path()).string();
	const std::string checkoutPrefix = checkout + "/";
	Expect(retail != checkout && prototype != checkout &&
		retail.compare(0, checkoutPrefix.size(), checkoutPrefix) != 0 &&
		prototype.compare(0, checkoutPrefix.size(), checkoutPrefix) != 0,
		"conventional roots never point into the repository checkout");

	if (hadHome)
		::setenv("HOME", priorHome.c_str(), 1);
	else
		::unsetenv("HOME");
}

fs::path CreateDataRoot(const TemporaryRoots& roots, const char* name, const char* sentinel)
{
	const fs::path dataRoot = roots.root / name;
	fs::create_directories(dataRoot / "System");
	fs::create_directories(dataRoot / "Maps");
	fs::create_directories(dataRoot / "Textures");
	WriteBytes(dataRoot / "System" / "Default.ini", std::string("[Sentinel]\nName=") + sentinel + "\n");
	WriteBytes(dataRoot / "Maps" / "PrivetDr.unr", sentinel);
	WriteBytes(dataRoot / "Maps" / "Startup.unr", sentinel);
	WriteBytes(dataRoot / "Textures" / "Shared.utx", sentinel);
	return dataRoot;
}

void TestDataSourceCatalogPersistence()
{
	TemporaryRoots roots;
	const fs::path retail = CreateDataRoot(roots, "RetailData", "retail");
	const fs::path prototype = CreateDataRoot(roots, "PrototypeData", "prototype");
	const fs::path unavailable = roots.root / "UnavailableData";
	std::string error;
	std::string canonicalRetail;
	std::string canonicalPrototype;
	Expect(ValidateHP2DataRoot(retail.string(), canonicalRetail, error), "Retail data root validates");
	Expect(ValidateHP2DataRoot(prototype.string(), canonicalPrototype, error), "Prototype data root validates");
	Expect(canonicalRetail == fs::canonical(retail).string(), "Retail root is canonicalized");
	Expect(canonicalPrototype == fs::canonical(prototype).string(), "Prototype root is canonicalized");
	Expect(ReadBytes(fs::path(canonicalRetail) / "Maps" / "PrivetDr.unr") == "retail",
		"Retail root resolves only its own map sentinel");
	Expect(ReadBytes(fs::path(canonicalPrototype) / "Maps" / "Startup.unr") == "prototype",
		"Prototype root resolves only its own map sentinel");
	Expect(!ValidateHP2DataRoot(unavailable.string(), canonicalRetail, error),
		"unavailable saved root is rejected only at bootstrap validation");

	DataSourceConfiguration configuration;
	Expect(LoadDataSourceConfiguration(roots.user.string(), configuration, error),
		"missing launcher catalog defaults successfully");
	Expect(configuration.selected == DataSource::Retail && configuration.retailRoot.empty() &&
		configuration.prototypeRoot.empty(), "missing launcher catalog defaults to an empty Retail selection");

	configuration.selected = DataSource::Prototype;
	configuration.retailRoot = canonicalRetail;
	configuration.prototypeRoot = canonicalPrototype;
	Expect(CommitDataSourceConfiguration(roots.user.string(), configuration, error),
		"absolute named roots commit to the launcher catalog: " + error);
	const fs::path catalog = roots.user / "Launcher.ini";
	const std::string firstCatalog = ReadBytes(catalog);
	Expect(firstCatalog.find("[DataSources]") != std::string::npos &&
		firstCatalog.find("Selected=Prototype") != std::string::npos &&
		firstCatalog.find("RetailRoot=" + canonicalRetail) != std::string::npos &&
		firstCatalog.find("PrototypeRoot=" + canonicalPrototype) != std::string::npos,
		"launcher catalog serializes its selected source and both canonical roots");

	DataSourceConfiguration loaded;
	Expect(LoadDataSourceConfiguration(roots.user.string(), loaded, error), "saved launcher catalog reloads");
	Expect(loaded.selected == DataSource::Prototype && loaded.retailRoot == canonicalRetail &&
		loaded.prototypeRoot == canonicalPrototype, "both named roots round-trip independently");

	DataSourceConfiguration unavailableCatalog = loaded;
	unavailableCatalog.selected = DataSource::Retail;
	unavailableCatalog.retailRoot = unavailable.string();
	Expect(CommitDataSourceConfiguration(roots.user.string(), unavailableCatalog, error),
		"an unavailable but absolute saved root remains editable catalog state");
	Expect(LoadDataSourceConfiguration(roots.user.string(), loaded, error), "unavailable catalog reloads");
	Expect(loaded.selected == DataSource::Retail && loaded.retailRoot == unavailable.string(),
		"unavailable root is retained instead of silently selecting another source");
	Expect(!ValidateHP2DataRoot(loaded.retailRoot, canonicalRetail, error),
		"saved unavailable root still fails launch-time validation");

	const std::string catalogBeforeRejectedCommit = ReadBytes(catalog);
	DataSourceConfiguration invalid = loaded;
	invalid.selected = static_cast<DataSource>(99);
	Expect(!CommitDataSourceConfiguration(roots.user.string(), invalid, error), "unknown data source enum is rejected");
	invalid = loaded;
	invalid.prototypeRoot = "relative-data-root";
	Expect(!CommitDataSourceConfiguration(roots.user.string(), invalid, error), "relative named root is rejected");
	Expect(ReadBytes(catalog) == catalogBeforeRejectedCommit, "rejected catalog inputs never alter persisted state");

	SetLauncherPublishFailureForTesting(1);
	loaded.prototypeRoot = canonicalPrototype;
	Expect(!CommitDataSourceConfiguration(roots.user.string(), loaded, error), "catalog publication failure is reported");
	SetLauncherPublishFailureForTesting(0);
	Expect(ReadBytes(catalog) == catalogBeforeRejectedCommit, "failed catalog publication restores prior bytes");
}

void TestMalformedDataSourceCatalogRecovery()
{
	TemporaryRoots roots;
	const fs::path prototype = CreateDataRoot(roots, "PrototypeData", "prototype");
	const std::string canonicalPrototype = fs::canonical(prototype).string();
	WriteBytes(roots.user / "Launcher.ini",
		"[DataSources]\nSelected=UnknownEdition\nRetailRoot=relative-retail\nPrototypeRoot=" +
		canonicalPrototype + "\n");

	DataSourceConfiguration loaded;
	std::string error;
	Expect(LoadDataSourceConfiguration(roots.user.string(), loaded, error), "malformed catalog recovers without failing launch setup");
	Expect(loaded.selected == DataSource::Retail, "unknown persisted source defaults to Retail");
	Expect(loaded.retailRoot.empty(), "malformed relative Retail root is discarded");
	Expect(loaded.prototypeRoot == canonicalPrototype, "valid independent Prototype root survives recovery");
}

void TestDataSourceProfilesAndMigration()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	WriteBytes(roots.user / "Game.ini", "[Engine.GameEngine]\nFrameRateLimit=30\n");
	WriteBytes(roots.user / "User.ini", "[Engine.PlayerPawn]\nDifficulty=DifficultyHard\n");
	fs::create_directories(roots.user / "Save" / "cache");
	fs::create_directories(roots.user / "Cache");
	WriteBytes(roots.user / "Save" / "Save1.usa", "retail save");
	WriteBytes(roots.user / "Save" / "cache" / "Thumb1.bmp", "cache");
	WriteBytes(roots.user / "Cache" / "Engine.cache", "engine cache");

	std::string error;
	std::string retailProfile;
	Expect(PrepareDataSourceProfile(roots.user.string(), DataSource::Retail, retailProfile, error),
		"first Retail profile prepares and migrates legacy state: " + error);
	const fs::path expectedRetail = roots.user / "Profiles" / "Retail";
	Expect(fs::path(retailProfile) == expectedRetail, "Retail profile is source-specific");
	Expect(ReadBytes(expectedRetail / "Game.ini").find("FrameRateLimit=30") != std::string::npos &&
		ReadBytes(expectedRetail / "Save" / "Save1.usa") == "retail save",
		"legacy settings and saves migrate into the selected profile");
	Expect(fs::exists(roots.user / "Game.ini") && fs::exists(roots.user / "Save" / "Save1.usa"),
		"legacy mutable state remains intact after durable profile migration");
	Expect(ReadBytes(roots.user / "Launcher.ini").find("LegacyProfile=Retail") != std::string::npos,
		"successful migration records its selected source in launcher-owned catalog");

	LauncherSettings retailSettings;
	retailSettings.frameRateLimit = 30;
	Expect(CommitLauncherSettings({retailProfile, roots.system.string()}, retailSettings, error),
		"Retail profile settings commit");
	fs::create_directories(expectedRetail / "Save");
	WriteBytes(expectedRetail / "Save" / "Save2.usa", "retail profile save");

	std::string prototypeProfile;
	Expect(PrepareDataSourceProfile(roots.user.string(), DataSource::Prototype, prototypeProfile, error),
		"Prototype profile prepares after Retail migration");
	const fs::path expectedPrototype = roots.user / "Profiles" / "Prototype";
	Expect(fs::path(prototypeProfile) == expectedPrototype, "Prototype profile is source-specific");
	Expect(!fs::exists(expectedPrototype / "Game.ini") && !fs::exists(expectedPrototype / "Save" / "Save1.usa"),
		"legacy state does not leak into the second profile");
	LauncherSettings prototypeSettings;
	prototypeSettings.frameRateLimit = 120;
	Expect(CommitLauncherSettings({prototypeProfile, roots.system.string()}, prototypeSettings, error),
		"Prototype profile settings commit");
	fs::create_directories(expectedPrototype / "Save");
	WriteBytes(expectedPrototype / "Save" / "Save3.usa", "prototype profile save");

	LauncherState retailState;
	LauncherState prototypeState;
	Expect(LoadLauncherState({retailProfile, roots.system.string()}, retailState, error), "Retail profile settings reload");
	Expect(LoadLauncherState({prototypeProfile, roots.system.string()}, prototypeState, error), "Prototype profile settings reload");
	Expect(retailState.settings.frameRateLimit == 30 && prototypeState.settings.frameRateLimit == 120,
		"profile settings remain isolated");
	std::vector<SaveRecord> retailSaves;
	std::vector<SaveRecord> prototypeSaves;
	Expect(DiscoverSaves(retailProfile, retailSaves, error) && DiscoverSaves(prototypeProfile, prototypeSaves, error),
		"profile save discovery succeeds");
	Expect(retailSaves.size() == 2 && retailSaves[0].saveIndex == 1 && retailSaves[1].saveIndex == 2 &&
		prototypeSaves.size() == 1 && prototypeSaves[0].saveIndex == 3,
		"profile save discovery does not cross sources");
}

void TestDataSourceMigrationFailureAndQuit()
{
	TemporaryRoots roots;
	WriteDefaultTemplates(roots);
	LaunchSelection quit;
	quit.action = LaunchAction::Quit;
	std::string command;
	std::string error;
	Expect(BuildSelectedCommand(quit, command, error), "Quit-like selection remains valid");
	Expect(!fs::exists(roots.user / "Launcher.ini") && !fs::exists(roots.user / "Profiles"),
		"Quit-like selection leaves catalog and profiles untouched");

	WriteBytes(roots.user / "Game.ini", "[Engine.GameEngine]\nFrameRateLimit=30\n");
	WriteBytes(roots.user / "outside-save", "must not follow");
	fs::create_directories(roots.user / "Save");
	fs::create_symlink(roots.user / "outside-save", roots.user / "Save" / "Save1.usa");
	std::string profile;
	Expect(!PrepareDataSourceProfile(roots.user.string(), DataSource::Retail, profile, error),
		"irregular legacy state rejects profile migration");
	Expect(!fs::exists(roots.user / "Profiles" / "Retail") && !fs::exists(roots.user / "Launcher.ini"),
		"failed migration publishes neither a profile nor a catalog marker");
	Expect(ReadBytes(roots.user / "Game.ini").find("FrameRateLimit=30") != std::string::npos &&
		fs::is_symlink(roots.user / "Save" / "Save1.usa"),
		"failed migration leaves legacy source state untouched");
}

std::string TcharAsciiPath(const TCHAR* path)
{
	std::string result;
	for (; path && *path; ++path)
		result.push_back(static_cast<char>(*path));
	return result;
}

void TestInstalledDataRootIsolation()
{
	TemporaryRoots roots;
	const fs::path retail = CreateDataRoot(roots, "RetailData", "retail");
	const fs::path prototype = CreateDataRoot(roots, "PrototypeData", "prototype");
	const fs::path retailProfile = roots.user / "Profiles" / "Retail";
	const fs::path prototypeProfile = roots.user / "Profiles" / "Prototype";
	std::string error;

	Expect(InstallHP2Paths(retail.string(), retailProfile.string(), error),
		"Retail root installs with its own profile: " + error);
	Expect(TcharAsciiPath(appBaseDir()) == fs::canonical(retail / "System").string() + "/" &&
		TcharAsciiPath(appUserDir()) == fs::canonical(retailProfile).string() + "/",
		"Retail installation exposes only Retail System and profile roots to the engine");

	Expect(InstallHP2Paths(prototype.string(), prototypeProfile.string(), error),
		"Prototype root installs with its own profile: " + error);
	Expect(TcharAsciiPath(appBaseDir()) == fs::canonical(prototype / "System").string() + "/" &&
		TcharAsciiPath(appUserDir()) == fs::canonical(prototypeProfile).string() + "/",
		"source switch replaces both active roots without an overlay");
	const std::string baseBeforeRejectedInstall = TcharAsciiPath(appBaseDir());
	const std::string userBeforeRejectedInstall = TcharAsciiPath(appUserDir());
	Expect(!InstallHP2Paths(retail.string(), (retail / "User").string(), error),
		"overlapping data and user roots are rejected");
	Expect(TcharAsciiPath(appBaseDir()) == baseBeforeRejectedInstall &&
		TcharAsciiPath(appUserDir()) == userBeforeRejectedInstall,
		"failed root installation leaves the active source unchanged");
}

void TestDataDirectoryArgumentContract()
{
	Expect(IsHP2DataDirectoryArgument("-DaTaDiR=/Retail"), "single-dash data root option is case-insensitive");
	Expect(std::string(HP2DataDirectoryArgumentValue("-datadir=/Retail")) == "/Retail",
		"recognized data root option exposes its unmodified value");
	Expect(!IsHP2DataDirectoryArgument("--datadir=/Retail") &&
		HP2DataDirectoryArgumentValue("--datadir=/Retail") == nullptr,
		"double-dash data root option remains unrecognized");
	Expect(!IsHP2DataDirectoryArgument("-datadir") && IsHP2DataDirectoryArgument("-datadir=") &&
		HP2DataDirectoryArgumentValue("-datadir=") != nullptr &&
		std::string(HP2DataDirectoryArgumentValue("-datadir=")).empty(),
		"empty single-dash data root remains a recognized invalid explicit override");

	TemporaryRoots roots;
	const fs::path retail = CreateDataRoot(roots, "RetailData", "retail");
	const fs::path prototype = CreateDataRoot(roots, "PrototypeData", "prototype");
	const char* priorHomeValue = std::getenv("HOME");
	const bool hadHome = priorHomeValue != nullptr;
	const std::string priorHome = hadHome ? priorHomeValue : "";
	::setenv("HOME", roots.root.c_str(), 1);
	Arguments arguments({"hp2", "--datadir=" + prototype.string(), "-DaTaDiR=" + retail.string(),
		"-datadir=" + prototype.string()});
	std::string retailArgumentBefore = arguments.storage[2];
	std::string prototypeArgumentBefore = arguments.storage[3];
	Expect(PrepareHP2Paths(static_cast<int>(arguments.values.size()), arguments.values.data()),
		"first recognized explicit data root prepares successfully");
	Expect(TcharAsciiPath(appBaseDir()) == fs::canonical(retail / "System").string() + "/",
		"first recognized single-dash data root wins over later or double-dash arguments");
	Expect(arguments.storage[2] == retailArgumentBefore && arguments.storage[3] == prototypeArgumentBefore,
		"path bootstrap preserves command-line data root arguments for launcher filtering");
	if (hadHome)
		::setenv("HOME", priorHome.c_str(), 1);
	else
		::unsetenv("HOME");
}

LegacySettingValue V(const char* section, const char* key, const char* value)
{
	return {section, key, value};
}

SettingsMigrationChange C(const char* section, const char* key,
	const char* previous, const char* next, const char* reason)
{
	return {section, key, previous, next, reason};
}

void ExpectRows(const std::vector<LegacySettingValue>& actual,
	const std::vector<LegacySettingValue>& expected, const std::string& context)
{
	Expect(actual.size() == expected.size(),
		context + ": row count is " + std::to_string(actual.size()) +
		", expected " + std::to_string(expected.size()));
	const std::size_t shared = std::min(actual.size(), expected.size());
	for (std::size_t i = 0; i < shared; ++i)
	{
		Expect(actual[i].section == expected[i].section && actual[i].key == expected[i].key &&
			actual[i].value == expected[i].value,
			context + ": row " + std::to_string(i) + " is [" + actual[i].section + "]" +
			actual[i].key + "=" + actual[i].value + ", expected [" + expected[i].section + "]" +
			expected[i].key + "=" + expected[i].value);
	}
}

void ExpectJournal(const std::vector<SettingsMigrationChange>& actual,
	const std::vector<SettingsMigrationChange>& expected, const std::string& context)
{
	Expect(actual.size() == expected.size(),
		context + ": journal length is " + std::to_string(actual.size()) +
		", expected " + std::to_string(expected.size()));
	const std::size_t shared = std::min(actual.size(), expected.size());
	for (std::size_t i = 0; i < shared; ++i)
	{
		Expect(actual[i].section == expected[i].section && actual[i].key == expected[i].key &&
			actual[i].previousValue == expected[i].previousValue &&
			actual[i].newValue == expected[i].newValue && actual[i].reason == expected[i].reason,
			context + ": journal entry " + std::to_string(i) + " is [" + actual[i].section + "]" +
			actual[i].key + " " + actual[i].previousValue + "->" + actual[i].newValue + " (" +
			actual[i].reason + "), expected [" + expected[i].section + "]" + expected[i].key + " " +
			expected[i].previousValue + "->" + expected[i].newValue + " (" + expected[i].reason + ")");
	}
}

void TestLegacySettingsMigration()
{
	struct MigrationCase
	{
		const char* description;
		std::vector<LegacySettingValue> input;
		std::vector<LegacySettingValue> expected;
		std::vector<SettingsMigrationChange> journal;
	};
	std::vector<MigrationCase> cases;

	// Renderer-era legacy configuration normalizes completely into the modern
	// schema, with every rewrite observable in the journal.
	{
		MigrationCase full;
		full.description = "renderer-era legacy configuration normalizes completely";
		full.input = {
			V("SDLDrv.SDLClient", "StartupFullscreen", "On"),
			V("SDLDrv.SDLClient", "BorderlessWindow", "yes"),
			V("SDLDrv.SDLClient", "UseDesktopResolution", "False"),
			V("SDLDrv.SDLClient", "WindowedViewportX", "1024"),
			V("SDLDrv.SDLClient", "WindowedViewportY", "768"),
			V("SDLDrv.SDLClient", "FullscreenViewportX", "640"),
			V("SDLDrv.SDLClient", "FullscreenViewportY", "480"),
			V("Engine.GameEngine", "FrameRateLimit", "30.000000"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.850000"),
			V("SDLDrv.SDLClient", "UIScale", "1.25"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "1"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "4"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "16.000000"),
			V("SDLDrv.SDLClient", "Brightness", "0.400000"),
			V("ALAudio.ALAudioSubsystem", "SoundVolume", "0.900000"),
			V("ALAudio.ALAudioSubsystem", "MusicVolume", "0.53"),
			V("Engine.PlayerPawn", "MouseSensitivity", "3"),
			V("Engine.PlayerPawn", "Difficulty", "difficultyhard"),
			V("Engine.PlayerPawn", "ObjectDetail", "objectdetailhigh"),
			V("SDLDrv.SDLClient", "TextureDetail", "LOW"),
			V("HGame.Harry", "bAutoQuaff", "No"),
			V("Unrelated.Section", "KeepMe", "Untouched"),
		};
		full.expected = {
			V("SDLDrv.SDLClient", "StartupFullscreen", "False"),
			V("SDLDrv.SDLClient", "BorderlessWindow", "True"),
			V("SDLDrv.SDLClient", "UseDesktopResolution", "False"),
			V("SDLDrv.SDLClient", "WindowedViewportX", "640"),
			V("SDLDrv.SDLClient", "WindowedViewportY", "480"),
			V("SDLDrv.SDLClient", "FullscreenViewportX", "640"),
			V("SDLDrv.SDLClient", "FullscreenViewportY", "480"),
			V("Engine.GameEngine", "FrameRateLimit", "30"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.85"),
			V("SDLDrv.SDLClient", "UIScale", "1.25"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "True"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "4"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "16"),
			V("SDLDrv.SDLClient", "Brightness", "0.4"),
			V("ALAudio.ALAudioSubsystem", "SoundVolume", "0.9"),
			V("ALAudio.ALAudioSubsystem", "MusicVolume", "0.53"),
			V("Engine.PlayerPawn", "MouseSensitivity", "3"),
			V("Engine.PlayerPawn", "Difficulty", "DifficultyHard"),
			V("Engine.PlayerPawn", "ObjectDetail", "ObjectDetailHigh"),
			V("SDLDrv.SDLClient", "TextureDetail", "Low"),
			V("HGame.Harry", "bAutoQuaff", "False"),
			V("Unrelated.Section", "KeepMe", "Untouched"),
		};
		full.journal = {
			C("SDLDrv.SDLClient", "StartupFullscreen", "On", "True", "settings.boolean_spelling"),
			C("SDLDrv.SDLClient", "BorderlessWindow", "yes", "True", "settings.boolean_spelling"),
			C("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "1", "True", "settings.boolean_spelling"),
			C("HGame.Harry", "bAutoQuaff", "No", "False", "settings.boolean_spelling"),
			C("SDLDrv.SDLClient", "StartupFullscreen", "True", "False", "settings.screen_mode_consolidated"),
			C("SDLDrv.SDLClient", "WindowedViewportX", "1024", "640", "settings.viewport_consolidated"),
			C("SDLDrv.SDLClient", "WindowedViewportY", "768", "480", "settings.viewport_consolidated"),
			C("Engine.GameEngine", "FrameRateLimit", "30.000000", "30", "settings.frame_rate_limit_normalized"),
			C("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.850000", "0.85", "settings.scale_normalized"),
			C("SDLDrv.SDLClient", "Brightness", "0.400000", "0.4", "settings.range_normalized"),
			C("ALAudio.ALAudioSubsystem", "SoundVolume", "0.900000", "0.9", "settings.range_normalized"),
			C("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "16.000000", "16", "settings.anisotropy_normalized"),
			C("SDLDrv.SDLClient", "TextureDetail", "LOW", "Low", "settings.enum_normalized"),
			C("Engine.PlayerPawn", "ObjectDetail", "objectdetailhigh", "ObjectDetailHigh", "settings.enum_normalized"),
			C("Engine.PlayerPawn", "Difficulty", "difficultyhard", "DifficultyHard", "settings.enum_normalized"),
		};
		cases.push_back(full);
	}

	// Conflicting switches and invalid values resolve exactly the way the
	// loader would have resolved them implicitly.
	{
		MigrationCase invalid;
		invalid.description = "conflicting and invalid legacy values fall back to model defaults";
		invalid.input = {
			V("SDLDrv.SDLClient", "StartupFullscreen", "True"),
			V("SDLDrv.SDLClient", "BorderlessWindow", "True"),
			V("SDLDrv.SDLClient", "WindowedViewportX", "12"),
			V("SDLDrv.SDLClient", "WindowedViewportY", "768"),
			V("Engine.GameEngine", "FrameRateLimit", "59"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.9"),
			V("SDLDrv.SDLClient", "UIScale", "abc"),
			V("SDLDrv.SDLClient", "Brightness", "nan"),
			V("Engine.PlayerPawn", "MouseSensitivity", "not-a-number"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "maybe"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "8"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "32"),
			V("Engine.PlayerPawn", "Difficulty", "Ultra"),
			V("SDLDrv.SDLClient", "NativeText", "garbage"),
		};
		invalid.expected = {
			V("SDLDrv.SDLClient", "StartupFullscreen", "False"),
			V("SDLDrv.SDLClient", "BorderlessWindow", "True"),
			V("SDLDrv.SDLClient", "WindowedViewportX", "800"),
			V("SDLDrv.SDLClient", "WindowedViewportY", "600"),
			V("Engine.GameEngine", "FrameRateLimit", "60"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "1"),
			V("SDLDrv.SDLClient", "UIScale", "1"),
			V("SDLDrv.SDLClient", "Brightness", "0.4"),
			V("Engine.PlayerPawn", "MouseSensitivity", "3"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "False"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "0"),
			V("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "4"),
			V("Engine.PlayerPawn", "Difficulty", "DifficultyEasy"),
			V("SDLDrv.SDLClient", "NativeText", "True"),
		};
		invalid.journal = {
			C("SDLDrv.SDLClient", "NativeText", "garbage", "True", "settings.boolean_invalid_defaulted"),
			C("XOpenGLDrv.XOpenGLRenderDevice", "UseAA", "maybe", "False", "settings.boolean_invalid_defaulted"),
			C("SDLDrv.SDLClient", "StartupFullscreen", "True", "False", "settings.screen_mode_consolidated"),
			C("SDLDrv.SDLClient", "WindowedViewportX", "12", "800", "settings.viewport_consolidated"),
			C("SDLDrv.SDLClient", "WindowedViewportY", "768", "600", "settings.viewport_consolidated"),
			C("Engine.GameEngine", "FrameRateLimit", "59", "60", "settings.frame_rate_limit_invalid_defaulted"),
			C("XOpenGLDrv.XOpenGLRenderDevice", "RenderScale", "0.9", "1", "settings.scale_invalid_defaulted"),
			C("SDLDrv.SDLClient", "UIScale", "abc", "1", "settings.scale_invalid_defaulted"),
			C("SDLDrv.SDLClient", "Brightness", "nan", "0.4", "settings.range_invalid_defaulted"),
			C("Engine.PlayerPawn", "MouseSensitivity", "not-a-number", "3", "settings.range_invalid_defaulted"),
			C("XOpenGLDrv.XOpenGLRenderDevice", "NumAASamples", "8", "0", "settings.antialiasing_normalized"),
			C("XOpenGLDrv.XOpenGLRenderDevice", "MaxAnisotropy", "32", "4", "settings.anisotropy_invalid_defaulted"),
			C("Engine.PlayerPawn", "Difficulty", "Ultra", "DifficultyEasy", "settings.enum_invalid_defaulted"),
		};
		cases.push_back(invalid);
	}

	// Windowed mode keeps the launcher on the windowed resolution pair even
	// when a stale fullscreen pair disagrees.
	{
		MigrationCase windowed;
		windowed.description = "windowed mode consolidates onto the windowed resolution pair";
		windowed.input = {
			V("SDLDrv.SDLClient", "StartupFullscreen", "False"),
			V("SDLDrv.SDLClient", "WindowedViewportX", "1280"),
			V("SDLDrv.SDLClient", "WindowedViewportY", "800"),
			V("SDLDrv.SDLClient", "FullscreenViewportX", "640"),
			V("SDLDrv.SDLClient", "FullscreenViewportY", "480"),
		};
		windowed.expected = {
			V("SDLDrv.SDLClient", "StartupFullscreen", "False"),
			V("SDLDrv.SDLClient", "WindowedViewportX", "1280"),
			V("SDLDrv.SDLClient", "WindowedViewportY", "800"),
			V("SDLDrv.SDLClient", "FullscreenViewportX", "1280"),
			V("SDLDrv.SDLClient", "FullscreenViewportY", "800"),
		};
		windowed.journal = {
			C("SDLDrv.SDLClient", "FullscreenViewportX", "640", "1280", "settings.viewport_consolidated"),
			C("SDLDrv.SDLClient", "FullscreenViewportY", "480", "800", "settings.viewport_consolidated"),
		};
		cases.push_back(windowed);
	}

	// Unrelated rows and already-canonical owned values pass through without
	// touching the journal.
	{
		MigrationCase passthrough;
		passthrough.description = "canonical and unrelated rows leave no journal entries";
		passthrough.input = {
			V("Engine.Engine", "GameRenderDevice", "D3DDrv.D3DRenderDevice"),
			V("SDLDrv.SDLClient", "ShowFPS", "True"),
			V("Unrelated.Section", "MiXeDKey", "MiXeDValue"),
		};
		passthrough.expected = passthrough.input;
		cases.push_back(passthrough);
	}

	// Duplicate rows follow INI lookup semantics: the final occurrence wins
	// and earlier shadowed rows remain untouched.
	{
		MigrationCase duplicates;
		duplicates.description = "final duplicated row wins while shadowed rows stay untouched";
		duplicates.input = {
			V("Engine.GameEngine", "FrameRateLimit", "120"),
			V("Engine.GameEngine", "FrameRateLimit", "30.000000"),
		};
		duplicates.expected = {
			V("Engine.GameEngine", "FrameRateLimit", "120"),
			V("Engine.GameEngine", "FrameRateLimit", "30"),
		};
		duplicates.journal = {
			C("Engine.GameEngine", "FrameRateLimit", "30.000000", "30", "settings.frame_rate_limit_normalized"),
		};
		cases.push_back(duplicates);
	}

	for (const MigrationCase& testCase : cases)
	{
		std::vector<SettingsMigrationChange> changes;
		const std::vector<LegacySettingValue> migrated =
			MigrateLegacySettings(testCase.input, changes);
		ExpectRows(migrated, testCase.expected, testCase.description);
		ExpectJournal(changes, testCase.journal, testCase.description);

		// Migration is idempotent: rerunning it on migrated output is a no-op
		// with an empty journal.
		std::vector<SettingsMigrationChange> rerunChanges;
		const std::vector<LegacySettingValue> rerun =
			MigrateLegacySettings(migrated, rerunChanges);
		ExpectRows(rerun, testCase.expected, std::string(testCase.description) + " (idempotence)");
		Expect(rerunChanges.empty(), std::string(testCase.description) + ": rerun journal is empty");
	}
}

bool SameSelectionCore(const LaunchSelection& a, const LaunchSelection& b)
{
	return a.action == b.action && a.hasSave == b.hasSave &&
		a.save.saveIndex == b.save.saveIndex &&
		a.save.usesSlotDirectory == b.save.usesSlotDirectory &&
		a.save.slot == b.save.slot;
}

LaunchSelection ContinueSelection(int index, int slot)
{
	LaunchSelection selection;
	selection.action = LaunchAction::Continue;
	selection.hasSave = true;
	selection.save.saveIndex = index;
	selection.save.usesSlotDirectory = slot >= 0;
	selection.save.slot = slot;
	selection.save.displayName = "Slot 3 - Save 12";
	selection.save.savePath = "/runtime/derived/path.usa";
	return selection;
}

LaunchSelectionField F(const char* key, const char* value)
{
	return {key, value};
}

void TestLaunchSelectionSerializationContract()
{
	std::string error;

	LaunchSelection quit;
	LaunchSelection newGame;
	newGame.action = LaunchAction::NewGame;
	struct SerializableRow
	{
		const char* description;
		LaunchSelection selection;
	};
	const std::vector<SerializableRow> serializable = {
		{"quit selection serializes", quit},
		{"new game selection serializes", newGame},
		{"flat continue selection serializes", ContinueSelection(12, -1)},
		{"slotted continue selection serializes", ContinueSelection(12, 3)},
	};
	for (const SerializableRow& row : serializable)
	{
		std::vector<LaunchSelectionField> fields;
		Expect(SerializeLaunchSelection(row.selection, fields, error),
			std::string(row.description) + ": " + error);
		LaunchSelection restored;
		Expect(DeserializeLaunchSelection(fields, restored, error),
			std::string(row.description) + " deserializes: " + error);
		Expect(SameSelectionCore(restored, row.selection),
			std::string(row.description) + " round-trips identically");
		Expect(error.empty(), std::string(row.description) + " leaves no diagnostic");
	}

	// Canonical row shapes are minimal: Quit/NewGame carry no save rows and
	// slotted Continue carries exactly the slot coordinate.
	{
		std::vector<LaunchSelectionField> fields;
		Expect(SerializeLaunchSelection(quit, fields, error), "quit serializes");
		Expect(fields.size() == 1 && fields[0].key == "Action" && fields[0].value == "Quit",
			"quit serializes to a single Action row");
		fields.clear();
		Expect(SerializeLaunchSelection(ContinueSelection(12, 3), fields, error), "slotted continue serializes");
		Expect(fields.size() == 4 && fields[1].key == "HasSave" && fields[1].value == "True" &&
			fields[2].key == "SaveIndex" && fields[2].value == "12" &&
			fields[3].key == "SaveSlot" && fields[3].value == "3",
			"slotted continue serializes Action, HasSave, SaveIndex, and SaveSlot rows");
	}

	struct RejectedRow
	{
		const char* description;
		LaunchSelection selection;
	};
	LaunchSelection unsavedContinue = ContinueSelection(-1, -1);
	unsavedContinue.hasSave = false;
	LaunchSelection negativeSlot = ContinueSelection(5, -1);
	negativeSlot.save.usesSlotDirectory = true;
	negativeSlot.save.slot = -7;
	LaunchSelection errorAction;
	errorAction.action = LaunchAction::Error;
	const std::vector<RejectedRow> rejected = {
		{"continue without a save is not serializable", unsavedContinue},
		{"continue with a negative save index is not serializable", ContinueSelection(-1, -1)},
		{"slotted continue with a negative slot is not serializable", negativeSlot},
		{"error action is not serializable", errorAction},
	};
	for (const RejectedRow& row : rejected)
	{
		std::vector<LaunchSelectionField> fields;
		Expect(!SerializeLaunchSelection(row.selection, fields, error) && !error.empty() && fields.empty(),
			std::string(row.description));
	}

	// Malformed or inconsistent field tables fall back as a whole to Quit.
	LaunchSelection fallback;
	struct MalformedRow
	{
		const char* description;
		std::vector<LaunchSelectionField> fields;
	};
	const std::vector<MalformedRow> malformed = {
		{"empty store falls back to quit", {}},
		{"unknown action falls back to quit", {F("Action", "Bogus")}},
		{"continue without a save marker falls back to quit", {F("Action", "Continue")}},
		{"continue with an explicit false save marker falls back to quit",
			{F("Action", "Continue"), F("HasSave", "False"), F("SaveIndex", "5")}},
		{"continue with a missing save index falls back to quit",
			{F("Action", "Continue"), F("HasSave", "True")}},
		{"continue with a nonnumeric save index falls back to quit",
			{F("Action", "Continue"), F("HasSave", "True"), F("SaveIndex", "nope")}},
		{"continue with a negative save index falls back to quit",
			{F("Action", "Continue"), F("HasSave", "True"), F("SaveIndex", "-5")}},
		{"slotted continue with a bad slot falls back to quit",
			{F("Action", "Continue"), F("HasSave", "True"), F("SaveIndex", "5"), F("SaveSlot", "x")}},
		{"unrelated rows alone fall back to quit", {F("RetailRoot", "/Retail")}},
	};
	for (const MalformedRow& row : malformed)
	{
		LaunchSelection restored;
		Expect(!DeserializeLaunchSelection(row.fields, restored, error) && !error.empty(),
			std::string(row.description) + " reports malformed store");
		Expect(restored.action == LaunchAction::Quit && !restored.hasSave,
			std::string(row.description) + " falls back to the safe quit selection");
	}
	fallback = LaunchSelection();
	Expect(!DeserializeLaunchSelection({}, fallback, error) && fallback.action == LaunchAction::Quit,
		"fallback selection is populated even when parsing fails");

	// Row order is free, later duplicates win, case-insensitive keys and
	// actions are accepted, and unrelated rows are ignored.
	{
		LaunchSelection restored;
		const std::vector<LaunchSelectionField> reordered = {
			F("SaveSlot", "3"), F("Unrelated", "ignored"), F("saveindex", "12"),
			F("hassave", "True"), F("ACTION", "CONTINUE"),
		};
		Expect(DeserializeLaunchSelection(reordered, restored, error),
			"field order, casing, and unrelated rows are tolerated: " + error);
		Expect(SameSelectionCore(restored, ContinueSelection(12, 3)),
			"reordered case-insensitive rows restore the slotted continue selection");

		const std::vector<LaunchSelectionField> duplicated = {
			F("Action", "NewGame"), F("Action", "Continue"),
			F("HasSave", "True"), F("SaveIndex", "7"),
		};
		Expect(DeserializeLaunchSelection(duplicated, restored, error),
			"duplicated rows deserialize: " + error);
		Expect(SameSelectionCore(restored, ContinueSelection(7, -1)),
			"the final duplicated action row wins");
	}
}

std::size_t CountOccurrences(const std::string& text, const std::string& needle)
{
	std::size_t count = 0;
	for (std::size_t at = text.find(needle); at != std::string::npos; at = text.find(needle, at + needle.size()))
		++count;
	return count;
}

void TestLaunchSelectionPersistenceRoundTrip()
{
	TemporaryRoots roots;
	const fs::path catalog = roots.user / "Launcher.ini";
	std::string error;

	// Round-trip contract: commit, reload, identical selection for every
	// canonical shape.
	LaunchSelection quit;
	LaunchSelection newGame;
	newGame.action = LaunchAction::NewGame;
	struct PersistedRow
	{
		const char* description;
		LaunchSelection selection;
	};
	const std::vector<PersistedRow> persisted = {
		{"quit selection persists", quit},
		{"new game selection persists", newGame},
		{"flat continue selection persists", ContinueSelection(12, -1)},
		{"slotted continue selection persists", ContinueSelection(12, 3)},
	};
	for (const PersistedRow& row : persisted)
	{
		Expect(CommitLaunchSelection(roots.user.string(), row.selection, error),
			std::string(row.description) + " commits: " + error);
		LaunchSelection restored;
		Expect(LoadLaunchSelection(roots.user.string(), restored, error),
			std::string(row.description) + " reloads: " + error);
		Expect(SameSelectionCore(restored, row.selection),
			std::string(row.description) + " round-trips identically");
	}

	// The store stays minimal: a NewGame catalog carries no save rows at all.
	Expect(CommitLaunchSelection(roots.user.string(), newGame, error), "minimal selection commits");
	const std::string minimalStore = ReadBytes(catalog);
	Expect(minimalStore.find("[LastLaunch]") != std::string::npos &&
		minimalStore.find("Action=NewGame") != std::string::npos &&
		minimalStore.find("SaveIndex") == std::string::npos &&
		minimalStore.find("HasSave") == std::string::npos,
		"non-continue selections persist only their action row");

	// Malformed stores fall back to the safe Quit selection with the whole
	// ordering documented in the pure contract.
	struct MalformedCatalog
	{
		const char* description;
		std::string contents;
	};
	const std::vector<MalformedCatalog> malformed = {
		{"an empty catalog falls back to quit", ""},
		{"an unknown action falls back to quit", "[LastLaunch]\nAction=Bogus\n"},
		{"continue without a save index falls back to quit",
			"[LastLaunch]\nAction=Continue\nHasSave=True\n"},
		{"a nonnumeric save index falls back to quit",
			"[LastLaunch]\nAction=Continue\nHasSave=True\nSaveIndex=nope\n"},
		{"a negative save index falls back to quit",
			"[LastLaunch]\nAction=Continue\nHasSave=True\nSaveIndex=-5\n"},
		{"a bad slot coordinate falls back to quit",
			"[LastLaunch]\nAction=Continue\nHasSave=True\nSaveIndex=5\nSaveSlot=x\n"},
		{"an inconsistent save marker falls back to quit",
			"[LastLaunch]\nAction=Continue\nHasSave=False\nSaveIndex=5\n"},
	};
	for (const MalformedCatalog& row : malformed)
	{
		WriteBytes(catalog, row.contents);
		LaunchSelection restored;
		Expect(!LoadLaunchSelection(roots.user.string(), restored, error) && !error.empty(),
			std::string(row.description) + " reports a malformed store");
		Expect(restored.action == LaunchAction::Quit && !restored.hasSave,
			std::string(row.description));
	}

	// A missing catalog behaves like a malformed one: usable fallback, no
	// crash, no implicit file creation.
	TemporaryRoots missingRoots;
	LaunchSelection restoredMissing;
	error.clear();
	Expect(!LoadLaunchSelection(missingRoots.user.string(), restoredMissing, error) &&
		!error.empty() && restoredMissing.action == LaunchAction::Quit,
		"a missing catalog yields the quit fallback without creating files");
	Expect(!fs::exists(missingRoots.user / "Launcher.ini"),
		"loading a missing catalog never materializes it");

	// Selection persistence coexists with the data source catalog: unrelated
	// sections survive untouched across selection commits.
	WriteBytes(catalog, "[DataSources]\nSelected=Prototype\n");
	DataSourceConfiguration sources;
	Expect(LoadDataSourceConfiguration(roots.user.string(), sources, error) &&
		sources.selected == DataSource::Prototype,
		"data source catalog loads alongside launch selection state");
	Expect(CommitLaunchSelection(roots.user.string(), ContinueSelection(7, -1), error),
		"selection commits into an existing catalog: " + error);
	sources = DataSourceConfiguration();
	LaunchSelection combined;
	Expect(LoadDataSourceConfiguration(roots.user.string(), sources, error) &&
		sources.selected == DataSource::Prototype,
		"selection commit preserves the data source section");
	Expect(LoadLaunchSelection(roots.user.string(), combined, error) &&
		SameSelectionCore(combined, ContinueSelection(7, -1)),
		"both catalogs coexist after a selection commit");
}

void TestDuplicateLaunchSelectionCommitKeepsLastWriter()
{
	TemporaryRoots roots;
	const fs::path catalog = roots.user / "Launcher.ini";
	std::string error;

	// Two sequential saves of different selections: the second publication
	// atomically replaces the first (last writer wins), leaving exactly one
	// canonical action row behind.
	const LaunchSelection first = ContinueSelection(7, -1);
	LaunchSelection second;
	second.action = LaunchAction::NewGame;
	Expect(CommitLaunchSelection(roots.user.string(), first, error),
		"duplicate-write first commit succeeds: " + error);
	const std::string firstStore = ReadBytes(catalog);
	Expect(!firstStore.empty() && !fs::exists(catalog.string() + ".bak"),
		"the first commit creates no backup because no prior generation existed");

	Expect(CommitLaunchSelection(roots.user.string(), second, error),
		"duplicate-write second commit succeeds: " + error);
	LaunchSelection reloaded;
	Expect(LoadLaunchSelection(roots.user.string(), reloaded, error),
		"second generation reloads: " + error);
	Expect(SameSelectionCore(reloaded, second),
		"the last writer's selection is the persisted one");

	const std::string secondStore = ReadBytes(catalog);
	Expect(secondStore != firstStore, "the second commit rewrote the catalog");
	Expect(CountOccurrences(secondStore, "Action=") == 1,
		"exactly one canonical action row survives duplicate writes");
	Expect(secondStore.find("SaveIndex") == std::string::npos && secondStore.find("HasSave") == std::string::npos,
		"the replaced generation leaves no stale save coordinates");
	Expect(fs::exists(catalog.string() + ".bak") && ReadBytes(catalog.string() + ".bak") == firstStore,
		"the previous generation is preserved exactly as the backup");

	// A third write keeps winning while the original backup is never
	// overwritten, matching settings-commit semantics.
	const LaunchSelection third = ContinueSelection(12, 3);
	Expect(CommitLaunchSelection(roots.user.string(), third, error),
		"duplicate-write third commit succeeds: " + error);
	Expect(LoadLaunchSelection(roots.user.string(), reloaded, error) &&
		SameSelectionCore(reloaded, third),
		"the third generation wins");
	const std::string thirdStore = ReadBytes(catalog);
	Expect(CountOccurrences(thirdStore, "Action=") == 1 &&
		thirdStore.find("SaveIndex=12") != std::string::npos &&
		thirdStore.find("SaveSlot=3") != std::string::npos,
		"the third generation is complete and singular");
	Expect(ReadBytes(catalog.string() + ".bak") == firstStore,
		"the first-generation backup is never overwritten by later writes");
}
 }


int main()
{
	GMalloc = &RuntimeMalloc;
	GMalloc->Init();
	TestPolicyClassification();
	TestSelectedCommands();
	TestValidationBoundaries();
	TestScaleDefaultsAndValidation();
	TestSaveDiscovery();
	TestConfigRoundTripAndBackups();
	TestEveryScaleRoundTrip();
	TestModernOptionRoundTrips();
	TestReadOnlyCancellationAndMaterialization();
	TestMalformedRecovery();
	TestUtf16AndInvalidEncodingRecovery();
	TestUnsafeBackupRejected();
	TestConventionalDataRoots();
	TestDataSourceCatalogPersistence();
	TestMalformedDataSourceCatalogRecovery();
	TestDataSourceProfilesAndMigration();
	TestDataSourceMigrationFailureAndQuit();
	TestInstalledDataRootIsolation();
	TestDataDirectoryArgumentContract();
	TestLegacySettingsMigration();
	TestLaunchSelectionSerializationContract();
	TestLaunchSelectionPersistenceRoundTrip();
	TestDuplicateLaunchSelectionCommitKeepsLastWriter();
	if (Failures != 0)
		std::fprintf(stderr, "%d launcher test(s) failed\n", Failures);
	return Failures == 0 ? 0 : 1;
}
