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

namespace
{
const char* SelectionActionName(LaunchAction action)
{
	switch (action)
	{
	case LaunchAction::Continue: return "Continue";
	case LaunchAction::NewGame: return "NewGame";
	case LaunchAction::Quit: return "Quit";
	}
	return nullptr;
}

bool ParseSelectionIndex(const std::string& text, int& value)
{
	if (text.empty()) return false;
	long long result = 0;
	for (char c : text)
	{
		if (c < '0' || c > '9') return false;
		result = result * 10 + (c - '0');
		if (result > std::numeric_limits<int>::max()) return false;
	}
	value = static_cast<int>(result);
	return true;
}
}

bool SerializeLaunchSelection(
	const LaunchSelection& selection,
	std::vector<LaunchSelectionField>& fields,
	std::string& error)
{
	fields.clear();
	error.clear();
	const char* name = SelectionActionName(selection.action);
	if (!name)
	{
		error = "The launch selection contains an invalid action.";
		return false;
	}
	// Validate completely before emitting rows so a rejected selection never
	// leaves a partial field list behind.
	if (selection.action == LaunchAction::Continue)
	{
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
	}
	fields.push_back({"Action", name});
	if (selection.action != LaunchAction::Continue)
		return true;
	fields.push_back({"HasSave", "True"});
	fields.push_back({"SaveIndex", std::to_string(selection.save.saveIndex)});
	if (selection.save.usesSlotDirectory)
		fields.push_back({"SaveSlot", std::to_string(selection.save.slot)});
	return true;
}

bool DeserializeLaunchSelection(
	const std::vector<LaunchSelectionField>& fields,
	LaunchSelection& selection,
	std::string& error)
{
	// Parse into a local so a malformed store never leaves a partially
	// populated selection behind: the Quit fallback is committed only on
	// success, matching the whole-selection fallback contract.
	selection = LaunchSelection();
	LaunchSelection parsedSelection;
	error.clear();

	std::string action, hasSave, saveIndex, saveSlot;
	bool hasAction = false, hasHasSave = false, hasSaveIndex = false, hasSaveSlot = false;
	for (const LaunchSelectionField& field : fields)
	{
		if (EqualAsciiInsensitive(field.key, "Action")) { action = field.value; hasAction = true; }
		else if (EqualAsciiInsensitive(field.key, "HasSave")) { hasSave = field.value; hasHasSave = true; }
		else if (EqualAsciiInsensitive(field.key, "SaveIndex")) { saveIndex = field.value; hasSaveIndex = true; }
		else if (EqualAsciiInsensitive(field.key, "SaveSlot")) { saveSlot = field.value; hasSaveSlot = true; }
	}
	if (!hasAction)
	{
		error = "No persisted launch action was found.";
		return false;
	}

	LaunchAction parsed = LaunchAction::Quit;
	if (EqualAsciiInsensitive(action, "Continue")) parsed = LaunchAction::Continue;
	else if (EqualAsciiInsensitive(action, "NewGame")) parsed = LaunchAction::NewGame;
	else if (!EqualAsciiInsensitive(action, "Quit"))
	{
		error = "The persisted launch action is unknown: '" + action + "'.";
		return false;
	}
	parsedSelection.action = parsed;
	if (parsed != LaunchAction::Continue)
	{
		selection = parsedSelection;
		return true;
	}

	if (!hasHasSave || !EqualAsciiInsensitive(hasSave, "True"))
	{
		error = "A persisted Continue selection is missing its save marker.";
		return false;
	}
	parsedSelection.hasSave = true;
	if (!hasSaveIndex || !ParseSelectionIndex(saveIndex, parsedSelection.save.saveIndex))
	{
		error = "A persisted Continue selection has an invalid save index.";
		return false;
	}
	if (hasSaveSlot)
	{
		parsedSelection.save.usesSlotDirectory = true;
		if (!ParseSelectionIndex(saveSlot, parsedSelection.save.slot))
		{
			error = "A persisted Continue selection has an invalid save slot.";
			return false;
		}
	}
	selection = parsedSelection;
	return true;
}
}
