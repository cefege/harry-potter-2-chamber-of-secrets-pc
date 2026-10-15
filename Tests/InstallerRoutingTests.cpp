// Installer disc routing: where each file from the game CD's InstallShield
// cabinets lands in the installed game folder. The cabinets only name their
// file groups (Component_2, Component_5, ...); the install folders live in the
// compiled setup script. Routing therefore decides by what a file is and by
// the cabinet's own directory names. A wrong route either drops a file the
// importer requires or lets a foreign-language file replace the US one.

#include "HP2ImportInternal.h"

#include <cstdio>
#include <set>
#include <string>

namespace
{
int GFailures = 0;

void Expect(const std::string& directory, const std::string& name, bool systemGroup,
	const std::set<std::string>& cutscenes, const std::string& expected)
{
	const std::string actual = HP2Import::Detail::RouteInstallerFile(directory, name, systemGroup, cutscenes);
	if (actual != expected)
	{
		std::fprintf(stderr, "FAIL: route('%s', '%s', system=%d) = '%s', expected '%s'\n",
			directory.c_str(), name.c_str(), systemGroup ? 1 : 0, actual.c_str(), expected.c_str());
		++GFailures;
	}
}
}

int main()
{
	const std::set<std::string> none;
	const std::set<std::string> cutscenes = {"00001privetintro.int"};

	// Packages by type, at the top of their group.
	Expect("", "PrivetDr.unr", false, none, "Maps/PrivetDr.unr");
	Expect("", "Adv4Greenhouse_Music.ogg", false, none, "Music/Adv4Greenhouse_Music.ogg");
	Expect("", "HP2_Master.utx", false, none, "Textures/HP2_Master.utx");
	Expect("", "Engine.u", true, none, "System/Engine.u");
	Expect("", "HpMenu.usa", false, none, "System/HpMenu.usa");
	Expect("", "Core.int", true, none, "System/Core.int");

	// The cabinet's own directories.
	Expect("help", "Splash.bmp", false, none, "Help/Splash.bmp");
	Expect("HELP", "Splash.bmp", false, none, "Help/Splash.bmp");
	Expect("Old textures", "HP_Menu.utx", false, none, "Textures/Old textures/HP_Menu.utx");
	Expect("Old_textures", "HP_Menu.utx", false, none, "Textures/Old textures/HP_Menu.utx");
	Expect("CUTSCENES", "00001PrivetIntro.int", true, none, "System/Cutscenes/00001PrivetIntro.int");
	Expect("\\CUTSCENES\\", "00001PrivetIntro.int", true, none, "System/Cutscenes/00001PrivetIntro.int");

	// A group holding copies of the cutscene scripts at its top level must not
	// spill them into System; they go where the cutscenes directory puts them.
	Expect("", "00001PrivetIntro.int", false, cutscenes, "System/Cutscenes/00001PrivetIntro.int");

	// Only the US-English dialogue bank; other languages' banks stay behind.
	Expect("", "AllDialog.USA_uax", false, none, "Sounds/AllDialog.USA_uax");
	Expect("", "AllDialog.FRE_uax", false, none, "");
	Expect("", "AllDialog.SPA_uax", false, none, "");
	Expect("", "HpMenu.fre", false, none, "");
	Expect("", "AOLDemo_HP2.spa_utx", false, none, "");

	// Each language group ships a default.ini; only the base System group's
	// file is the template (the importer sets the language itself).
	Expect("", "Default.ini", true, none, "System/Default.ini");
	Expect("", "DefUser.ini", true, none, "System/DefUser.ini");
	Expect("", "default.ini", false, none, "");

	// Executables, installer UI files and source-control debris never route.
	Expect("", "Game.exe", true, none, "");
	Expect("", "ALAudio.dll", true, none, "");
	Expect("", "SECDRV.SYS", true, none, "");
	Expect("", "vssver.scc", false, none, "");
	Expect("", "WS_FTP.LOG", false, none, "");
	Expect("", "Background.bmp", false, none, "");
	Expect("Thumbnails", "Entry.bmp", true, none, "");
	Expect("help", "Setup.exe", false, none, "");
	Expect("", "../evil.u", false, none, "");
	Expect("", "nested/evil.u", false, none, "");

	if (GFailures)
	{
		std::fprintf(stderr, "installer_routing: %d failure(s)\n", GFailures);
		return 1;
	}
	std::printf("installer_routing: all routes as expected\n");
	return 0;
}
