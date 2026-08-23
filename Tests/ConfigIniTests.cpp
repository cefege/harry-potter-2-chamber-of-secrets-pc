/*=============================================================================
	ConfigIniTests.cpp: FConfigCacheIni/FConfigFile behavior contracts.

	Freezes the CURRENT ini parse/write semantics (HarryPotter2/Unreal/
	Core/Inc/FConfigCacheIni.h, header-only) so future launcher config
	migrations can rely on them. Every quirk below asserts the ACTUAL
	behavior with a literal expected value and an explanatory comment.

	Link note: unlike EaxaTests (which compiles only the decoder unit), this
	test links the full hp2_core object library. The config implementation is
	header-only but its behavior is defined by CORE_API helpers that live in
	UnAnsi.cpp/UnMisc.cpp (appAtoi/appAtof/appStrcmp/appStricmp/appSprintf/
	appLoadFileToString/appSaveStringToFile and the UTF-16 codecs); a
	minimal-TU link would require hand-written stubs and would freeze the
	stubs instead of the shipping behavior.
=============================================================================*/

#include "Core.h"
#include "FConfigCacheIni.h"
#include "FMallocAnsi.h"
#include "FFileManagerUnix.h"

#include <cstdarg>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <fstream>
#include <iterator>
#include <string>

// GPackage is per-executable (declared extern in Core.h, defined by each
// program, see AbiTests.cpp).
extern "C" { TCHAR GPackage[64] = TEXT("ConfigIniTests"); }

namespace
{
	// Installed into GMalloc as the first statement of main(), before any
	// engine allocation happens (global operator new routes to GMalloc).
	FMallocAnsi GTestAllocator;
	FFileManagerUnix GTestFileManager;

	int GFailures = 0;
	const char* GTestName = "config_ini";
	std::string GArtifactDir; // assigned in main(), never at static-init time

	void vFailf( const char* Fmt, va_list Args )
	{
		std::fprintf( stderr, "%s: FAIL: ", GTestName );
		std::vfprintf( stderr, Fmt, Args );
		std::fputc( '\n', stderr );
		++GFailures;
	}

	void Failf( const char* Fmt, ... )
	{
		va_list Args;
		va_start( Args, Fmt );
		vFailf( Fmt, Args );
		va_end( Args );
	}

	void Require( bool Condition, const char* Fmt, ... )
	{
		if( Condition )
			return;
		va_list Args;
		va_start( Args, Fmt );
		vFailf( Fmt, Args );
		va_end( Args );
	}

	// Case-sensitive exact comparison (FString operator== is case-INSENSITIVE,
	// so pinning literal casing must go through appStrcmp).
	bool Exact( const FString& S, const TCHAR* Expected )
	{
		return appStrcmp( *S, Expected ) == 0;
	}

	std::string ArtifactPath( const char* Name )
	{
		return GArtifactDir + "/" + Name;
	}

	// Fixture writer: raw bytes, binary mode, so the code under test sees
	// exactly the line-ending mix we pin. Fixtures live only under
	// HP2_ARTIFACT_DIR.
	void WriteBytes( const char* Name, const std::string& Bytes )
	{
		std::ofstream Out( ArtifactPath( Name ).c_str(), std::ios::binary | std::ios::trunc );
		if( !Out )
		{
			Failf( "cannot create fixture %s", Name );
			return;
		}
		Out.write( Bytes.data(), (std::streamsize)Bytes.size() );
		if( !Out.good() )
			Failf( "short write on fixture %s", Name );
	}

	std::string ReadBytes( const char* Name )
	{
		std::ifstream In( ArtifactPath( Name ).c_str(), std::ios::binary );
		if( !In )
			return std::string();
		return std::string( (std::istreambuf_iterator<char>(In)), std::istreambuf_iterator<char>() );
	}

	bool FileExists( const char* Name )
	{
		std::ifstream In( ArtifactPath( Name ).c_str(), std::ios::binary );
		return (bool)In;
	}

	FString PathAsFString( const char* Name )
	{
		const std::string P = ArtifactPath( Name );
		return FString( appFromAnsi( P.c_str() ) );
	}

	// FConfigFile is only a container; read one value the way the engine's
	// own accessors do (both lookups are FString operator== based, i.e.
	// case-INSENSITIVE).
	bool SectionValue( FConfigFile& File, const TCHAR* Section, const TCHAR* Key, FString& Out )
	{
		FConfigSection* Sec = File.Find( Section );
		if( !Sec )
			return 0;
		FString* Value = Sec->Find( Key );
		if( !Value )
			return 0;
		Out = *Value;
		return 1;
	}

	INT CountValues( FConfigFile& File, const TCHAR* Section, const TCHAR* Key )
	{
		FConfigSection* Sec = File.Find( Section );
		if( !Sec )
			return 0;
		TArray<FString> Values;
		Sec->MultiFind( Key, Values );
		return Values.Num();
	}

	/*---------------------------------------------------------------------
		1. Parse semantics (FConfigFile::Read).
	---------------------------------------------------------------------*/

	struct ParseCase
	{
		const char* Name;
		const char* IniText;      // raw bytes of the fixture
		const TCHAR* Section;
		const TCHAR* Key;
		bool ExpectFound;
		const TCHAR* ExpectValue; // meaningful only when ExpectFound
	};

	void TestParseSemantics()
	{
		static const ParseCase Cases[] =
		{
			// Plain assignment with CRLF.
			{ "plain", "[S]\r\nK=V\r\n", TEXT("S"), TEXT("K"), true, TEXT("V") },
			// LF-only files parse identically: Read splits on \r and \n
			// interchangeably.
			{ "lf_only", "[S]\nK=V\n", TEXT("S"), TEXT("K"), true, TEXT("V") },
			// CR-only (classic Mac) files also parse.
			{ "cr_only", "[S]\rK=V\r", TEXT("S"), TEXT("K"), true, TEXT("V") },
			// Split happens at the FIRST '='; later '=' belong to the value.
			{ "second_equals_in_value", "[S]\nK=a=b\n", TEXT("S"), TEXT("K"), true, TEXT("a=b") },
			// Empty value after '=' is found and is the empty string.
			{ "empty_value", "[S]\nK=\n", TEXT("S"), TEXT("K"), true, TEXT("") },
			// A value of '=' (i.e. "K==v") yields "=v".
			{ "leading_equals_value", "[S]\nK==v\n", TEXT("S"), TEXT("K"), true, TEXT("=v") },
			// Fully quoted values have exactly the outer quote pair stripped;
			// there is NO escape processing inside quotes.
			{ "quoted", "[S]\nK=\"quoted\"\n", TEXT("S"), TEXT("K"), true, TEXT("quoted") },
			// Inner quotes survive: only the first and last characters are
			// examined.
			{ "quoted_inner_quotes", "[S]\nK=\"a\"b\"\n", TEXT("S"), TEXT("K"), true, TEXT("a\"b") },
			// Unterminated leading quote is kept literally: stripping only
			// happens when the value BOTH starts and ends with '"'.
			{ "unterminated_quote", "[S]\nK=\"partial\n", TEXT("S"), TEXT("K"), true, TEXT("\"partial") },
			// Whitespace is never trimmed, neither on keys nor values.
			{ "untrimmed_value", "[S]\nK= untrimmed \n", TEXT("S"), TEXT("K"), true, TEXT(" untrimmed ") },
			{ "untrimmed_key", "[S]\n  Key =  Val \n", TEXT("S"), TEXT("  Key "), true, TEXT("  Val ") },
			// There is no inline-comment syntax: everything after '=' up to
			// the line break is the value.
			{ "inline_comment_is_value", "[S]\nc=x ; tail\n", TEXT("S"), TEXT("c"), true, TEXT("x ; tail") },
			// A bare ';'/'#' comment line that CONTAINS '=' is parsed as a
			// key/value pair ('; note' = 'x'); the engine has no comment
			// support at all.
			{ "comment_with_equals_is_pair", "[S]\n; note=x\n", TEXT("S"), TEXT("; note"), true, TEXT("x") },
			// Keys are stored and matched verbatim (quotes included).
			{ "quoted_key_verbatim", "[S]\n\"Q\"=1\n", TEXT("S"), TEXT("\"Q\""), true, TEXT("1") },
			// Blank lines before a section are fine.
			{ "leading_blanks", "\n\n[S]\nK=V\n", TEXT("S"), TEXT("K"), true, TEXT("V") },
			// Assignments before the first section header are dropped.
			{ "before_section_dropped", "K=V\n[S]\n", TEXT("S"), TEXT("K"), false, TEXT("") },
			// A line without '=' inside a section is silently dropped.
			{ "no_equals_dropped", "[S]\nK\n", TEXT("S"), TEXT("K"), false, TEXT("") },
			// A '[' line that does not END with ']' is not a section header;
			// without '=' it is dropped entirely.
			{ "unclosed_section_dropped", "[Unclosed\nK=V\n", TEXT("Unclosed"), TEXT("K"), false, TEXT("") },
			{ "section_with_trailing_junk_dropped", "[S]tray\nK=V\n", TEXT("S"), TEXT("K"), false, TEXT("") },
			// The empty section name '[]' is legal.
			{ "empty_section_name", "[]\nK=V\n", TEXT(""), TEXT("K"), true, TEXT("V") },
			// A completely empty file has no sections.
			{ "empty_file", "", TEXT("S"), TEXT("K"), false, TEXT("") },
		};

		for( const ParseCase& C : Cases )
		{
			WriteBytes( C.Name, C.IniText );
			FConfigFile File;
			File.Read( *PathAsFString( C.Name ) );
			FString Value;
			const bool Found = SectionValue( File, C.Section, C.Key, Value ) != 0;
			if( C.ExpectFound )
			{
				Require( Found, "%s: expected [%s]%s to parse", C.Name,
					appToAnsi( C.Section ), appToAnsi( C.Key ) );
				if( Found )
					Require( Exact( Value, C.ExpectValue ), "%s: [%s]%s: expected \"%s\", got \"%s\"",
						C.Name, appToAnsi( C.Section ), appToAnsi( C.Key ),
						appToAnsi( C.ExpectValue ), appToAnsi( *Value ) );
			}
			else
			{
				Require( !Found, "%s: expected [%s]%s NOT to parse", C.Name,
					appToAnsi( C.Section ), appToAnsi( C.Key ) );
			}
		}

		// Missing file: Read leaves the config empty (no error, no throw).
		{
			FConfigFile File;
			File.Read( *PathAsFString( "definitely_missing_file.ini" ) );
			FString Value;
			Require( SectionValue( File, TEXT("S"), TEXT("K"), Value ) == 0,
				"missing file must yield an empty config" );
		}

		// Duplicate keys: TMultiMap keeps every pair, but lookups walk the
		// hash chain newest-first, so the LAST occurrence in file order wins.
		{
			WriteBytes( "dup_keys", "[S]\nA=1\nA=2\n" );
			FConfigFile File;
			File.Read( *PathAsFString( "dup_keys" ) );
			FString Value;
			Require( SectionValue( File, TEXT("S"), TEXT("A"), Value ) && Exact( Value, TEXT("2") ),
				"duplicate key: expected last-wins value \"2\", got \"%s\"", appToAnsi( *Value ) );
			Require( CountValues( File, TEXT("S"), TEXT("A") ) == 2,
				"duplicate key: both occurrences must be retained" );
		}

		// Duplicate keys that differ only by case: BOTH are stored (Read
		// never dedupes) and lookups are case-insensitive, so the later
		// entry still shadows the earlier one.
		{
			WriteBytes( "dup_case_keys", "[S]\nA=1\na=2\n" );
			FConfigFile File;
			File.Read( *PathAsFString( "dup_case_keys" ) );
			FString Value;
			Require( SectionValue( File, TEXT("S"), TEXT("A"), Value ) && Exact( Value, TEXT("2") ),
				"case-differing duplicates: expected \"2\" for lookup A, got \"%s\"", appToAnsi( *Value ) );
			Require( SectionValue( File, TEXT("S"), TEXT("a"), Value ) && Exact( Value, TEXT("2") ),
				"case-differing duplicates: lookup must be case-insensitive" );
			Require( CountValues( File, TEXT("S"), TEXT("A") ) == 2,
				"case-differing duplicates: both entries retained" );
		}

		// Section headers are matched case-insensitively: [Render] and
		// [SEC] below merge into the FIRST-seen spelling ("Render").
		{
			WriteBytes( "section_case_merge", "[Render]\nk=1\n[RENDER]\nk2=2\n" );
			FConfigFile File;
			File.Read( *PathAsFString( "section_case_merge" ) );
			FString Value;
			Require( SectionValue( File, TEXT("render"), TEXT("K"), Value ) && Exact( Value, TEXT("1") ),
				"section lookup must be case-insensitive" );
			Require( SectionValue( File, TEXT("Render"), TEXT("K2"), Value ) && Exact( Value, TEXT("2") ),
				"case-variant section headers must merge into one section" );
			Require( File.Num() == 1, "merged case-variant sections must produce a single section, got %i", File.Num() );
		}

		// Same key name in different sections stays independent.
		{
			WriteBytes( "two_sections", "[S]\nk=v\n[T]\nk=w\n" );
			FConfigFile File;
			File.Read( *PathAsFString( "two_sections" ) );
			FString Value;
			Require( SectionValue( File, TEXT("S"), TEXT("k"), Value ) && Exact( Value, TEXT("v") ), "section S/k" );
			Require( SectionValue( File, TEXT("T"), TEXT("k"), Value ) && Exact( Value, TEXT("w") ), "section T/k" );
		}

		if( GFailures )
			Failf( "parse_semantics: %i failure(s)", GFailures );
	}

	/*---------------------------------------------------------------------
		2. Typed getters: int/float/bool parsing quirks.
	---------------------------------------------------------------------*/

	void TestTypedGetters()
	{
		// LF endings here also re-pins LF parsing inside the cache path.
		WriteBytes( "typed.ini",
			"[Nums]\n"
			"IntPlain=42\n"
			"IntSigned=-7\n"
			"IntPlus=+7\n"
			"IntWsGarbage= 42abc\n"
			"IntHex=0x10\n"
			"IntWord=yes\n"
			"IntEmpty=\n"
			"IntFloaty=3.9\n"
			"[Reals]\n"
			"F1=1.5\n"
			"FExp=2e1\n"
			"FGarbage=abc\n"
			"FComma=1,5\n"
			"[Bools]\n"
			"BTrue=True\n"
			"BLower=true\n"
			"BUpper=TRUE\n"
			"BOne=1\n"
			"BZero=0\n"
			"BNoWord=no\n"
			"BTwo=2\n"
			"BSpaceed= true \n"
			"BOneTrailing=1x\n"
			"BFalse=False\n"
			"[Empty]\n" );

		FConfigCacheIni Cache;
		const FString File = PathAsFString( "typed.ini" );

		// --- GetInt: appAtoi == wcstol base 10. Returns 1 whenever the key
		// EXISTS, even when the text is garbage (Value then 0).
		INT V = 0;
		struct IntCase { const TCHAR* Key; INT Expected; };
		static const IntCase IntCases[] =
		{
			{ TEXT("IntPlain"), 42 },
			{ TEXT("IntSigned"), -7 },
			{ TEXT("IntPlus"), 7 },      // wcstol accepts a leading '+'
			{ TEXT("IntWsGarbage"), 42 },// leading whitespace skipped, parse stops at 'a'
			{ TEXT("IntHex"), 0 },       // NO hex support: base-10 parse of "0x10" stops at 'x'
			{ TEXT("IntWord"), 0 },      // non-numeric text yields 0 but still returns 1
			{ TEXT("IntEmpty"), 0 },     // empty value yields 0 but still returns 1
			{ TEXT("IntFloaty"), 3 },    // no rounding: parse stops at '.'
		};
		for( const IntCase& C : IntCases )
		{
			V = -12345;
			Require( Cache.GetInt( TEXT("Nums"), C.Key, V, *File ) == 1,
				"GetInt(%s) must return 1 for an existing key", appToAnsi( C.Key ) );
			Require( V == C.Expected, "GetInt(%s): expected %i, got %i",
				appToAnsi( C.Key ), C.Expected, V );
		}

		// Missing key: returns 0 and leaves the caller's Value untouched.
		V = 987;
		Require( Cache.GetInt( TEXT("Nums"), TEXT("Absent"), V, *File ) == 0, "GetInt missing key must return 0" );
		Require( V == 987, "GetInt missing key must not modify Value" );

		// --- GetFloat: appAtof == atof (LC_ALL=C in the test environment,
		// so '.' is the decimal separator).
		FLOAT F = 0.0f;
		struct FloatCase { const TCHAR* Key; FLOAT Expected; };
		static const FloatCase FloatCases[] =
		{
			{ TEXT("F1"), 1.5f },
			{ TEXT("FExp"), 20.0f },    // exponent notation supported
			{ TEXT("FGarbage"), 0.0f }, // garbage yields 0 but still returns 1
			{ TEXT("FComma"), 1.0f },   // C locale: parse stops at ','
		};
		for( const FloatCase& C : FloatCases )
		{
			F = -1.0f;
			Require( Cache.GetFloat( TEXT("Reals"), C.Key, F, *File ) == 1,
				"GetFloat(%s) must return 1 for an existing key", appToAnsi( C.Key ) );
			Require( F == C.Expected, "GetFloat(%s): expected %g, got %g", appToAnsi( C.Key ), C.Expected, F );
		}
		// Integer text converts through atof exactly.
		F = -1.0f;
		Cache.GetFloat( TEXT("Nums"), TEXT("IntPlain"), F, *File );
		Require( F == 42.0f, "GetFloat(IntPlain): expected 42, got %g", F );

		// --- GetBool: 1 when appStricmp(Text,"True")==0, otherwise
		// Value = (appAtoi(Text)==1). So: any integer other than exactly 1
		// is FALSE (including 2!), "yes"/"no" are NOT booleans, but " true "
		// with spaces is TRUE because wcstol skips whitespace and finds 1.
		struct BoolCase { const TCHAR* Key; UBOOL Expected; };
		static const BoolCase BoolCases[] =
		{
			{ TEXT("BTrue"), 1 },
			{ TEXT("BLower"), 1 },   // stricmp match: case-insensitive "true"
			{ TEXT("BUpper"), 1 },
			{ TEXT("BOne"), 1 },     // atoi==1
			{ TEXT("BZero"), 0 },
			{ TEXT("BNoWord"), 0 },  // "no" is NOT a boolean literal; atoi("no")==0
			{ TEXT("BTwo"), 0 },     // QUIRK: "2" is false (must be exactly 1)
			{ TEXT("BSpaceed"), 0 }, // QUIRK: " true " fails stricmp("True") AND atoi -> false
			{ TEXT("BOneTrailing"), 1 }, // "1x" -> atoi==1 -> true
			{ TEXT("BFalse"), 0 },   // "False": not stricmp-True, atoi==0
		};
		for( const BoolCase& C : BoolCases )
		{
			UBOOL B = 77;
			Require( Cache.GetBool( TEXT("Bools"), C.Key, B, *File ) == 1,
				"GetBool(%s) must return 1 for an existing key", appToAnsi( C.Key ) );
			Require( B == C.Expected, "GetBool(%s): expected %i, got %i",
				appToAnsi( C.Key ), C.Expected, B );
		}

		// Missing key: returns 0, Value untouched.
		UBOOL B = 5;
		Require( Cache.GetBool( TEXT("Bools"), TEXT("Absent"), B, *File ) == 0, "GetBool missing key must return 0" );
		Require( B == 5, "GetBool missing key must not modify Value" );

		// --- GetString buffer contract: appStrncpy keeps Size-1 characters
		// and NUL-terminates.
		WriteBytes( "trunc.ini", "[S]\nK=abcdefghij\n" );
		FConfigCacheIni TruncCache;
		const FString TruncFile = PathAsFString( "trunc.ini" );
		TCHAR Buf[5] = TEXT("ZZZZ");
		Require( TruncCache.GetString( TEXT("S"), TEXT("K"), Buf, 5, *TruncFile ) == 1, "GetString must find K" );
		Require( appStrcmp( Buf, TEXT("abcd") ) == 0, "GetString(Size=5) must truncate to 4 chars, got \"%s\"", appToAnsi( Buf ) );
		TCHAR Tiny[1] = { TEXT('Z') };
		Require( TruncCache.GetString( TEXT("S"), TEXT("K"), Tiny, 1, *TruncFile ) == 1, "GetString(Size=1) must still return 1" );
		Require( Tiny[0] == 0, "GetString(Size=1) must yield an empty string" );

		// --- GetSection: entries joined "k=v" and separated by NULs, with a
		// final extra NUL (double-NUL terminated list).
		FConfigCacheIni SecCache;
		WriteBytes( "two_sections.ini", "[S]\nk=v\n[T]\nk=w\n" );
		const FString SecFile = PathAsFString( "two_sections.ini" ); // no dot-in-last-4 => engine would append .ini
		TCHAR Sec[64];
		Require( SecCache.GetSection( TEXT("S"), Sec, 64, *SecFile ) == 1, "GetSection must find S" );
		{
			static const TCHAR Expected[] = TEXT("k=v");
			// Layout: "k=v" '\0' '\0'
			Require( appStrcmp( Sec, Expected ) == 0 && Sec[4] == 0,
				"GetSection layout mismatch: got \"%s\"", appToAnsi( Sec ) );
		}

		WriteBytes( "sect2.ini", "[S]\nK1=V1\nK2=V2\n" );
		FConfigCacheIni Sec2Cache;
		const FString Sec2File = PathAsFString( "sect2.ini" );
		TCHAR Sec2[64];
		Require( Sec2Cache.GetSection( TEXT("S"), Sec2, 64, *Sec2File ) == 1, "GetSection must find S" );
		{
			// "K1=V1" '\0' "K2=V2" '\0' plus the final terminator = 13
			// TCHARs actually written by GetSection.
			static const TCHAR Expected[13] =
			{ 'K','1','=','V','1',0,'K','2','=','V','2',0,0 };
			Require( appStrncmp( Sec2, Expected, 13 ) == 0, "GetSection two-pair layout mismatch" );
		}
		// Truncated GetSection: the loop stops before an entry that no longer
		// fits (Size=8 fits only "K1=V1").
		TCHAR SecT[8];
		Require( Sec2Cache.GetSection( TEXT("S"), SecT, 8, *Sec2File ) == 1, "GetSection truncated must return 1" );
		Require( appStrcmp( SecT, TEXT("K1=V1") ) == 0 && SecT[6] == 0,
			"GetSection(Size=8) must contain only the first pair" );

		if( GFailures )
			Failf( "typed_getters: %i failure(s)", GFailures );
	}

	/*---------------------------------------------------------------------
		3. SetString/SetInt/SetFloat/SetBool, Dirty gating, Detach/NoSave.
	---------------------------------------------------------------------*/

	void TestSetAndWrite()
	{
		// Fresh cache + nonexistent file: SetString creates the file on
		// Write with the exact canonical byte layout.
		{
			const char* Name = "setwrite_new.ini";
			// Artifact dirs persist across ctest reruns; start from absence.
			std::remove( ArtifactPath( Name ).c_str() );
			FConfigCacheIni Cache;
			const FString File = PathAsFString( Name );
			Cache.SetString( TEXT("Video"), TEXT("Mode"), TEXT("32"), *File );
			// Write via Flush(Read=0): persists but keeps the cache entry.
			Cache.Flush( 0, *File );
			const std::string Bytes = ReadBytes( Name );
			// Exact layout: "[Section]\r\nKey=Value\r\n\r\n" — CRLF line
			// endings and one blank line after every section.
			Require( Bytes == "[Video]\r\nMode=32\r\n\r\n",
				"fresh SetString+Flush: unexpected bytes, got %zu bytes starting \"%.32s\"",
				Bytes.size(), Bytes.c_str() );
		}

		// Dirty gating: a cache whose only change is a same-value SetString
		// is NOT dirty, so Write creates nothing.
		{
			const char* Name = "setwrite_same.ini";
			WriteBytes( Name, "[S]\nK=v\n" ); // clean on-disk state
			FConfigCacheIni Cache;
			const FString File = PathAsFString( Name );
			Cache.SetString( TEXT("S"), TEXT("K"), TEXT("v"), *File ); // identical re-set
			FConfigFile* Entry = Cache.TMap<FString,FConfigFile>::Find( File );
			Require( Entry != NULL && Entry->Dirty == 0,
				"same-value SetString must leave the file clean" );
			Cache.Flush( 0, *File );
			// Unchanged: still the LF fixture, never rewritten as canonical
			// CRLF output.
			Require( ReadBytes( Name ) == "[S]\nK=v\n",
				"clean file must not be rewritten, got \"%.32s\"", ReadBytes( Name ).c_str() );
		}

		// QUIRK (migration-critical): a case-ONLY value change is a silent
		// no-op. SetString looks the key up case-insensitively and then only
		// enters the update branch when appStricmp differs — "True" -> "TRUE"
		// neither updates the stored value nor marks the file dirty.
		{
			const char* Name = "setwrite_case.ini";
			WriteBytes( Name, "[S]\nK=True\n" ); // clean on-disk state
			FConfigCacheIni Cache;
			const FString File = PathAsFString( Name );
			Cache.SetString( TEXT("S"), TEXT("K"), TEXT("TRUE"), *File );
			FConfigFile* Entry = Cache.TMap<FString,FConfigFile>::Find( File );
			Require( Entry != NULL && Entry->Dirty == 0,
				"case-only SetString must not mark the file dirty" );
			Cache.Flush( 0, *File );
			Require( ReadBytes( Name ) == "[S]\nK=True\n",
				"case-only SetString must not rewrite the file, got \"%.32s\"", ReadBytes( Name ).c_str() );
			// The in-memory value is still the ORIGINAL casing.
			TCHAR Buf[16];
			Require( Cache.GetString( TEXT("S"), TEXT("K"), Buf, 16, *File ) == 1, "GetString after case-only set" );
			Require( appStrcmp( Buf, TEXT("True") ) == 0,
				"case-only SetString must keep the old value, got \"%s\"", appToAnsi( Buf ) );
		}

		// A value that differs case-INSENSITIVELY updates and dirties; the
		// dirty flag is exactly "case-sensitively different".
		{
			const char* Name = "setwrite_update.ini";
			WriteBytes( Name, "[S]\nK=True\n" ); // clean on-disk state
			FConfigCacheIni Cache;
			const FString File = PathAsFString( Name );
			Cache.SetString( TEXT("S"), TEXT("K"), TEXT("False"), *File );
			FConfigFile* Entry = Cache.TMap<FString,FConfigFile>::Find( File );
			Require( Entry != NULL && Entry->Dirty == 1, "real value change must dirty the file" );
			Cache.Flush( 0, *File );
			Require( ReadBytes( Name ) == "[S]\r\nK=False\r\n\r\n",
				"updated value must be written, got \"%.32s\"", ReadBytes( Name ).c_str() );
		}

		// Canonical typed writers.
		{
			const char* Name = "setwrite_typed.ini";
			FConfigCacheIni Cache;
			const FString File = PathAsFString( Name );
			Cache.SetInt( TEXT("S"), TEXT("I"), -42, *File );
			Cache.SetFloat( TEXT("S"), TEXT("F"), 0.5f, *File );
			Cache.SetBool( TEXT("S"), TEXT("B"), 1, *File );
			Cache.SetBool( TEXT("S"), TEXT("B2"), 0, *File );
			Cache.Flush( 0, *File );
			// SetInt uses "%i", SetFloat uses "%f" (six decimals), SetBool
			// writes the literals True/False.
			Require( ReadBytes( Name ) == "[S]\r\nI=-42\r\nF=0.500000\r\nB=True\r\nB2=False\r\n\r\n",
				"typed writers byte layout mismatch, got \"%.64s\"", ReadBytes( Name ).c_str() );
		}

		// SetString on a key with pre-existing duplicates updates only the
		// newest entry (the one lookups return); the older duplicate stays
		// in the section and is written back on rewrite.
		{
			WriteBytes( "setwrite_dup.ini", "[S]\nA=1\nA=2\n" );
			FConfigCacheIni Cache;
			const FString File = PathAsFString( "setwrite_dup.ini" );
			Cache.SetString( TEXT("S"), TEXT("A"), TEXT("3"), *File );
			Cache.Flush( 0, *File );
			// The newest pair (A=2) becomes A=3; A=1 remains untouched.
			Require( ReadBytes( "setwrite_dup.ini" ) == "[S]\r\nA=1\r\nA=3\r\n\r\n",
				"SetString on duplicated key must update the newest entry only, got \"%.48s\"",
				ReadBytes( "setwrite_dup.ini" ).c_str() );
		}

		// EmptySection: clears an existing non-empty section and dirties;
		// a missing section is a no-op (no dirty, no write).
		{
			WriteBytes( "emptysection.ini", "[S]\nK=V\n[T]\nJ=W\n" );
			FConfigCacheIni Cache;
			const FString File = PathAsFString( "emptysection.ini" );
			Cache.EmptySection( TEXT("Absent"), *File ); // no-op
			FConfigFile* Entry = Cache.TMap<FString,FConfigFile>::Find( File );
			Require( Entry != NULL && Entry->Dirty == 0, "EmptySection on missing section must not dirty" );
			Cache.EmptySection( TEXT("S"), *File );
			Require( Entry != NULL && Entry->Dirty == 1, "EmptySection on existing section must dirty" );
			Cache.Flush( 0, *File );
			// The emptied section still exists: bare header + blank line.
			Require( ReadBytes( "emptysection.ini" ) == "[S]\r\n\r\n[T]\r\nJ=W\r\n\r\n",
				"emptied section must remain as a bare header, got \"%.48s\"",
				ReadBytes( "emptysection.ini" ).c_str() );
		}

		// Detach sets NoSave: even dirty data is never written, including by
		// the destructor's implicit Flush.
		{
			const char* Name = "detach_nosave.ini";
			std::remove( ArtifactPath( Name ).c_str() );
			{
				FConfigCacheIni Cache;
				const FString File = PathAsFString( Name );
				Cache.SetString( TEXT("S"), TEXT("K"), TEXT("v"), *File );
				Cache.Detach( *File );
				Cache.Flush( 0, *File ); // explicit attempt must be a no-op too
				Require( !FileExists( Name ), "detached (NoSave) file must not be written by Flush" );
			} // destructor Flush(1) here — must also not write
			Require( !FileExists( Name ), "detached (NoSave) file must not be written by the destructor" );
		}

		// Write of a dirty but EMPTY config: appSaveStringToFile refuses
		// empty strings, so Write returns 0 and creates nothing.
		{
			const char* Name = "empty_dirty.ini";
			std::remove( ArtifactPath( Name ).c_str() );
			FConfigFile File; // fresh, empty, Dirty=0
			Require( File.Write( *PathAsFString( Name ) ) == 1, "clean Write must return 1 without writing" );
			Require( !FileExists( Name ), "clean Write must create nothing" );
			File.Dirty = 1;
			Require( File.Write( *PathAsFString( Name ) ) == 0, "dirty empty Write must return 0 (empty string save refused)" );
			Require( !FileExists( Name ), "dirty empty Write must create no file" );
		}

		if( GFailures )
			Failf( "set_and_write: %i failure(s)", GFailures );
	}

	/*---------------------------------------------------------------------
		4. Rewrite normalization: what a Read->Write round-trip keeps,
		   drops, and rewrites.
	---------------------------------------------------------------------*/

	void TestRewriteNormalization()
	{
		// Mixed line endings, bare comments, comment-with-'=', quoted value,
		// duplicate keys, a '='-less line, and a repeated section header.
		WriteBytes( "rewrite.ini",
			"; top comment\r\n"
			"[Render]\r\n"
			"Mode=32\r\n"
			"; note=x\r\n"
			"Quality=\"High\"\r\n"
			"noequals\r\n"
			"Mode=16\n"
			"[Audio]\n"
			"\n"
			"[Render]\r\n"
			"Extra=1\r\n" );

		FConfigFile File;
		File.Read( *PathAsFString( "rewrite.ini" ) );

		// Pinned read-side facts feeding the rewrite:
		// - bare comment dropped, comment-with-'=' became the pair "; note"=x
		// - quoted value stored WITHOUT quotes
		// - duplicate Mode entries both stored, last one wins lookups
		// - [Audio] exists but is empty; the second [Render] header merged
		FString Value;
		Require( SectionValue( File, TEXT("Render"), TEXT("Quality"), Value ) && Exact( Value, TEXT("High") ),
			"rewrite input: quoted value must be stored unquoted" );
		Require( SectionValue( File, TEXT("Render"), TEXT("MODE"), Value ) && Exact( Value, TEXT("16") ),
			"rewrite input: duplicate key last-wins" );
		Require( SectionValue( File, TEXT("Render"), TEXT("; note"), Value ) && Exact( Value, TEXT("x") ),
			"rewrite input: comment-with-= must be a pair" );
		Require( CountValues( File, TEXT("Render"), TEXT("Mode") ) == 2, "rewrite input: duplicates retained" );

		// Mark dirty and write back; then pin the EXACT canonical bytes.
		File.Dirty = 1;
		Require( File.Write( *PathAsFString( "rewrite.ini" ) ) == 1, "rewrite Write must succeed" );
		const std::string Out = ReadBytes( "rewrite.ini" );
		static const char Expected[] =
			"[Render]\r\n"
			"Mode=32\r\n"      // insertion order preserved (first occurrence first)
			"; note=x\r\n"     // the pseudo pair survives verbatim
			"Quality=High\r\n" // quotes are NOT re-added: rewrite strips them forever
			"Mode=16\r\n"      // duplicate kept, original order
			"Extra=1\r\n"      // merged from the second [Render] header
			"\r\n"             // blank line after each section
			"[Audio]\r\n"      // empty section: bare header
			"\r\n";
		Require( Out == Expected,
			"rewrite bytes mismatch:\n expected %zu bytes\n got      %zu bytes: \"%.200s\"",
			(size_t)(sizeof(Expected)-1), Out.size(), Out.c_str() );

		// Re-reading the rewritten file is stable (fixed point).
		FConfigFile Again;
		Again.Read( *PathAsFString( "rewrite.ini" ) );
		Again.Dirty = 1;
		Again.Write( *PathAsFString( "rewrite2.ini" ) );
		Require( ReadBytes( "rewrite2.ini" ) == Expected, "second rewrite must be a fixed point" );

		if( GFailures )
			Failf( "rewrite_normalization: %i failure(s)", GFailures );
	}

	/*---------------------------------------------------------------------
		5. FConfigCacheIni filename handling. Pinned ENGINE REALITY:
		   - Find() appends ".ini" when the name is shorter than five
		     characters or carries NO '.' at either of the positions Len-4
		     and Len-5 (both count as an extension dot); otherwise the name
		     is used verbatim, so any dot in the last five characters
		     suppresses appending ("a.b.c" is never re-extended).
		   - user.ini/system.ini translation happens AFTER extension
	           appending and replaces the whole name with the configured
	           SystemIni/UserIni path; NULL means SystemIni.
		   - Resolved names hit the filesystem relative to the process CWD,
		     so file-backed assertions below use ABSOLUTE fixture paths;
		     the pure string-resolution rule is pinned through SetString's
		     cache keys, which need no filesystem backing.
	---------------------------------------------------------------------*/

	void TestCacheFilenames()
	{
		WriteBytes( "CfgSys.ini", "[S]\nA=from_sys\n" );
		WriteBytes( "CfgUserX.ini", "[S]\nA=from_user\n" );
		WriteBytes( "PlainName.ini", "[S]\nA=from_plain\n" );

		FConfigCacheIni Cache;
		const FString Sys = PathAsFString( "CfgSys.ini" );
		const FString Usr = PathAsFString( "CfgUserX.ini" );
		Cache.Init( *Sys, *Usr, 0 );

		TCHAR Buf[32];

		// "user.ini" (any casing) is transparently translated to UserIni.
		Require( Cache.GetString( TEXT("S"), TEXT("A"), Buf, 32, TEXT("user.ini") ) == 1, "user.ini translation must find the file" );
		Require( appStrcmp( Buf, TEXT("from_user") ) == 0, "user.ini must read UserIni, got \"%s\"", appToAnsi( Buf ) );
		Require( Cache.GetString( TEXT("S"), TEXT("A"), Buf, 32, TEXT("USER.INI") ) == 1, "USER.INI translation must be case-insensitive" );
		Require( appStrcmp( Buf, TEXT("from_user") ) == 0, "USER.INI must read UserIni" );

		// "system.ini" likewise translates to SystemIni.
		Require( Cache.GetString( TEXT("S"), TEXT("A"), Buf, 32, TEXT("system.ini") ) == 1, "system.ini translation" );
		Require( appStrcmp( Buf, TEXT("from_sys") ) == 0, "system.ini must read SystemIni" );

		// Filename=NULL defaults to SystemIni.
		Require( Cache.GetString( TEXT("S"), TEXT("A"), Buf, 32, NULL ) == 1, "NULL filename must default to SystemIni" );
		Require( appStrcmp( Buf, TEXT("from_sys") ) == 0, "NULL filename must read SystemIni" );

		// Extension appending on a real lookup: the extension-less absolute
		// fixture path gains ".ini" and resolves.
		const FString Plain = PathAsFString( "PlainName" );
		Require( Cache.GetString( TEXT("S"), TEXT("A"), Buf, 32, *Plain ) == 1, "extension must be appended to the extension-less path" );
		Require( appStrcmp( Buf, TEXT("from_plain") ) == 0, "extension-less path must resolve to PlainName.ini" );

		// A name that already ends in ".ini" is used verbatim (absolute path).
		Require( Cache.GetString( TEXT("S"), TEXT("A"), Buf, 32, *Usr ) == 1, "explicit .ini name must be used verbatim" );
		Require( appStrcmp( Buf, TEXT("from_user") ) == 0, "explicit .ini name must not be re-extended" );

		// Pure string-resolution rule, pinned through SetString's cache keys
		// (SetString always materializes the entry under the RESOLVED name,
		// even when no file exists). Every entry is detached afterwards so
		// the cache destructor never writes these scratch keys anywhere.
		{
			FConfigCacheIni Rules;
			Rules.Init( *Sys, *Usr, 0 );
			Rules.SetString( TEXT("R"), TEXT("K"), TEXT("v"), TEXT("ab") );
			Require( Rules.TMap<FString,FConfigFile>::Find( TEXT("ab.ini") ) != NULL, "names shorter than five characters get .ini appended" );
			Rules.SetString( TEXT("R"), TEXT("K"), TEXT("v"), TEXT("abcde") );
			Require( Rules.TMap<FString,FConfigFile>::Find( TEXT("abcde.ini") ) != NULL, "five-character name without a dot gets .ini appended" );
			Rules.SetString( TEXT("R"), TEXT("K"), TEXT("v"), TEXT("a.b.c") );
			Require( Rules.TMap<FString,FConfigFile>::Find( TEXT("a.b.c") ) != NULL &&
				Rules.TMap<FString,FConfigFile>::Find( TEXT("a.b.c.ini") ) == NULL,
				"a dot at position Len-4 counts as an existing extension" );
			Rules.SetString( TEXT("R"), TEXT("K"), TEXT("v"), TEXT("wxyz.ini") );
			Require( Rules.TMap<FString,FConfigFile>::Find( TEXT("wxyz.ini") ) != NULL &&
				Rules.TMap<FString,FConfigFile>::Find( TEXT("wxyz.ini.ini") ) == NULL,
				"explicit .ini names are used verbatim" );
			Rules.Detach( TEXT("ab.ini") );
			Rules.Detach( TEXT("abcde.ini") );
			Rules.Detach( TEXT("a.b.c") );
			Rules.Detach( TEXT("wxyz.ini") );
		}

		// A missing file yields return 0 (and no file is created because
		// GetString uses CreateIfNotFound=0).
		const FString Missing = PathAsFString( "NopeMissing.ini" );
		Require( Cache.GetString( TEXT("S"), TEXT("A"), Buf, 32, *Missing ) == 0, "missing file must return 0" );
		Require( !FileExists( "NopeMissing.ini" ), "GetString must not create missing files" );

		// The cache is keyed by the RESOLVED filename: "user.ini" and the
		// UserIni path hit the same entry.
		Cache.SetString( TEXT("S"), TEXT("A"), TEXT("changed"), TEXT("user.ini") );
		FConfigFile* Entry = Cache.TMap<FString,FConfigFile>::Find( Usr );
		Require( Entry != NULL, "SetString via user.ini must populate the resolved UserIni entry" );
		TCHAR Buf2[32];
		Require( Cache.GetString( TEXT("S"), TEXT("A"), Buf2, 32, *Usr ) == 1, "resolved path must see the same cache entry" );
		Require( appStrcmp( Buf2, TEXT("changed") ) == 0, "user.ini and UserIni path must share one cache entry" );

		if( GFailures )
			Failf( "cache_filenames: %i failure(s)", GFailures );
	}

	/*---------------------------------------------------------------------
		6. Unicode round-trip: non-ASCII values are persisted as UTF-16LE
		   with a BOM (TCHAR is UTF-32 on this host), ASCII-only values as
		   raw ANSI bytes with no BOM.
	---------------------------------------------------------------------*/

	void TestUnicodeRoundTrip()
	{
		// "caf" + U+00E9 + U+4E2D + U+1F3AE (astral: exercises surrogate
		// encoding) + "!" — built from explicit code units so the fixture
		// never depends on source-file encoding.
		static const TCHAR Text[] = { 'c','a','f', 0x00E9, 0x4E2D, 0x1F3AE, '!', 0 };

		const char* Name = "unicode_rt.ini";
		{
			FConfigCacheIni Cache;
			const FString File = PathAsFString( Name );
			Cache.SetString( TEXT("Uni"), TEXT("Text"), Text, *File );
			Cache.Flush( 0, *File );
		}

		const std::string Bytes = ReadBytes( Name );
		Require( Bytes.size() >= 2, "unicode file must exist with a BOM" );
		// Any char > U+00FF forces the Unicode save path: FF FE BOM + UTF-16LE.
		Require( (unsigned char)Bytes[0] == 0xFF && (unsigned char)Bytes[1] == 0xFE,
			"non-ASCII content must be saved as UTF-16LE with BOM, got %02X %02X",
			(unsigned char)Bytes[0], (unsigned char)Bytes[1] );
		// U+4E2D in UTF-16LE is the byte pair 2D 4E.
		Require( Bytes.find( std::string("\x2D\x4E", 2) ) != std::string::npos,
			"U+4E2D must appear as UTF-16LE bytes 2D 4E" );
		// U+1F3AE encodes as the surrogate pair D83C DFAE; little-endian
		// that is the byte sequence 3C D8 AE DF.
		Require( Bytes.find( std::string("\x3C\xD8\xAE\xDF", 4) ) != std::string::npos,
			"U+1F3AE must appear as the surrogate pair D83C DFAE (bytes 3C D8 AE DF)" );
		{
			FConfigCacheIni Cache;
			const FString File = PathAsFString( Name );
			FString Loaded;
			Require( Cache.GetString( TEXT("Uni"), TEXT("Text"), Loaded, *File ) == 1, "unicode reload must find the key" );
			Require( appStrcmp( *Loaded, Text ) == 0, "unicode round-trip must reproduce the exact value" );
			Require( appStrlen( *Loaded ) == 7, "astral char must decode to ONE host TCHAR (got %i units)", appStrlen( *Loaded ) );
		}

		// ASCII-only content takes the ANSI path: no BOM, raw bytes.
		{
			const char* AsciiName = "ascii_only.ini";
			FConfigCacheIni Cache;
			const FString File = PathAsFString( AsciiName );
			Cache.SetString( TEXT("S"), TEXT("K"), TEXT("plain"), *File );
			Cache.Flush( 0, *File );
			Require( ReadBytes( AsciiName ) == "[S]\r\nK=plain\r\n\r\n",
				"ASCII-only content must be saved as raw ANSI bytes without BOM, got \"%.32s\"",
				ReadBytes( AsciiName ).c_str() );
		}

		// A UTF-16LE BOM fixture is decoded on read: section/key/value all
		// parse from the decoded text.
		{
			// Build "[S]\r\nK=V\r\n" as UTF-16LE with BOM.
			static const TCHAR Body[] = TEXT("[S]\r\nK=V\r\n");
			std::string Bytes16( "\xFF\xFE", 2 );
			for( const TCHAR* P = Body; *P; ++P )
			{
				Bytes16.push_back( (char)((*P) & 0xFF) );
				Bytes16.push_back( (char)(((*P) >> 8) & 0xFF) );
			}
			WriteBytes( "utf16_fixture.ini", Bytes16 );
			FConfigFile File;
			File.Read( *PathAsFString( "utf16_fixture.ini" ) );
			FString Value;
			Require( SectionValue( File, TEXT("S"), TEXT("K"), Value ) && Exact( Value, TEXT("V") ),
				"UTF-16LE BOM fixture must decode and parse" );
		}

		if( GFailures )
			Failf( "unicode_roundtrip: %i failure(s)", GFailures );
	}

} // namespace

int main( int ArgC, char** ArgV )
{
	// Must happen before ANY engine allocation: the global operator new
	// overridden in UnFile.h routes through GMalloc, whose Core.cpp default
	// aborts on use.
	GMalloc = &GTestAllocator;
	GFileManager = &GTestFileManager;

	const char* Env = std::getenv( "HP2_ARTIFACT_DIR" );
	if( !Env || !*Env )
	{
		Failf( "HP2_ARTIFACT_DIR is required (all test files must stay inside it)" );
		return 1;
	}
	GArtifactDir = Env;

	const char* Test = NULL;
	for( int Index = 1; Index < ArgC; ++Index )
	{
		if( std::strncmp( ArgV[Index], "--test=", 7 ) == 0 )
			Test = ArgV[Index] + 7;
	}

	const bool RunAll = ( Test == NULL );
	struct Group { const char* Name; void (*Fn)(); };
	static const Group Groups[] =
	{
		{ "parse_semantics",        TestParseSemantics },
		{ "typed_getters",          TestTypedGetters },
		{ "set_and_write",          TestSetAndWrite },
		{ "rewrite_normalization",  TestRewriteNormalization },
		{ "cache_filenames",        TestCacheFilenames },
		{ "unicode_roundtrip",      TestUnicodeRoundTrip },
	};

	bool Matched = false;
	for( const Group& G : Groups )
	{
		if( RunAll || std::strcmp( Test, G.Name ) == 0 )
		{
			Matched = true;
			G.Fn();
		}
	}
	if( !Matched )
	{
		Failf( "unknown --test value '%s'", Test );
		return 1;
	}
	if( GFailures != 0 )
	{
		std::printf( "%s: %i failure(s)\n", GTestName, GFailures );
		return 1;
	}
	std::printf( "Config ini contract tests passed\n" );
	return 0;
}
