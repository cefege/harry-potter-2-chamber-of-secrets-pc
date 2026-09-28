/*=============================================================================
	HP2DataImport.cpp: Portable game-data import service.
=============================================================================*/

#include "HP2DataImport.h"

#include "HP2Paths.h"

#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>
#include <spawn.h>

extern char** environ;

namespace
{

// Reads the real path of an executable found on PATH, or returns an empty
// string. Resolving once up front means the import does not depend on the
// launcher's PATH later in the round trip.
std::string FindOnPath(const char* ExecutableName)
{
	if (!ExecutableName || !ExecutableName[0])
	{
		return std::string();
	}
	if (std::strchr(ExecutableName, '/') != nullptr)
	{
		return access(ExecutableName, X_OK) == 0 ? std::string(ExecutableName) : std::string();
	}

	const char* PathEnvironment = std::getenv("PATH");
	if (!PathEnvironment)
	{
		return std::string();
	}

	const std::string Path(PathEnvironment);
	size_t Start = 0;
	while (Start <= Path.size())
	{
		const size_t End = Path.find(':', Start);
		const std::string Directory = Path.substr(
			Start,
			End == std::string::npos ? std::string::npos : End - Start);
		if (!Directory.empty())
		{
			std::string Candidate = Directory;
			if (Candidate.back() != '/')
			{
				Candidate += '/';
			}
			Candidate += ExecutableName;
			if (access(Candidate.c_str(), X_OK) == 0)
			{
				return Candidate;
			}
		}
		if (End == std::string::npos)
		{
			break;
		}
		Start = End + 1;
	}
	return std::string();
}

bool IsRegularFile(const std::string& Path)
{
	struct stat Info;
	return !Path.empty()
		&& stat(Path.c_str(), &Info) == 0
		&& S_ISREG(Info.st_mode)
		&& access(Path.c_str(), R_OK) == 0;
}

bool IsDirectory(const std::string& Path)
{
	struct stat Info;
	return !Path.empty() && stat(Path.c_str(), &Info) == 0 && S_ISDIR(Info.st_mode);
}

std::string ParentDirectory(const std::string& Path)
{
	if (Path.empty())
	{
		return std::string();
	}
	size_t Slash = Path.find_last_of('/');
	if (Slash == std::string::npos)
	{
		return std::string();
	}
	while (Slash > 1 && Path[Slash - 1] == '/')
	{
		--Slash;
	}
	if (Slash == 0)
	{
		return std::string("/");
	}
	return Path.substr(0, Slash);
}

// Absolute path of the running executable, resolved through the current
// working directory because argv[0] is frequently relative.
std::string ExecutablePath(const std::string& Invoked)
{
	if (Invoked.empty())
	{
		return std::string();
	}
	if (Invoked.find('/') == std::string::npos)
	{
		return FindOnPath(Invoked.c_str());
	}
	if (Invoked[0] == '/')
	{
		return Invoked;
	}
	char Buffer[4096];
	if (getcwd(Buffer, sizeof(Buffer)) == nullptr)
	{
		return std::string();
	}
	std::string Path(Buffer);
	if (Path.empty() || Path.back() != '/')
	{
		Path += '/';
	}
	Path += Invoked;
	return Path;
}

// Walks up from `Start` looking for a bundled prepare_retail_data.py.
std::string FindImporterScript(const std::string& Start)
{
	// Both layouts the project actually ships, relative to the executable:
	//   macOS  HarryPotter2.app/Contents/MacOS -> Contents/Resources
	//   Linux  dist/linux-arm64/bin           -> share/hp2-launcher
	// plus the source-checkout locations for an in-tree run. The Linux entries
	// matter: a dist tree moved off the build machine has no checkout above
	// it, so without them the import button would hide itself.
	static const char* const RelativeCandidates[] = {
		"../Resources/prepare_retail_data.py",
		"../share/hp2-launcher/prepare_retail_data.py",
		"../share/hp2/prepare_retail_data.py",
		"../lib/hp2/prepare_retail_data.py",
		"Build/prepare_retail_data.py",
		"../Build/prepare_retail_data.py",
	};

	std::string Current = Start;
	for (int Depth = 0; Depth < 8 && !Current.empty(); ++Depth)
	{
		for (const char* const Relative : RelativeCandidates)
		{
			std::string Candidate = Current;
			if (Candidate.empty() || Candidate.back() != '/')
			{
				Candidate += '/';
			}
			Candidate += Relative;
			if (IsRegularFile(Candidate))
			{
				return Candidate;
			}
		}
		const std::string Parent = ParentDirectory(Current);
		if (Parent.empty() || Parent == Current)
		{
			break;
		}
		Current = Parent;
	}
	return std::string();
}


}

namespace HP2DataImport
{

Toolchain ResolveToolchain(const std::string& ExecutablePathArgument)
{
	Toolchain Result;
	Result.python = FindOnPath("python3");

	std::string Start = ExecutablePath(ExecutablePathArgument);
	if (Start.empty())
	{
		char Buffer[4096];
		if (getcwd(Buffer, sizeof(Buffer)) != nullptr)
		{
			Start = Buffer;
		}
	}
	Result.importerScript = FindImporterScript(Start);
	return Result;
}

std::string UnavailableReason(const Toolchain& Tool)
{
	if (Tool.importerScript.empty())
	{
		return "The bundled game data importer is missing from this installation. "
			"Reinstall the app, or run Build/prepare_retail_data.py from a source checkout.";
	}
	if (Tool.python.empty())
	{
#if MACOSX
		return "This Mac has no Python 3 installed. Open Terminal, run "
			"\"xcode-select --install\", wait for it to finish, then import again.";
#else
		return "This system has no Python 3 installed. Install it with your package "
			"manager (for example \"sudo apt install python3\" or \"sudo pacman -S python\") "
			"and import again.";
#endif
	}
	return std::string();
}

bool GetConventionalRetailDestination(std::string& Destination)
{
	return GetHP2ConventionalRetailDataRoot(Destination);
}

bool RunRetailImport(
	const Toolchain& Tool,
	const std::string& SourcePath,
	const std::string& Destination,
	std::string& Output,
	std::string& Error)
{
	Output.clear();
	Error.clear();

	const std::string Unavailable = UnavailableReason(Tool);
	if (!Unavailable.empty())
	{
		Error = Unavailable;
		return false;
	}
	if (SourcePath.empty() || !IsDirectory(SourcePath) && !IsRegularFile(SourcePath))
	{
		Error = "Choose the folder your game was installed into, or an archive of it.";
		return false;
	}

#if defined(__EMSCRIPTEN__)
	// A browser page cannot spawn a host process, and Emscripten does not
	// provide posix_spawnp at all. Retail import is a native workflow: the
	// browser build imports through the page's own folder import instead, so
	// report the same failure the native path reports when the spawn call
	// itself fails, and the caller's existing error branch handles it.
	Error = std::string("Could not start the importer: ") + std::strerror(ENOSYS);
	return false;
#else

	const std::string LogPath = Destination + ".import.log";
	// The importer prints a progress summary and can take minutes on a cold
	// copy; capture it to a log so a long blocking run still leaves a record
	// the launcher can show, and so the process is never attached to a pipe
	// that nobody drains.
	FILE* Log = std::fopen(LogPath.c_str(), "w");
	if (!Log)
	{
		Error = "Could not write the import log next to the game data folder.";
		return false;
	}

	const bool SourceIsDirectory = IsDirectory(SourcePath);
	std::string SourceArgument = SourceIsDirectory ? "--retail-root" : "--archive";

	std::vector<std::string> Arguments;
	Arguments.push_back(Tool.python);
	Arguments.push_back(Tool.importerScript);
	Arguments.push_back(SourceArgument);
	Arguments.push_back(SourcePath);
	Arguments.push_back("--output");
	Arguments.push_back(Destination);
	Arguments.push_back("--profile");
	Arguments.push_back("retail-only");
	Arguments.push_back("--link-mode");
	Arguments.push_back("copy");

	std::vector<char*> Argv;
	Argv.reserve(Arguments.size() + 1);
	for (std::string& Argument : Arguments)
	{
		Argv.push_back(const_cast<char*>(Argument.c_str()));
	}
	Argv.push_back(nullptr);

	pid_t Pid = -1;
	const int SpawnStatus = posix_spawnp(
		&Pid,
		Tool.python.c_str(),
		nullptr,
		nullptr,
		Argv.data(),
		environ);
	if (SpawnStatus != 0)
	{
		std::fclose(Log);
		Error = std::string("Could not start the importer: ") + std::strerror(SpawnStatus);
		return false;
	}

	int Status = 0;
	while (waitpid(Pid, &Status, 0) == -1)
	{
		// Only a genuine wait failure ends this; EINTR is retried.
	}
	const bool Succeeded = WIFEXITED(Status) && WEXITSTATUS(Status) == 0;
	std::fclose(Log);

	Output = LogPath;
	if (!Succeeded)
	{
		Error = WIFEXITED(Status)
			? "The import did not finish. See the log above for what was missing."
			: "The importer stopped unexpectedly. See the log above for details.";
		return false;
	}
	return true;
#endif
}

}
