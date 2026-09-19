/*=============================================================================
	HP2ShellLauncher.cpp: Quickshell-based launcher backend for Linux.
=============================================================================*/

#include "HP2ShellLauncher.h"
#include "HP2LauncherModel.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>
#include <spawn.h>

namespace
{
	// Escape and write a JSON string value (handles quotes, backslashes, control chars)
	void WriteJsonString(std::FILE* file, const char* str)
	{
		std::fputc('"', file);
		if (str)
		{
			for (; *str; ++str)
			{
				unsigned char ch = static_cast<unsigned char>(*str);
				if (ch == '"' || ch == '\\')
				{
					std::fputc('\\', file);
					std::fputc(ch, file);
				}
				else if (ch < 32)
				{
					std::fprintf(file, "\\u%04x", ch);
				}
				else
				{
					std::fputc(ch, file);
				}
			}
		}
		std::fputc('"', file);
	}

	// Write a JSON number value (double with %.6g formatting)
	void WriteJsonNumber(std::FILE* file, double value)
	{
		std::fprintf(file, "%.6g", value);
	}

	// Write a JSON boolean
	void WriteJsonBool(std::FILE* file, bool value)
	{
		std::fputs(value ? "true" : "false", file);
	}

	// Build and write the request JSON to a file
	bool WriteRequestJson(const std::string& path, const HP2Launcher::LauncherRequest& req)
	{
		std::FILE* file = std::fopen(path.c_str(), "w");
		if (!file)
			return false;

		std::fputs("{\"schema\":2,\"userRoot\":", file);
		WriteJsonString(file, req.userRoot.c_str());
		std::fputs(",\"logPath\":", file);
		WriteJsonString(file, req.logPath.c_str());
		std::fputs(",\"errorMessage\":", file);
		WriteJsonString(file, req.errorMessage.c_str());
		std::fputs(",\"hasExplicitDataRootOverride\":", file);
		WriteJsonBool(file, req.hasExplicitDataRootOverride);
		std::fputs(",\"explicitDataRoot\":", file);
		WriteJsonString(file, req.explicitDataRoot.c_str());

		// Data source configuration: which of retail/prototype is selected,
		// plus the persisted root for each.
		std::fputs(",\"dataSources\":{\"selected\":", file);
		WriteJsonString(file, req.dataSources.selected == HP2Launcher::DataSource::Retail ? "retail" : "prototype");
		std::fputs(",\"retailRoot\":", file);
		WriteJsonString(file, req.dataSources.retailRoot.c_str());
		std::fputs(",\"prototypeRoot\":", file);
		WriteJsonString(file, req.dataSources.prototypeRoot.c_str());
		std::fputs("}", file);

		// Data source options: availability/error probed for each source.
		std::fputs(",\"dataSourceOptions\":[", file);
		for (size_t i = 0; i < req.dataSourceOptions.size(); ++i)
		{
			if (i > 0) std::fputc(',', file);
			const auto& option = req.dataSourceOptions[i];
			std::fputs("{\"source\":", file);
			WriteJsonString(file, option.source == HP2Launcher::DataSource::Retail ? "retail" : "prototype");
			std::fputs(",\"root\":", file);
			WriteJsonString(file, option.root.c_str());
			std::fputs(",\"available\":", file);
			WriteJsonBool(file, option.available);
			std::fputs(",\"error\":", file);
			WriteJsonString(file, option.error.c_str());
			std::fputc('}', file);
		}
		std::fputs("]", file);

		// Settings object
		std::fputs(",\"settings\":{", file);
		std::fprintf(file, "\"screenMode\":");
		switch (req.settings.screenMode)
		{
			case HP2Launcher::ScreenMode::Windowed: WriteJsonString(file, "windowed"); break;
			case HP2Launcher::ScreenMode::Fullscreen: WriteJsonString(file, "fullscreen"); break;
			case HP2Launcher::ScreenMode::BorderlessDesktop: WriteJsonString(file, "borderlessDesktop"); break;
		}
		std::fprintf(file, ",\"renderBackend\":");
		switch (req.settings.renderBackend)
		{
			case HP2Launcher::RenderBackend::XOpenGL: WriteJsonString(file, "xopengl"); break;
			case HP2Launcher::RenderBackend::Vulkan: WriteJsonString(file, "vulkan"); break;
		}
		std::fprintf(file, ",\"textureDetail\":");
		switch (req.settings.textureDetail)
		{
			case HP2Launcher::TextureDetail::Low: WriteJsonString(file, "low"); break;
			case HP2Launcher::TextureDetail::Medium: WriteJsonString(file, "medium"); break;
			case HP2Launcher::TextureDetail::High: WriteJsonString(file, "high"); break;
		}
		std::fprintf(file, ",\"objectDetail\":");
		switch (req.settings.objectDetail)
		{
			case HP2Launcher::ObjectDetail::VeryLow: WriteJsonString(file, "veryLow"); break;
			case HP2Launcher::ObjectDetail::Low: WriteJsonString(file, "low"); break;
			case HP2Launcher::ObjectDetail::Medium: WriteJsonString(file, "medium"); break;
			case HP2Launcher::ObjectDetail::High: WriteJsonString(file, "high"); break;
			case HP2Launcher::ObjectDetail::VeryHigh: WriteJsonString(file, "veryHigh"); break;
		}
		std::fprintf(file, ",\"difficulty\":");
		switch (req.settings.difficulty)
		{
			case HP2Launcher::Difficulty::Easy: WriteJsonString(file, "easy"); break;
			case HP2Launcher::Difficulty::Medium: WriteJsonString(file, "medium"); break;
			case HP2Launcher::Difficulty::Hard: WriteJsonString(file, "hard"); break;
		}
		std::fprintf(file, ",\"controlMode\":");
		switch (req.settings.controlMode)
		{
			case HP2Launcher::ControlMode::Classic: WriteJsonString(file, "classic"); break;
			case HP2Launcher::ControlMode::Modern: WriteJsonString(file, "modern"); break;
		}
		std::fprintf(file, ",\"resolutionWidth\":%d,\"resolutionHeight\":%d",
			req.settings.resolution.width, req.settings.resolution.height);
		std::fprintf(file, ",\"verticalSync\":");
		WriteJsonBool(file, req.settings.verticalSync);
		std::fprintf(file, ",\"renderScale\":%.6g", req.settings.renderScale);
		std::fprintf(file, ",\"uiScale\":%.6g", req.settings.uiScale);
		std::fprintf(file, ",\"frameRateLimit\":%d", req.settings.frameRateLimit);
		std::fprintf(file, ",\"showFPS\":");
		WriteJsonBool(file, req.settings.showFPS);
		std::fprintf(file, ",\"maintainVerticalFOV\":");
		WriteJsonBool(file, req.settings.maintainVerticalFOV);
		std::fprintf(file, ",\"nativeText\":");
		WriteJsonBool(file, req.settings.nativeText);
		std::fprintf(file, ",\"antiAliasingSamples\":%d", req.settings.antiAliasingSamples);
		std::fprintf(file, ",\"anisotropy\":%d", req.settings.anisotropy);
		std::fprintf(file, ",\"brightness\":%.6g", req.settings.brightness);
		std::fprintf(file, ",\"soundEnabled\":");
		WriteJsonBool(file, req.settings.soundEnabled);
		std::fprintf(file, ",\"soundVolume\":%.6g", req.settings.soundVolume);
		std::fprintf(file, ",\"musicVolume\":%.6g", req.settings.musicVolume);
		std::fprintf(file, ",\"mouseSensitivity\":%.6g", req.settings.mouseSensitivity);
		std::fprintf(file, ",\"invertMouse\":");
		WriteJsonBool(file, req.settings.invertMouse);
		std::fprintf(file, ",\"autoCenterCamera\":");
		WriteJsonBool(file, req.settings.autoCenterCamera);
		std::fprintf(file, ",\"moveWhileCasting\":");
		WriteJsonBool(file, req.settings.moveWhileCasting);
		std::fprintf(file, ",\"autoQuaff\":");
		WriteJsonBool(file, req.settings.autoQuaff);
		std::fprintf(file, ",\"screenFlashes\":");
		WriteJsonBool(file, req.settings.screenFlashes);
		std::fprintf(file, ",\"joystickEnabled\":");
		WriteJsonBool(file, req.settings.joystickEnabled);
		std::fputs("}", file);

		// Choices object
		std::fputs(",\"choices\":{", file);
		std::fputs("\"renderScale\":[", file);
		for (size_t i = 0; i < HP2Launcher::RenderScaleValues.size(); ++i)
		{
			if (i > 0) std::fputc(',', file);
			WriteJsonNumber(file, HP2Launcher::RenderScaleValues[i]);
		}
		std::fputs("],\"uiScale\":[", file);
		for (size_t i = 0; i < HP2Launcher::UIScaleValues.size(); ++i)
		{
			if (i > 0) std::fputc(',', file);
			WriteJsonNumber(file, HP2Launcher::UIScaleValues[i]);
		}
		std::fputs("],\"antiAliasingSamples\":[", file);
		for (size_t i = 0; i < HP2Launcher::AntiAliasingSampleValues.size(); ++i)
		{
			if (i > 0) std::fputc(',', file);
			std::fprintf(file, "%d", HP2Launcher::AntiAliasingSampleValues[i]);
		}
		std::fputs("],\"anisotropy\":[", file);
		for (size_t i = 0; i < HP2Launcher::AnisotropyValues.size(); ++i)
		{
			if (i > 0) std::fputc(',', file);
			std::fprintf(file, "%d", HP2Launcher::AnisotropyValues[i]);
		}
		std::fputs("],\"frameRateLimit\":[", file);
		for (size_t i = 0; i < HP2Launcher::FrameRateLimitValues.size(); ++i)
		{
			if (i > 0) std::fputc(',', file);
			std::fprintf(file, "%d", HP2Launcher::FrameRateLimitValues[i]);
		}
		std::fputs("]}", file);

		// Saves array
		std::fputs(",\"saves\":[", file);
		for (size_t i = 0; i < req.saves.size(); ++i)
		{
			if (i > 0) std::fputc(',', file);
			const auto& save = req.saves[i];
			std::fputs("{\"slot\":", file);
			std::fprintf(file, "%d", save.slot);
			std::fputs(",\"saveIndex\":", file);
			std::fprintf(file, "%d", save.saveIndex);
			std::fputs(",\"usesSlotDirectory\":", file);
			WriteJsonBool(file, save.usesSlotDirectory);
			std::fputs(",\"savePath\":", file);
			WriteJsonString(file, save.savePath.c_str());
			std::fputs(",\"thumbnailPath\":", file);
			WriteJsonString(file, save.thumbnailPath.c_str());
			std::fputs(",\"displayName\":", file);
			WriteJsonString(file, save.displayName.c_str());
			std::fputs(",\"size\":", file);
			std::fprintf(file, "%lld", (long long)save.size);
			std::fputs(",\"modifiedSeconds\":", file);
			std::fprintf(file, "%lld", (long long)save.modifiedSeconds);
			std::fputc('}', file);
		}
		std::fputs("],", file);

		// Display modes array
		std::fputs("\"displayModes\":[", file);
		for (size_t i = 0; i < req.displayModes.size(); ++i)
		{
			if (i > 0) std::fputc(',', file);
			const auto& mode = req.displayModes[i];
			std::fputs("{\"width\":", file);
			std::fprintf(file, "%d", mode.width);
			std::fputs(",\"height\":", file);
			std::fprintf(file, "%d", mode.height);
			std::fputs(",\"label\":", file);
			WriteJsonString(file, mode.label.c_str());
			std::fputc('}', file);
		}
		std::fputs("]}", file);

		if (std::fclose(file) != 0)
			return false;

		return true;
	}

	// Parse result.conf key=value file
	bool ParseResultConf(const std::string& path, HP2Launcher::LauncherResult& result)
	{
		std::FILE* file = std::fopen(path.c_str(), "r");
		if (!file)
			return false;

		char line[1024];
		std::string action;

		while (std::fgets(line, sizeof(line), file))
		{
			// Remove trailing newline
			size_t len = std::strlen(line);
			if (len > 0 && line[len - 1] == '\n')
				line[--len] = '\0';

			// Skip comments and empty lines
			if (line[0] == '#' || line[0] == '\0')
				continue;

			// Parse key=value
			const char* eq = std::strchr(line, '=');
			if (!eq)
				continue;

			std::string key(line, static_cast<size_t>(eq - line));
			const char* value = eq + 1;

			if (key == "action")
			{
				action = value;
			}
			else if (key == "message")
			{
				// For error messages
				// (would be stored in a separate error field if needed)
			}
			else if (key == "saveIndex")
			{
				result.selection.save.saveIndex = std::atoi(value);
			}
			else if (key == "slot")
			{
				result.selection.save.slot = std::atoi(value);
			}
			else if (key == "usesSlotDirectory")
			{
				result.selection.save.usesSlotDirectory = std::strcmp(value, "true") == 0;
			}
			else if (key == "savePath")
			{
				result.selection.save.savePath = value;
			}
			else if (key.substr(0, 12) == "dataSources.")
			{
				const std::string sourceField = key.substr(12);
				if (sourceField == "selected")
				{
					result.dataSources.selected = std::strcmp(value, "retail") == 0
						? HP2Launcher::DataSource::Retail : HP2Launcher::DataSource::Prototype;
				}
				else if (sourceField == "retailRoot")
					result.dataSources.retailRoot = value;
				else if (sourceField == "prototypeRoot")
					result.dataSources.prototypeRoot = value;
			}
			else if (key.substr(0, 9) == "settings.")
			{
				std::string settingName = key.substr(9);
				// Parse settings (simplified; the QML will emit all settings keys)
				if (settingName == "screenMode")
				{
					if (std::strcmp(value, "windowed") == 0)
						result.settings.screenMode = HP2Launcher::ScreenMode::Windowed;
					else if (std::strcmp(value, "fullscreen") == 0)
						result.settings.screenMode = HP2Launcher::ScreenMode::Fullscreen;
					else if (std::strcmp(value, "borderlessDesktop") == 0)
						result.settings.screenMode = HP2Launcher::ScreenMode::BorderlessDesktop;
				}
				else if (settingName == "textureDetail")
				{
					if (std::strcmp(value, "low") == 0)
						result.settings.textureDetail = HP2Launcher::TextureDetail::Low;
					else if (std::strcmp(value, "medium") == 0)
						result.settings.textureDetail = HP2Launcher::TextureDetail::Medium;
					else if (std::strcmp(value, "high") == 0)
						result.settings.textureDetail = HP2Launcher::TextureDetail::High;
				}
				else if (settingName == "objectDetail")
				{
					if (std::strcmp(value, "veryLow") == 0)
						result.settings.objectDetail = HP2Launcher::ObjectDetail::VeryLow;
					else if (std::strcmp(value, "low") == 0)
						result.settings.objectDetail = HP2Launcher::ObjectDetail::Low;
					else if (std::strcmp(value, "medium") == 0)
						result.settings.objectDetail = HP2Launcher::ObjectDetail::Medium;
					else if (std::strcmp(value, "high") == 0)
						result.settings.objectDetail = HP2Launcher::ObjectDetail::High;
					else if (std::strcmp(value, "veryHigh") == 0)
						result.settings.objectDetail = HP2Launcher::ObjectDetail::VeryHigh;
				}
				else if (settingName == "renderBackend")
				{
					if (std::strcmp(value, "xopengl") == 0)
						result.settings.renderBackend = HP2Launcher::RenderBackend::XOpenGL;
					else if (std::strcmp(value, "vulkan") == 0)
						result.settings.renderBackend = HP2Launcher::RenderBackend::Vulkan;
				}
				else if (settingName == "controlMode")
				{
					if (std::strcmp(value, "classic") == 0)
						result.settings.controlMode = HP2Launcher::ControlMode::Classic;
					else if (std::strcmp(value, "modern") == 0)
						result.settings.controlMode = HP2Launcher::ControlMode::Modern;
				}
				else if (settingName == "difficulty")
				{
					if (std::strcmp(value, "easy") == 0)
						result.settings.difficulty = HP2Launcher::Difficulty::Easy;
					else if (std::strcmp(value, "medium") == 0)
						result.settings.difficulty = HP2Launcher::Difficulty::Medium;
					else if (std::strcmp(value, "hard") == 0)
						result.settings.difficulty = HP2Launcher::Difficulty::Hard;
				}
			else if (settingName == "resolutionWidth")
				result.settings.resolution.width = std::atoi(value);
			else if (settingName == "resolutionHeight")
				result.settings.resolution.height = std::atoi(value);
				else if (settingName == "verticalSync")
					result.settings.verticalSync = std::strcmp(value, "true") == 0;
				else if (settingName == "renderScale")
					result.settings.renderScale = std::strtod(value, nullptr);
				else if (settingName == "uiScale")
					result.settings.uiScale = std::strtod(value, nullptr);
				else if (settingName == "frameRateLimit")
					result.settings.frameRateLimit = std::atoi(value);
				else if (settingName == "showFPS")
					result.settings.showFPS = std::strcmp(value, "true") == 0;
				else if (settingName == "nativeText")
					result.settings.nativeText = std::strcmp(value, "true") == 0;
				else if (settingName == "maintainVerticalFOV")
					result.settings.maintainVerticalFOV = std::strcmp(value, "true") == 0;
				else if (settingName == "antiAliasingSamples")
					result.settings.antiAliasingSamples = std::atoi(value);
				else if (settingName == "anisotropy")
					result.settings.anisotropy = std::atoi(value);
				else if (settingName == "brightness")
					result.settings.brightness = std::strtod(value, nullptr);
				else if (settingName == "soundEnabled")
					result.settings.soundEnabled = std::strcmp(value, "true") == 0;
				else if (settingName == "soundVolume")
					result.settings.soundVolume = std::strtod(value, nullptr);
				else if (settingName == "musicVolume")
					result.settings.musicVolume = std::strtod(value, nullptr);
				else if (settingName == "mouseSensitivity")
					result.settings.mouseSensitivity = std::strtod(value, nullptr);
				else if (settingName == "invertMouse")
					result.settings.invertMouse = std::strcmp(value, "true") == 0;
				else if (settingName == "autoCenterCamera")
					result.settings.autoCenterCamera = std::strcmp(value, "true") == 0;
				else if (settingName == "moveWhileCasting")
					result.settings.moveWhileCasting = std::strcmp(value, "true") == 0;
				else if (settingName == "autoQuaff")
					result.settings.autoQuaff = std::strcmp(value, "true") == 0;
				else if (settingName == "screenFlashes")
					result.settings.screenFlashes = std::strcmp(value, "true") == 0;
				else if (settingName == "joystickEnabled")
					result.settings.joystickEnabled = std::strcmp(value, "true") == 0;
			}
		}

		std::fclose(file);

		// Determine action from the parsed string
		if (action == "continue")
		{
			result.selection.action = HP2Launcher::LaunchAction::Continue;
			result.selection.hasSave = true;
		}
		else if (action == "newGame")
		{
			result.selection.action = HP2Launcher::LaunchAction::NewGame;
		}
		else if (action == "quit")
		{
			result.selection.action = HP2Launcher::LaunchAction::Quit;
		}
		else
		{
			result.selection.action = HP2Launcher::LaunchAction::Error;
		}

		return true;
	}

	// Ensure directory exists, create if needed
	bool EnsureDirectory(const std::string& path)
	{
		struct stat info;
		if (stat(path.c_str(), &info) == 0)
			return S_ISDIR(info.st_mode);
		return mkdir(path.c_str(), 0700) == 0;
	}
}

namespace HP2Launcher
{
	LaunchAction RunHP2ShellLauncher(
		const LauncherRequest& request,
		LauncherResult& result,
		std::string& error)
	{
		// Check for display
		if (!std::getenv("WAYLAND_DISPLAY") && !std::getenv("DISPLAY"))
		{
			error = "no Wayland or X11 display; run the game with -NOFRONTEND and an explicit map to skip the launcher";
			return LaunchAction::Error;
		}

		// Ensure Launcher directory exists
		std::string launcherDir = request.userRoot + "/Launcher";
		if (!EnsureDirectory(launcherDir))
		{
			error = "could not create launcher directory";
			return LaunchAction::Error;
		}

		std::string requestPath = launcherDir + "/request.json";
		std::string resultPath = launcherDir + "/result.conf";

		// Write request.json
		if (!WriteRequestJson(requestPath, request))
		{
			error = "could not write launcher request";
			return LaunchAction::Error;
		}

		// Remove stale result file
		::unlink(resultPath.c_str());

		// Resolve Quickshell binary
		const char* qsBin = std::getenv("HP2_QUICKSHELL_BIN");
		if (!qsBin)
			qsBin = "qs";

		// Resolve QML directory
		const char* qmlDir = std::getenv("HP2_LAUNCHER_QML");
		if (!qmlDir)
			qmlDir = HP2_LAUNCHER_QML_DIR;

		// Check if QML directory exists
		struct stat info;
		if (stat(qmlDir, &info) != 0 || !S_ISDIR(info.st_mode))
		{
			error = std::string("launcher QML not found: ") + qmlDir;
			return LaunchAction::Error;
		}

		// Build environment with request/result paths
		std::vector<const char*> envp;
	for (char** env = ::environ; *env; ++env)
		{
			envp.push_back(*env);
		}

		// Add our custom variables (simplified; in a real implementation these would
		// replace existing values if present)
		std::string requestEnv = std::string("HP2_LAUNCHER_REQUEST=") + requestPath;
		std::string resultEnv = std::string("HP2_LAUNCHER_RESULT=") + resultPath;
		std::string qmlEnv = std::string("HP2_LAUNCHER_QML=") + qmlDir;

		envp.push_back(requestEnv.c_str());
		envp.push_back(resultEnv.c_str());
		envp.push_back(qmlEnv.c_str());
		envp.push_back(nullptr);

		// Build argv for qs -p Launcher/Quickshell/shell.qml
		std::string qmlPath = std::string(qmlDir) + "/shell.qml";
		std::vector<const char*> argv = {qsBin, "-p", qmlPath.c_str(), nullptr};

		// Spawn process
		pid_t pid;
		const int SpawnStatus = posix_spawnp(&pid, qsBin, nullptr, nullptr,
			const_cast<char* const*>(argv.data()),
			const_cast<char* const*>(envp.data()));
		if (SpawnStatus != 0)
		{
			error = std::string("failed to spawn ") + qsBin + ": " + std::strerror(SpawnStatus);
			::unlink(requestPath.c_str());
			return LaunchAction::Error;
		}

		// Wait for process
		int status;
		if (waitpid(pid, &status, 0) == -1)
		{
			error = "error waiting for launcher process";
			::unlink(requestPath.c_str());
			::unlink(resultPath.c_str());
			return LaunchAction::Error;
		}

		// Check exit status
		if (!WIFEXITED(status) || WEXITSTATUS(status) != 0)
		{
			int exitCode = WIFEXITED(status) ? WEXITSTATUS(status) : -1;
			error = "the launcher exited without a decision (status " + std::to_string(exitCode) + ")";
			::unlink(requestPath.c_str());
			::unlink(resultPath.c_str());
			return LaunchAction::Error;
		}

		// Parse result.conf
		if (!ParseResultConf(resultPath, result))
		{
			error = "could not parse launcher result";
			::unlink(requestPath.c_str());
			::unlink(resultPath.c_str());
			return LaunchAction::Error;
		}

		// Clean up
		::unlink(requestPath.c_str());
		::unlink(resultPath.c_str());

		return result.selection.action;
	}
}
