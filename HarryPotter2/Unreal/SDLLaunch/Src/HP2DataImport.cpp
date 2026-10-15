/*=============================================================================
	HP2DataImport.cpp: In-process retail game-data import service.
=============================================================================*/

#include "HP2DataImport.h"

#include "HP2Paths.h"

#include <filesystem>
#include <fstream>

namespace HP2DataImport
{

bool GetConventionalRetailDestination(std::string& Destination)
{
	return GetHP2ConventionalRetailDataRoot(Destination);
}

bool RunRetailImport(
	const std::string& SourcePath,
	const std::string& Destination,
	HP2Import::Progress* Progress,
	std::string& Output,
	std::string& Error)
{
	Output.clear();
	Error.clear();
	HP2Import::Options Options;
	Options.output = Destination;
	Options.profile = HP2Import::Profile::RetailOnly;
	Options.linkMode = HP2Import::LinkMode::Copy;
#if defined(__EMSCRIPTEN__)
	// The browser's source is its own scratch copy (/hp2import/source); an
	// unpacked installer disc frees its cabinets right away to bound memory.
	Options.consumeInstallerSource = true;
#endif

	const bool IsDirectory = std::filesystem::is_directory(std::filesystem::u8path(SourcePath));
	if (IsDirectory)
		Options.retailRoot = SourcePath;
	else
		Options.archive = SourcePath;

	const std::filesystem::path LogPath = std::filesystem::u8path(Destination + ".import.log");
	std::error_code Ec;
	if (!LogPath.parent_path().empty())
		std::filesystem::create_directories(LogPath.parent_path(), Ec);
	if (Ec)
	{
		Error = "Could not write the import log next to the game data folder.";
		return false;
	}
	std::ofstream Log(LogPath, std::ios::binary | std::ios::trunc);
	if (!Log)
	{
		Error = "Could not write the import log next to the game data folder.";
		return false;
	}
	const bool Succeeded = HP2Import::RunImport(Options, Progress, Output, Error);
	Log.write(Output.data(), static_cast<std::streamsize>(Output.size()));
	if (!Log)
	{
		Error = "Could not write the import log next to the game data folder.";
		return false;
	}
	return Succeeded;
}

}
