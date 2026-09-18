/*=============================================================================
	HP2LauncherModel.h: Portable launcher data model.
=============================================================================*/
#ifndef HP2LAUNCHERMODEL_H
#define HP2LAUNCHERMODEL_H

#include <array>
#include <cstdint>
#include <string>
#include <vector>

namespace HP2Launcher
{
enum class LaunchAction
{
	Continue,
	NewGame,
	Quit,
	Error
};

enum class ScreenMode
{
	Windowed,
	Fullscreen,
	BorderlessDesktop
};

enum class TextureDetail
{
	Low,
	Medium,
	High
};

enum class ObjectDetail
{
	VeryLow,
	Low,
	Medium,
	High,
	VeryHigh
};

enum class Difficulty
{
	Easy,
	Medium,
	Hard
};

enum class ControlMode
{
	Classic,
	Modern
};

enum class RenderBackend
{
	XOpenGL,
	Vulkan
};
enum class DataSource
{
	Retail,
	Prototype
};

struct DataSourceConfiguration
{
	DataSource selected = DataSource::Retail;
	std::string retailRoot;
	std::string prototypeRoot;
};

struct DataSourceOption
{
	DataSource source = DataSource::Retail;
	std::string root;
	bool available = false;
	std::string error;
};


struct DisplayResolution
{
	int width = 800;
	int height = 600;
	std::string label;
};
inline constexpr std::array<double, 5> RenderScaleValues = {{0.50, 0.67, 0.75, 0.85, 1.00}};
inline constexpr std::array<double, 6> UIScaleValues = {{0.75, 1.00, 1.25, 1.50, 1.75, 2.00}};
inline constexpr std::array<int, 3> AntiAliasingSampleValues = {{0, 2, 4}};
inline constexpr std::array<int, 4> AnisotropyValues = {{0, 4, 8, 16}};
inline constexpr std::array<int, 5> FrameRateLimitValues = {{0, 30, 60, 120, 144}};

struct SaveRecord
{
	int slot = -1;
	int saveIndex = 0;
	bool usesSlotDirectory = false;
	std::string savePath;
	std::string thumbnailPath;
	std::string displayName;
	std::int64_t size = 0;
	std::int64_t modifiedSeconds = 0;
};

struct LauncherSettings
{
	ScreenMode screenMode = ScreenMode::Windowed;
	DisplayResolution resolution;
	RenderBackend renderBackend = RenderBackend::XOpenGL;
	bool verticalSync = false;
	double renderScale = 1.0;
	double uiScale = 1.0;
	int frameRateLimit = 144;
	bool showFPS = false;
	bool maintainVerticalFOV = true;
	bool nativeText = true;
	int antiAliasingSamples = 4;
	int anisotropy = 16;
	double brightness = 0.4;
	TextureDetail textureDetail = TextureDetail::High;
	ObjectDetail objectDetail = ObjectDetail::VeryHigh;
	bool soundEnabled = true;
	double soundVolume = 0.9;
	double musicVolume = 0.53;
	double mouseSensitivity = 3.0;
	bool invertMouse = false;
	ControlMode controlMode = ControlMode::Classic;
	bool autoCenterCamera = true;
	bool moveWhileCasting = true;
	bool autoQuaff = true;
	bool screenFlashes = true;
	Difficulty difficulty = Difficulty::Easy;
	bool joystickEnabled = true;
};

struct LauncherPaths
{
	std::string userRoot;
	std::string systemRoot;
};

struct LauncherState
{
	LauncherSettings settings;
	std::vector<SaveRecord> saves;
};

struct LauncherRequest
{
	LauncherSettings settings;
	std::vector<SaveRecord> saves;
	std::vector<DisplayResolution> displayModes;
	std::string errorMessage;
	std::string userRoot;
	std::string logPath;
	std::vector<DataSourceOption> dataSourceOptions;
	DataSourceConfiguration dataSources;
	bool hasExplicitDataRootOverride = false;
	std::string explicitDataRoot;
};

struct LaunchSelection
{
	LaunchAction action = LaunchAction::Quit;
	bool hasSave = false;
	SaveRecord save;
};

struct LauncherResult
{
	LaunchSelection selection;
	LauncherSettings settings;
	DataSourceConfiguration dataSources;
};

}

#endif
