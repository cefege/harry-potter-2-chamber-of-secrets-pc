#include "HP2LaunchPolicy.h"
#include "HP2LauncherStore.h"

#include <cmath>
#include <cctype>
#include <cstdio>
#include <cstdlib>
#include <filesystem>
#include <fstream>
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
		"MaintainVerticalFOV=True\nScreenFlashes=True\n"
		"[XOpenGLDrv.XOpenGLRenderDevice]\n"
		"UseAA=False\nNumAASamples=0\nMaxAnisotropy=4\n"
		"[ALAudio.ALAudioSubsystem]\nMusicVolume=0.53\nSoundVolume=0.9\n");
	WriteBytes(roots.system / "DefUser.ini",
		"; immutable user default\n"
		"[Engine.PlayerPawn]\nDifficulty=DifficultyEasy\n"
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

	const char* names[] = {"LOAD", "NOFRONTEND", "TESTTICKS", "TESTRENDEV", "SERVER", "REPLAY",
		"RECORD", "BENCHMARK", "COMMANDLET", "MAKE", "EXEC"};
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
	settings.frameRateLimit = 120;
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
	for (int cap : {0, 30, 60, 120}) { bad = settings; bad.frameRateLimit = cap; Expect(Valid(bad), "allowed frame cap is accepted"); }
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
	settings.frameRateLimit = 120;
	settings.maintainVerticalFOV = false;
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
		"MaintainVerticalFOV=True\r\nScreenFlashes=True\r\n"
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
	Expect(game.find("frAMeRateLimit = 120 ; final value") != std::string::npos, "only final repeated owned key value is replaced while case/comment remain");
	Expect(user.find("Difficulty=DifficultyEasy") != std::string::npos, "earlier repeated user key remains untouched");
	Expect(user.find("Difficulty = DifficultyHard ; final") != std::string::npos, "final user key is replaced in place");
	Expect(game.find("UIScale=1.0") != std::string::npos, "earlier repeated UI scale remains untouched");
	Expect(game.find("UIScale = 1.75 ; final UI scale") != std::string::npos, "final UI scale is replaced in place");
	Expect(game.find("RenderScale=0.67") != std::string::npos, "earlier repeated render scale remains untouched");
	Expect(game.find("RenderScale = 0.85 ; final render scale") != std::string::npos, "final render scale is replaced in place");
	Expect(game.find("DriverNote=PreserveRendererSetting") != std::string::npos, "unrelated renderer setting is preserved");
	Expect(game.find("MaintainVerticalFOV=False") != std::string::npos &&
		game.find("ScreenFlashes=False") != std::string::npos,
		"widescreen and flash preferences persist under SDLDrv.SDLClient");
	Expect(game.find("UseAA=True") != std::string::npos &&
		game.find("NumAASamples=4") != std::string::npos &&
		game.find("MaxAnisotropy=16") != std::string::npos,
		"MSAA and anisotropy persist under XOpenGLDrv");
	Expect(user.find("MaintainVerticalFOV") == std::string::npos &&
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
	Expect(loaded.settings.verticalSync && loaded.settings.frameRateLimit == 120, "VSync and frame cap round-trip");
	Expect(!loaded.settings.maintainVerticalFOV && loaded.settings.antiAliasingSamples == 4 &&
		loaded.settings.anisotropy == 16, "widescreen, MSAA, and anisotropy round-trip");
	Expect(loaded.settings.renderScale == 0.85 && loaded.settings.uiScale == 1.75, "render and UI scales round-trip");
	Expect(loaded.settings.textureDetail == TextureDetail::Low && loaded.settings.objectDetail == ObjectDetail::VeryHigh, "detail levels round-trip");
	Expect(!loaded.settings.soundEnabled && loaded.settings.soundVolume == 0.25 && loaded.settings.musicVolume == 0.5, "audio settings round-trip");
	Expect(loaded.settings.mouseSensitivity == 7.25 && loaded.settings.invertMouse, "mouse settings round-trip");
	Expect(loaded.settings.difficulty == Difficulty::Hard, "difficulty round-trips");
	Expect(!loaded.settings.autoCenterCamera && !loaded.settings.moveWhileCasting && !loaded.settings.autoQuaff, "gameplay toggles round-trip");
	Expect(!loaded.settings.screenFlashes, "screen flash preference round-trips");

	selected.frameRateLimit = 30;
	Expect(CommitLauncherSettings(roots.paths(), selected, error), "second settings commit succeeds");
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
		settings.maintainVerticalFOV = enabled;
		settings.screenFlashes = enabled;
		Expect(CommitLauncherSettings(roots.paths(), settings, error), "modern boolean settings commit");
		LauncherState loaded;
		Expect(LoadLauncherState(roots.paths(), loaded, error), "modern boolean settings reload");
		Expect(loaded.settings.maintainVerticalFOV == enabled, "widescreen preference round-trips exactly");
		Expect(loaded.settings.screenFlashes == enabled, "screen flash preference round-trips exactly");
	}

	WriteBytes(roots.user / "Game.ini",
		"[SDLDrv.SDLClient]\nMaintainVerticalFOV=True\nScreenFlashes=True\n"
		"[XOpenGLDrv.XOpenGLRenderDevice]\n"
		"UseAA=False\nNumAASamples=4\nMaxAnisotropy=8.000000\n");
	LauncherState loaded;
	Expect(LoadLauncherState(roots.paths(), loaded, error), "runtime-formatted modern options load");
	Expect(loaded.settings.antiAliasingSamples == 0, "disabled UseAA takes precedence over a stale sample count");
	Expect(loaded.settings.anisotropy == 8, "floating-point renderer anisotropy text loads as a discrete value");
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
		"MaintainVerticalFOV=maybe\nScreenFlashes=sometimes\n"
		"[XOpenGLDrv.XOpenGLRenderDevice]\nRenderScale=0.8\nUseAA=True\nNumAASamples=8\nMaxAnisotropy=2\n";
	const std::string malformedUser =
		"[Engine.PlayerPawn\nDifficulty=DifficultyHard\n"
		"[Engine.PlayerPawn]\nDifficulty=Unknown\nMouseSensitivity=not-a-number\n"
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
	Expect(state.settings.maintainVerticalFOV == defaults.maintainVerticalFOV &&
		state.settings.screenFlashes == defaults.screenFlashes,
		"invalid modern booleans fall back to model defaults");
	Expect(state.settings.antiAliasingSamples == defaults.antiAliasingSamples &&
		state.settings.anisotropy == defaults.anisotropy,
		"invalid MSAA and anisotropy values fall back to model defaults");
	Expect(state.settings.difficulty == defaults.difficulty && state.settings.mouseSensitivity == defaults.mouseSensitivity,
		"invalid user values fall back to model defaults");
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
}

int main()
{
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
	if (Failures != 0)
		std::fprintf(stderr, "%d launcher test(s) failed\n", Failures);
	return Failures == 0 ? 0 : 1;
}
