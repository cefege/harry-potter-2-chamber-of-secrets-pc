/*=============================================================================
	HP2Paths.cpp: Pre-appInit data and user path bootstrap.
=============================================================================*/

#include "Core.h"
#include "HP2Paths.h"

#include <cerrno>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <sys/stat.h>
#include <unistd.h>

namespace
{
constexpr const char* DataDirPrefix = "-datadir=";
constexpr const char* ExternalDataSuffix =
	"Library/Application Support/Harry Potter 2/Data/Unreal";
constexpr const char* ConventionalRetailDataSuffix =
	"Library/Application Support/Harry Potter 2/Data/Retail";
constexpr const char* ConventionalPrototypeDataSuffix =
	"Library/Application Support/Harry Potter 2/Data/Prototype";
constexpr const char* UserSuffix =
	"Library/Application Support/Harry Potter 2/User";

bool EqualsAsciiNoCase(char A, char B)
{
	if (A >= 'A' && A <= 'Z')
		A = static_cast<char>(A - 'A' + 'a');
	if (B >= 'A' && B <= 'Z')
		B = static_cast<char>(B - 'A' + 'a');
	return A == B;
}

const char* DataDirArgument(const char* Argument)
{
	if (!Argument)
		return nullptr;

	const size_t PrefixLength = std::strlen(DataDirPrefix);
	for (size_t Index = 0; Index < PrefixLength; ++Index)
		if (!Argument[Index] || !EqualsAsciiNoCase(Argument[Index], DataDirPrefix[Index]))
			return nullptr;
	return Argument + PrefixLength;
}

char* JoinPath(const char* Left, const char* Right)
{
	if (!Left || !Right)
		return nullptr;
	const size_t LeftLength = std::strlen(Left);
	const size_t RightLength = std::strlen(Right);
	const bool NeedSlash = LeftLength > 0 && Left[LeftLength - 1] != '/';
	const size_t SeparatorLength = NeedSlash ? 1 : 0;
	if (LeftLength > static_cast<size_t>(-1) - RightLength - SeparatorLength - 1)
		return nullptr;

	char* Result = static_cast<char*>(std::malloc(
		LeftLength + SeparatorLength + RightLength + 1));
	if (!Result)
		return nullptr;
	std::memcpy(Result, Left, LeftLength);
	size_t Position = LeftLength;
	if (NeedSlash)
		Result[Position++] = '/';
	std::memcpy(Result + Position, Right, RightLength + 1);
	return Result;
}

char* CanonicalizeExistingDirectory(const char* Directory);

bool GetConventionalDataRoot(
	const char* Suffix,
	std::string& AbsoluteRoot)
{
	AbsoluteRoot.clear();
	char* Home = CanonicalizeExistingDirectory(std::getenv("HOME"));
	if (!Home)
		return false;

	char* Root = JoinPath(Home, Suffix);
	std::free(Home);
	if (!Root)
		return false;
	AbsoluteRoot = Root;
	std::free(Root);
	return true;
}

bool IsReadableFile(const char* Filename)
{
	struct stat Info;
	return Filename
		&& stat(Filename, &Info) == 0
		&& S_ISREG(Info.st_mode)
		&& access(Filename, R_OK) == 0;
}

bool IsDirectory(const char* Directory)
{
	struct stat Info;
	return Directory && stat(Directory, &Info) == 0 && S_ISDIR(Info.st_mode);
}

bool IsWritableDirectory(const char* Directory)
{
	return IsDirectory(Directory) && access(Directory, W_OK | X_OK) == 0;
}

char* CanonicalizeExistingDirectory(const char* Directory)
{
	char* Canonical = Directory ? realpath(Directory, nullptr) : nullptr;
	if (Canonical && !IsDirectory(Canonical))
	{
		std::free(Canonical);
		Canonical = nullptr;
	}
	return Canonical;
}

bool IsSameOrDescendant(const char* Path, const char* Parent)
{
	const size_t ParentLength = std::strlen(Parent);
	return std::strcmp(Path, Parent) == 0
		|| (std::strncmp(Path, Parent, ParentLength) == 0
			&& Path[ParentLength]
			&& (ParentLength == 1 || Path[ParentLength] == '/'));
}

char* ValidateDataRoot(const char* Candidate)
{
	char* CanonicalRoot = CanonicalizeExistingDirectory(Candidate);
	if (!CanonicalRoot)
		return nullptr;

	char* DefaultIni = JoinPath(CanonicalRoot, "System/Default.ini");
	const bool Valid = IsReadableFile(DefaultIni);
	std::free(DefaultIni);
	if (!Valid)
	{
		std::free(CanonicalRoot);
		return nullptr;
	}
	return CanonicalRoot;
}

char* DiscoverDevelopmentDataRoot()
{
	char* Current = getcwd(nullptr, 0);
	if (!Current)
		return nullptr;

	for (;;)
	{
		static const char* CandidateSuffixes[] =
		{
			"out/retail-data",
			"HarryPotter2/Unreal"
		};
		for( size_t CandidateIndex=0; CandidateIndex<sizeof(CandidateSuffixes)/sizeof(CandidateSuffixes[0]); ++CandidateIndex )
		{
			char* Candidate = JoinPath(Current, CandidateSuffixes[CandidateIndex]);
			char* CanonicalRoot = ValidateDataRoot(Candidate);
			std::free(Candidate);
			if( CanonicalRoot )
			{
				std::free(Current);
				return CanonicalRoot;
			}
		}
		if (std::strcmp(Current, "/") == 0)
			break;

		char* Slash = std::strrchr(Current, '/');
		if (!Slash || Slash == Current)
			Current[1] = '\0';
		else
			*Slash = '\0';
	}
	std::free(Current);
	return nullptr;
}

char* DiscoverPrototypeDataRoot()
{
	char* Current = getcwd(nullptr, 0);
	if (!Current)
		return nullptr;

	for (;;)
	{
		char* Candidate = JoinPath(Current, "HarryPotter2/Unreal");
		char* CanonicalRoot = ValidateDataRoot(Candidate);
		std::free(Candidate);
		if (CanonicalRoot)
		{
			std::free(Current);
			return CanonicalRoot;
		}
		if (std::strcmp(Current, "/") == 0)
			break;

		char* Slash = std::strrchr(Current, '/');
		if (!Slash || Slash == Current)
			Current[1] = '\0';
		else
			*Slash = '\0';
	}
	std::free(Current);
	return nullptr;
}

bool EnsureDirectory(const char* Directory, mode_t Mode)
{
	if (Directory && mkdir(Directory, Mode) == 0)
		return true;
	return Directory && errno == EEXIST && IsDirectory(Directory);
}

bool EnsureDirectoryTree(const char* Directory, mode_t Mode)
{
	if (!Directory || Directory[0] != '/')
		return false;

	const size_t Length = std::strlen(Directory);
	char* Partial = static_cast<char*>(std::malloc(Length + 1));
	if (!Partial)
		return false;
	std::memcpy(Partial, Directory, Length + 1);

	bool Success = true;
	for (size_t Index = 1; Index <= Length; ++Index)
	{
		if (Partial[Index] != '/' && Partial[Index] != '\0')
			continue;
		const char Saved = Partial[Index];
		Partial[Index] = '\0';
		if (Index > 1 && !EnsureDirectory(Partial, Mode))
			Success = false;
		Partial[Index] = Saved;
		if (!Success)
			break;
	}
	std::free(Partial);
	return Success;
}

bool EnsureWritableChildDirectory(const char* Parent, const char* Child, mode_t Mode)
{
	char* Directory = JoinPath(Parent, Child);
	const bool Success = EnsureDirectory(Directory, Mode)
		&& IsWritableDirectory(Directory);
	std::free(Directory);
	return Success;
}

char* PrepareLauncherRoot(const char* Home)
{
	char* ApplicationSupport = JoinPath(
		Home, "Library/Application Support/Harry Potter 2");
	char* UserRoot = JoinPath(Home, UserSuffix);
	if (!ApplicationSupport || !UserRoot
		|| !EnsureDirectoryTree(ApplicationSupport, 0755)
		|| !EnsureDirectory(UserRoot, 0700))
	{
		std::free(ApplicationSupport);
		std::free(UserRoot);
		return nullptr;
	}
	std::free(ApplicationSupport);

	char* CanonicalUserRoot = CanonicalizeExistingDirectory(UserRoot);
	std::free(UserRoot);
	if (!CanonicalUserRoot || !IsWritableDirectory(CanonicalUserRoot))
	{
		std::free(CanonicalUserRoot);
		return nullptr;
	}
	return CanonicalUserRoot;
}

char* PrepareActiveUserRoot(const char* UserRoot)
{
	if (!UserRoot || UserRoot[0] != '/'
		|| !EnsureDirectoryTree(UserRoot, 0700))
		return nullptr;

	char* CanonicalUserRoot = CanonicalizeExistingDirectory(UserRoot);
	if (!CanonicalUserRoot || !IsWritableDirectory(CanonicalUserRoot))
	{
		std::free(CanonicalUserRoot);
		return nullptr;
	}

	const bool Success = EnsureWritableChildDirectory(CanonicalUserRoot, "Save", 0700)
		&& EnsureWritableChildDirectory(CanonicalUserRoot, "Save/cache", 0700)
		&& EnsureWritableChildDirectory(CanonicalUserRoot, "Cache", 0700);
	if (!Success)
	{
		std::free(CanonicalUserRoot);
		return nullptr;
	}
	return CanonicalUserRoot;
}

char* PrepareUserRoot(const char* Home)
{
	char* LauncherRoot = PrepareLauncherRoot(Home);
	if (!LauncherRoot)
		return nullptr;
	char* UserRoot = PrepareActiveUserRoot(LauncherRoot);
	std::free(LauncherRoot);
	return UserRoot;
}

bool Utf8ToTchar(const char* Input, TCHAR*& Output)
{
	static_assert(sizeof(TCHAR) == 4, "Native macOS paths require 32-bit host TCHAR");
	Output = nullptr;
	if (!Input)
		return false;

	const size_t InputLength = std::strlen(Input);
	if (InputLength >= static_cast<size_t>(MAXINT)
		|| InputLength > static_cast<size_t>(-1) / sizeof(TCHAR) - 1)
		return false;
	const INT Capacity = static_cast<INT>(InputLength + 1);
	TCHAR* Buffer = static_cast<TCHAR*>(
		std::malloc(static_cast<size_t>(Capacity) * sizeof(TCHAR)));
	if (!Buffer)
		return false;
	if (!appFromUtf8InPlace(Buffer, Input, Capacity))
	{
		std::free(Buffer);
		return false;
	}
	Output = Buffer;
	return true;
}
}
const char* HP2DataDirectoryArgumentValue(const char* Argument)
{
	return DataDirArgument(Argument);
}

bool IsHP2DataDirectoryArgument(const char* Argument)
{
	return HP2DataDirectoryArgumentValue(Argument) != nullptr;
}

bool GetHP2ConventionalRetailDataRoot(std::string& AbsoluteRoot)
{
	return GetConventionalDataRoot(ConventionalRetailDataSuffix, AbsoluteRoot);
}

bool GetHP2ConventionalPrototypeDataRoot(std::string& AbsoluteRoot)
{
	return GetConventionalDataRoot(ConventionalPrototypeDataSuffix, AbsoluteRoot);
}

bool DiscoverHP2RetailDataRoot(std::string& CanonicalRoot)
{
	CanonicalRoot.clear();
	char* Home = CanonicalizeExistingDirectory(std::getenv("HOME"));
	if (!Home)
		return false;
	char* Candidate = JoinPath(Home, ExternalDataSuffix);
	char* Root = ValidateDataRoot(Candidate);
	std::free(Candidate);
	std::free(Home);
	if (!Root)
		return false;
	CanonicalRoot = Root;
	std::free(Root);
	return true;
}

bool DiscoverHP2PrototypeDataRoot(std::string& CanonicalRoot)
{
	CanonicalRoot.clear();
	char* Root = DiscoverPrototypeDataRoot();
	if (!Root)
		return false;
	CanonicalRoot = Root;
	std::free(Root);
	return true;
}

bool PrepareHP2LauncherHome(std::string& LauncherRoot, std::string& Error)
{
	LauncherRoot.clear();
	Error.clear();
	char* Home = CanonicalizeExistingDirectory(std::getenv("HOME"));
	if (!Home)
	{
		Error = "HOME does not name an existing directory";
		return false;
	}

	char* Root = PrepareLauncherRoot(Home);
	std::free(Home);
	if (!Root)
	{
		Error = "could not create writable launcher directory";
		return false;
	}
	LauncherRoot = Root;
	std::free(Root);
	return true;
}

bool ValidateHP2DataRoot(
	const std::string& Candidate,
	std::string& CanonicalRoot,
	std::string& Error)
{
	CanonicalRoot.clear();
	Error.clear();
	char* Root = ValidateDataRoot(Candidate.c_str());
	if (!Root)
	{
		Error = "expected a readable System/Default.ini";
		return false;
	}
	CanonicalRoot = Root;
	std::free(Root);
	return true;
}

bool InstallHP2Paths(
	const std::string& DataRoot,
	const std::string& UserRoot,
	std::string& Error)
{
	Error.clear();
	std::string CanonicalDataRoot;
	if (!ValidateHP2DataRoot(DataRoot, CanonicalDataRoot, Error))
		return false;
	if (UserRoot.empty() || UserRoot[0] != '/')
	{
		Error = "user directory must be an absolute path";
		return false;
	}

	char* CanonicalUserRoot = PrepareActiveUserRoot(UserRoot.c_str());
	if (!CanonicalUserRoot)
	{
		Error = "could not create writable user directory";
		return false;
	}
	if (IsSameOrDescendant(CanonicalDataRoot.c_str(), CanonicalUserRoot)
		|| IsSameOrDescendant(CanonicalUserRoot, CanonicalDataRoot.c_str()))
	{
		Error = "data and user directories must be separate";
		std::free(CanonicalUserRoot);
		return false;
	}

	char* SystemDirectory = JoinPath(CanonicalDataRoot.c_str(), "System");
	char* CanonicalSystem = CanonicalizeExistingDirectory(SystemDirectory);
	std::free(SystemDirectory);
	if (!CanonicalSystem)
	{
		Error = "could not canonicalize the validated System directory";
		std::free(CanonicalUserRoot);
		return false;
	}

	char* BaseDirectory = JoinPath(CanonicalSystem, "");
	char* UserDirectory = JoinPath(CanonicalUserRoot, "");
	std::free(CanonicalSystem);
	std::free(CanonicalUserRoot);
	TCHAR* BaseTchar = nullptr;
	TCHAR* UserTchar = nullptr;
	if (!BaseDirectory || !UserDirectory
		|| !Utf8ToTchar(BaseDirectory, BaseTchar)
		|| !Utf8ToTchar(UserDirectory, UserTchar))
	{
		Error = "filesystem paths are not valid UTF-8";
		std::free(BaseTchar);
		std::free(UserTchar);
		std::free(BaseDirectory);
		std::free(UserDirectory);
		return false;
	}
	std::free(BaseDirectory);
	std::free(UserDirectory);
	appSetBaseDir(BaseTchar);
	appSetUserDir(UserTchar);
	std::free(BaseTchar);
	std::free(UserTchar);
	return true;
}


bool PrepareHP2Paths(int ArgC, char* const ArgV[])
{
	const char* ExplicitDataDir = nullptr;
	for (int Index = 1; Index < ArgC; ++Index)
	{
		const char* Value = DataDirArgument(ArgV[Index]);
		if (Value)
		{
			ExplicitDataDir = Value;
			break;
		}
	}

	char* DataRoot = nullptr;
	if (ExplicitDataDir)
	{
		DataRoot = ValidateDataRoot(ExplicitDataDir);
		if (!DataRoot)
		{
			std::fprintf(stderr,
				"hp2: invalid -datadir '%s': expected a readable System/Default.ini\n",
				ExplicitDataDir);
			return false;
		}
	}

	const char* HomeEnvironment = std::getenv("HOME");
	char* Home = CanonicalizeExistingDirectory(HomeEnvironment);

	if (!ExplicitDataDir)
	{
		DataRoot = DiscoverDevelopmentDataRoot();
		if (!DataRoot && Home)
		{
			char* ExternalDataRoot = JoinPath(Home, ExternalDataSuffix);
			DataRoot = ValidateDataRoot(ExternalDataRoot);
			std::free(ExternalDataRoot);
		}
		if (!DataRoot)
		{
			char* ExpectedRoot = Home ? JoinPath(Home, ExternalDataSuffix) : nullptr;
			std::fprintf(stderr,
				"hp2: unable to locate game data; expected HarryPotter2/Unreal/System/Default.ini from the working directory or %s/System/Default.ini\n",
				ExpectedRoot ? ExpectedRoot : "$HOME/Library/Application Support/Harry Potter 2/Data/Unreal");
			std::free(ExpectedRoot);
			std::free(Home);
			return false;
		}
	}

	if (!Home)
	{
		std::fprintf(stderr, "hp2: HOME does not name an existing directory\n");
		std::free(DataRoot);
		return false;
	}

	char* UserRoot = PrepareUserRoot(Home);
	if (!UserRoot)
	{
		char* ExpectedUserRoot = JoinPath(Home, UserSuffix);
		std::fprintf(stderr, "hp2: could not create writable user directory '%s'\n",
			ExpectedUserRoot ? ExpectedUserRoot : UserSuffix);
		std::free(ExpectedUserRoot);
		std::free(DataRoot);
		std::free(Home);
		return false;
	}
	std::free(Home);

	if (IsSameOrDescendant(DataRoot, UserRoot)
		|| IsSameOrDescendant(UserRoot, DataRoot))
	{
		std::fprintf(stderr, "hp2: data and user directories must be separate\n");
		std::free(UserRoot);
		std::free(DataRoot);
		return false;
	}

	char* SystemDirectory = JoinPath(DataRoot, "System");
	char* CanonicalSystem = CanonicalizeExistingDirectory(SystemDirectory);
	std::free(SystemDirectory);
	std::free(DataRoot);
	if (!CanonicalSystem)
	{
		std::fprintf(stderr, "hp2: could not canonicalize the validated System directory\n");
		std::free(UserRoot);
		return false;
	}

	char* BaseDirectory = JoinPath(CanonicalSystem, "");
	char* UserDirectory = JoinPath(UserRoot, "");
	std::free(CanonicalSystem);
	std::free(UserRoot);
	TCHAR* BaseTchar = nullptr;
	TCHAR* UserTchar = nullptr;
	if (!BaseDirectory || !UserDirectory
		|| !Utf8ToTchar(BaseDirectory, BaseTchar)
		|| !Utf8ToTchar(UserDirectory, UserTchar))
	{
		std::fprintf(stderr, "hp2: filesystem paths are not valid UTF-8\n");
		std::free(BaseTchar);
		std::free(UserTchar);
		std::free(BaseDirectory);
		std::free(UserDirectory);
		return false;
	}
	std::free(BaseDirectory);
	std::free(UserDirectory);

	appSetBaseDir(BaseTchar);
	appSetUserDir(UserTchar);
	std::free(BaseTchar);
	std::free(UserTchar);
	return true;
}
