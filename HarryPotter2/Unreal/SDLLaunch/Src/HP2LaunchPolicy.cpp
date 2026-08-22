/*=============================================================================
	HP2LaunchPolicy.cpp: Pure command-line launcher policy.
=============================================================================*/
#include "HP2LaunchPolicy.h"

#include <cerrno>
#include <cctype>
#include <cmath>
#include <cstdlib>
#include <limits>

namespace HP2Launcher
{
namespace
{
bool EqualAsciiInsensitive(const std::string& left, const char* right)
{
	std::size_t index = 0;
	for (; index < left.size() && right[index] != '\0'; ++index)
	{
		const unsigned char a = static_cast<unsigned char>(left[index]);
		const unsigned char b = static_cast<unsigned char>(right[index]);
		if (std::tolower(a) != std::tolower(b))
			return false;
	}
	return index == left.size() && right[index] == '\0';
}

std::string OptionName(const char* argument)
{
	if (!argument || argument[0] != '-')
		return std::string();

	std::size_t first = 1;
	while (argument[first] == '-')
		++first;
	if (argument[first] == '\0')
		return std::string();

	std::size_t end = first;
	while (argument[end] != '\0' && argument[end] != '=')
		++end;
	return std::string(argument + first, end - first);
}



bool IsDataDirectoryArgument(const char* argument)
{
	if (!argument || argument[0] != '-' || argument[1] == '-')
		return false;
	static const char Prefix[] = "datadir=";
	for (std::size_t index = 0; Prefix[index] != '\0'; ++index)
	{
		if (argument[index + 1] == '\0' ||
			std::tolower(static_cast<unsigned char>(argument[index + 1])) != Prefix[index])
			return false;
	}
	return true;
}
}

bool ShouldRunNativeLauncher(int argc, char* const argv[])
{
	static const char* const BypassOptions[] = {
		"LOAD", "NOFRONTEND", "TESTTICKS", "TESTRENDEV", "TESTNATIVETEXT", "SERVER",
		"REPLAY", "RECORD", "BENCHMARK", "COMMANDLET", "MAKE", "EXEC"
	};

	for (int index = 1; index < argc; ++index)
	{
		const char* argument = argv ? argv[index] : nullptr;
		if (!argument || argument[0] == '\0')
			continue;
		if (argument[0] != '-')
			return false;

		const std::string name = OptionName(argument);
		if (IsDataDirectoryArgument(argument))
			continue;
		for (const char* bypass : BypassOptions)
		{
			if (EqualAsciiInsensitive(name, bypass))
				return false;
		}
	}
	return true;
}



bool BuildSelectedCommand(
	const LaunchSelection& selection,
	std::string& utf8Prefix,
	std::string& error)
{
	utf8Prefix.clear();
	error.clear();

	switch (selection.action)
	{
	case LaunchAction::Quit:
		return true;
	case LaunchAction::NewGame:
		utf8Prefix = "PrivetDr.unr";
		return true;
	case LaunchAction::Continue:
		if (!selection.hasSave)
		{
			error = "Continue requires a selected save.";
			return false;
		}
		if (selection.save.saveIndex < 0)
		{
			error = "The selected save index is invalid.";
			return false;
		}
		if (selection.save.usesSlotDirectory && selection.save.slot < 0)
		{
			error = "The selected save slot is invalid.";
			return false;
		}
		utf8Prefix = "Startup.unr -LOAD=" + std::to_string(selection.save.saveIndex);
		if (selection.save.usesSlotDirectory)
			utf8Prefix += " -SAVESLOT=" + std::to_string(selection.save.slot);
		return true;
	case LaunchAction::Error:
	default:
		error = "The launcher did not return a valid action.";
		return false;
	}
}
}
