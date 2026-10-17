/*=============================================================================
	HP2DataImport.cpp: In-process retail game-data import service.
=============================================================================*/

#include "HP2DataImport.h"

#include "HP2Paths.h"

#include <filesystem>
#include <fstream>

#ifndef __EMSCRIPTEN__
#include "hp2_rs.h"
#endif

namespace HP2DataImport
{

bool GetConventionalRetailDestination(std::string& Destination)
{
	return GetHP2ConventionalRetailDataRoot(Destination);
}

namespace
{
#ifndef __EMSCRIPTEN__
void CaptureSink(void* Ctx, const uint8_t* Text, size_t Len)
{
	static_cast<std::string*>(Ctx)->assign(reinterpret_cast<const char*>(Text), Len);
}
#endif
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
#ifdef __EMSCRIPTEN__
	// The web build has no Rust at all -- see HP2Targets.cmake's
	// hp2_import_core for the full reasoning (no stable Rust wasm64
	// Emscripten target exists). This stays on the real C++ importer
	// until that changes.
	const bool Succeeded = HP2Import::RunImport(Options, Progress, Output, Error);
#else
	std::string Result;
	const bool Succeeded = hp2_rs_import_run(
		reinterpret_cast<const uint8_t*>(Options.retailRoot.data()), Options.retailRoot.size(),
		reinterpret_cast<const uint8_t*>(Options.archive.data()), Options.archive.size(),
		reinterpret_cast<const uint8_t*>(Options.prototypeRoot.data()), Options.prototypeRoot.size(),
		reinterpret_cast<const uint8_t*>(Options.output.data()), Options.output.size(),
		static_cast<uint32_t>(Options.profile),
		static_cast<uint32_t>(Options.linkMode),
		Options.checkOnly,
		Progress,
		&CaptureSink,
		&Result);
	if (Succeeded)
		Output = std::move(Result);
	else
		Error = std::move(Result);
#endif
	Log.write(Output.data(), static_cast<std::streamsize>(Output.size()));
	if (!Log)
	{
		Error = "Could not write the import log next to the game data folder.";
		return false;
	}
	return Succeeded;
}

}
