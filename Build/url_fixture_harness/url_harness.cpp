// Fixture driver for the ORIGINAL UnURL.cpp at the pinned ref (see gen_url.sh).
//
// Drives FURL::StaticInit/StaticExit, FURL(const TCHAR*), FURL(FURL*, TextURL, ETravelType),
// LoadURLConfig, SaveURLConfig, HasOption, GetOption, AddOption, operator==, String,
// IsInternal, IsLocalInternal and appFilePathToFURLToken against a mock GConfig and a mock
// GLog. Each record is one line: KIND=<k>, then the inputs, then the ORIGINAL outputs.
// String values are printed as [..] groups (one group per value, lists as consecutive
// groups); integers and flags are bare. The Rust replay parses the same grammar.
//
// Mock GConfig/GLog implement the pure virtuals the URL code reaches. The remaining pure
// virtuals abort with `STUB <name>`. Core string helpers, FString::Printf and
// FOutputDevice::Logf come from the real Core TUs that gen_url.sh links.
#include "EnginePrivate.h"
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <map>
#include <string>
#include <vector>

[[noreturn]] static void stub( const char* Name )
{
	fprintf( stderr, "STUB %s\n", Name );
	abort();
}

// Side effects observed for the current record: debugf text, GetSection reads and
// SetString writes. Each entry is printed as one [..] group.
static std::wstring g_trace;
static void trace( const std::wstring& Entry )
{
	g_trace += L"[";
	g_trace += Entry;
	g_trace += L"]";
}

class MockLog : public FOutputDevice
{
public:
	void Serialize( const TCHAR* V, EName Event ) override
	{
		(void)Event;
		trace( std::wstring(V) );
	}
};

static MockLog g_log;
static MockLog g_null_log;
FOutputDevice* GLog  = &g_log;
FOutputDevice* GNull = &g_null_log;
// Plain malloc/free allocator for GMalloc. FMallocAnsi times each call through appCycles, which
// has no macOS definition in this build; the timing does not affect URL output.
class HarnessMalloc : public FMalloc
{
public:
	void* Malloc( DWORD Count, const TCHAR* Tag )                        { return malloc( Count ); }
	void* Realloc( void* Original, DWORD Count, const TCHAR* Tag )       { return realloc( Original, Count ); }
	void Free( void* Original )                                          { free( Original ); }
	void DumpAllocs() {}
	void HeapCheck() {}
	void Init() {}
	void Exit() {}
};
static HarnessMalloc g_malloc;
FMalloc* GMalloc = &g_malloc;

static std::map<std::wstring, std::wstring>              g_cfg;      // "Section|Key" -> value
static std::map<std::wstring, std::vector<std::wstring>> g_sections; // Section -> "Key=Value" items

class MockConfig : public FConfigCache
{
public:
	UBOOL GetBool( const TCHAR*, const TCHAR*, UBOOL&, const TCHAR* ) override { stub("GetBool"); }
	UBOOL GetInt( const TCHAR*, const TCHAR*, INT&, const TCHAR* ) override { stub("GetInt"); }
	UBOOL GetFloat( const TCHAR*, const TCHAR*, FLOAT&, const TCHAR* ) override { stub("GetFloat"); }
	UBOOL GetString( const TCHAR*, const TCHAR*, TCHAR*, INT, const TCHAR* ) override { stub("GetString(TCHAR*)"); }
	UBOOL GetString( const TCHAR*, const TCHAR*, FString&, const TCHAR* ) override { stub("GetString(FString&)"); }

	const TCHAR* GetStr( const TCHAR* Section, const TCHAR* Key, const TCHAR* Filename ) override
	{
		(void)Filename;
		auto It = g_cfg.find( std::wstring(Section) + L"|" + Key );
		return It == g_cfg.end() ? L"" : It->second.c_str();
	}

	UBOOL GetSection( const TCHAR* Section, TCHAR* Value, INT Size, const TCHAR* Filename ) override
	{
		(void)Filename;
		trace( std::wstring(L"GS:") + Section );
		INT Pos = 0;
		auto It = g_sections.find( Section );
		if( It != g_sections.end() )
			for( const std::wstring& Item : It->second )
			{
				if( Pos + static_cast<INT>(Item.size()) + 2 > Size )
					stub("GetSection overflow");
				for( wchar_t C : Item )
					Value[Pos++] = C;
				Value[Pos++] = 0;
			}
		Value[Pos] = 0;
		return It != g_sections.end();
	}

	TMultiMap<FString,FString>* GetSectionPrivate( const TCHAR*, UBOOL, UBOOL, const TCHAR* ) override { stub("GetSectionPrivate"); }
	void EmptySection( const TCHAR*, const TCHAR* ) override { stub("EmptySection"); }
	void SetBool( const TCHAR*, const TCHAR*, UBOOL, const TCHAR* ) override { stub("SetBool"); }
	void SetInt( const TCHAR*, const TCHAR*, INT, const TCHAR* ) override { stub("SetInt"); }
	void SetFloat( const TCHAR*, const TCHAR*, FLOAT, const TCHAR* ) override { stub("SetFloat"); }

	void SetString( const TCHAR* Section, const TCHAR* Key, const TCHAR* Value, const TCHAR* Filename ) override
	{
		(void)Filename;
		trace( std::wstring(L"SS:") + Section + L":" + Key + L"=" + Value );
	}

	void Flush( UBOOL, const TCHAR* ) override { stub("Flush"); }
	void Detach( const TCHAR* ) override { stub("Detach"); }
	void Init( const TCHAR*, const TCHAR*, UBOOL ) override { stub("Init"); }
	void Exit() override { stub("Exit"); }
	void Dump( FOutputDevice& ) override { stub("Dump"); }
};

static MockConfig g_config;
FConfigCache* GConfig = &g_config;

// Deterministic xorshift generator; the fixture must regenerate byte-identically.
static uint64_t g_rng = 0x9E3779B97F4A7C15ull;
static uint32_t rnd( uint32_t N )
{
	g_rng ^= g_rng << 13;
	g_rng ^= g_rng >> 7;
	g_rng ^= g_rng << 17;
	return static_cast<uint32_t>((g_rng >> 11) % N);
}
template <typename T, size_t K>
static T pick( const T (&Pool)[K] )
{
	return Pool[rnd( static_cast<uint32_t>(K) )];
}

static const wchar_t* const kProtocol[]   = { L"unreal", L"http", L"unreal", L"", L"UNREAL", L"x" };
static const wchar_t* const kDesc[]       = { L"Unreal Protocol", L"HTTP", L"" };
static const wchar_t* const kName[]       = { L"Player", L"Harry", L"", L"Default Name" };
static const wchar_t* const kMap[]        = { L"Index", L"Start", L"", L"Hub.unr" };
static const wchar_t* const kLocalMap[]   = { L"Entry", L"", L"Start.unr" };
static const wchar_t* const kHost[]       = { L"", L"localhost", L"", L"h.example.com" };
static const wchar_t* const kPortal[]     = { L"", L"Start", L"P1" };
static const wchar_t* const kMapExt[]     = { L"unr", L"UNR", L"", L"dat" };
static const wchar_t* const kSaveExt[]    = { L"usa", L"USA", L"", L"sav" };
static const wchar_t* const kPort[]       = { L"7777", L"0", L"8000", L"abc", L"-5", L"", L"12x" };
static const char* const kCfgField[10]    = { "CFG_PROTOCOL", "CFG_DESC", "CFG_NAME", "CFG_MAP", "CFG_LOCALMAP",
                                              "CFG_HOST", "CFG_PORTAL", "CFG_MAPEXT", "CFG_SAVEEXT", "CFG_PORT" };
static const wchar_t* const kCfgKey[10]   = { L"Protocol", L"ProtocolDescription", L"Name", L"Map", L"LocalMap",
                                              L"Host", L"Portal", L"MapExt", L"SaveExt", L"Port" };

static const wchar_t* const kTok[] = {
	L"unreal://", L"http://", L"://", L"/", L"//", L"\\", L"?", L"#", L":", L".", L"=", L" ", L"Name=", L"name=",
	L"NAME=", L"Team=", L"Class=", L"Skin=", L"Face=", L"Voice=", L"OverrideClass=", L"Index", L"Start", L"Map",
	L"MyMap", L".unr", L".UNR", L".usa", L"?listen", L"?PUSH", L"?pop", L"?PEER", L"?load", L"?Team=blue",
	L"?Name=Harry", L"?Class=Wiz", L"?Skin=s1", L"?Face=f", L"?Voice=v", L"?OverrideClass=X", L"?game=Foo",
	L"?restart", L"#Portal", L"192.168.0.1", L":7777", L":abc", L"x", L"0", L"a", L"B", L"Dir/", L"Save.usa",
	L"Index.unr", L"unreal", L"localhost", L"%", L"&", L"~", L"Z:", L"C:\\", L"?name", L"?Team", L"?NAME=", L"?=",
	L"?a=b=c", L"Name=Bob", L"PUSH", L"POP", L"peer"
};
static const wchar_t* const kOpt[]     = { L"Team", L"Name=Bob", L"listen", L"PUSH", L"Team=red", L"Class=C", L"x=y",
                                           L"?", L"name=", L"Skin=S", L"", L"NAME=Harry", L"Name" };
static const wchar_t* const kBaseOp[]  = { L"Team=red", L"PUSH", L"Name=Bob", L"POP", L"peer", L"Class=C", L"Face=F",
                                           L"listen", L"x=y", L"Skin=S", L"OverrideClass=Z", L"Voice=V", L"load",
                                           L"Team=", L"name", L"Name=" };
static const wchar_t* const kBHost[]   = { L"", L"h.com", L"10.0.0.1", L"h:9" };
static const wchar_t* const kBMap[]    = { L"", L"Index", L"Map.unr", L"A B" };
static const wchar_t* const kBPortal[] = { L"", L"P1" };
static const long kBPort[]             = { 0, 7777, 8000, -1 };
static const wchar_t* const kSection[] = { L"Pending", L"Startup", L"Empty", L"Opts" };
static const wchar_t* const kItem[]    = { L"Name=Bob", L"Team=red", L"PUSH", L"?listen", L"x=y", L"Class=C",
                                           L"OverrideClass=Z", L"", L"Skin=S", L"Face=F" };
static const wchar_t* const kSaveItem[] = { L"Team", L"Name", L"name", L"Class", L"x", L"", L"listen", L"Skin",
                                            L"Face", L"OverrideClass" };
static const long kCap[]               = { 0, 1, 2, 3, 4, 6, 8, 12, 16, 32, 64, 256 };

static std::wstring text_random()
{
	if( rnd(30) == 0 )
		return L"";
	std::wstring T;
	for( uint32_t N = 1 + rnd(5); N; N-- )
		T += pick(kTok);
	return T;
}

static void cfg_random( std::wstring V[10] )
{
	V[0] = pick(kProtocol);
	V[1] = pick(kDesc);
	V[2] = pick(kName);
	V[3] = pick(kMap);
	V[4] = pick(kLocalMap);
	V[5] = pick(kHost);
	V[6] = pick(kPortal);
	V[7] = pick(kMapExt);
	V[8] = pick(kSaveExt);
	V[9] = pick(kPort);
}

static void cfg_install( const std::wstring V[10] )
{
	g_cfg.clear();
	for( int I = 0; I < 10; I++ )
		g_cfg[ std::wstring(L"URL|") + kCfgKey[I] ] = V[I];
	FURL::StaticInit();
}

// Record line builder. Strings are [..] groups; integers are bare; a list is consecutive groups.
static std::wstring g_line;
static void sep( const char* Key )
{
	if( !g_line.empty() )
		g_line += L'\t';
	for( const char* P = Key; *P; ++P )
		g_line += static_cast<wchar_t>(*P);
	g_line += L'=';
}
static void Fs( const char* Key, const std::wstring& V )
{
	sep(Key);
	g_line += L'[';
	g_line += V;
	g_line += L']';
}
static void Fl( const char* Key, const std::vector<std::wstring>& V )
{
	sep(Key);
	for( const std::wstring& Item : V )
	{
		g_line += L'[';
		g_line += Item;
		g_line += L']';
	}
}
static void Fi( const char* Key, long long V )
{
	sep(Key);
	g_line += std::to_wstring(V);
}
static void Fr( const char* Key, const std::wstring& V )
{
	sep(Key);
	g_line += V;
}
static void emit_line()
{
	printf( "%ls\n", g_line.c_str() );
	g_line.clear();
}

static void cfg_emit( const std::wstring V[10] )
{
	for( int I = 0; I < 10; I++ )
		Fs( kCfgField[I], V[I] );
}

static std::vector<std::wstring> ops_of( TArray<FString>& Op )
{
	std::vector<std::wstring> Out;
	for( INT I = 0; I < Op.Num(); I++ )
		Out.push_back( *Op(I) );
	return Out;
}

static void emit_state( FURL& U )
{
	Fi( "VALID", U.Valid );
	Fs( "PROTOCOL", *U.Protocol );
	Fs( "HOST", *U.Host );
	Fi( "PORT", U.Port );
	Fs( "MAP", *U.Map );
	Fs( "PORTAL", *U.Portal );
	Fl( "OPS", ops_of(U.Op) );
	Fs( "STR0", *U.String(0) );
	Fs( "STR1", *U.String(1) );
	Fi( "ISINT", U.IsInternal() );
	Fi( "ISLOCAL", U.IsLocalInternal() );
}

struct BaseSpec
{
	bool Present = false;
	bool Valid = true;
	std::wstring Protocol, Host, Map, Portal;
	long Port = 0;
	std::vector<std::wstring> Ops;
};

static BaseSpec base_random()
{
	BaseSpec B;
	B.Present  = true;
	B.Valid    = rnd(4) != 0;
	B.Protocol = pick(kProtocol);
	B.Host     = pick(kBHost);
	B.Map      = pick(kBMap);
	B.Portal   = pick(kBPortal);
	B.Port     = pick(kBPort);
	for( uint32_t N = rnd(5); N; N-- )
		B.Ops.push_back( pick(kBaseOp) );
	return B;
}

static void base_build( FURL& F, const BaseSpec& B )
{
	F.Protocol = B.Protocol.c_str();
	F.Host     = B.Host.c_str();
	F.Map      = B.Map.c_str();
	F.Portal   = B.Portal.c_str();
	F.Port     = static_cast<INT>(B.Port);
	F.Valid    = B.Valid;
	for( const std::wstring& Op : B.Ops )
		new(F.Op) FString( Op.c_str() );
}

static void base_emit( const BaseSpec& B )
{
	Fi( "BPRESENT", B.Present );
	Fi( "BVALID", B.Valid );
	Fs( "BPROTOCOL", B.Protocol );
	Fs( "BHOST", B.Host );
	Fi( "BPORT", B.Port );
	Fs( "BMAP", B.Map );
	Fs( "BPORTAL", B.Portal );
	Fl( "BOPS", B.Ops );
}

static void emit_defaults()
{
	Fs( "DEF_PROTOCOL", *FURL::DefaultProtocol );
	Fs( "DEF_DESC", *FURL::DefaultProtocolDescription );
	Fs( "DEF_NAME", *FURL::DefaultName );
	Fs( "DEF_MAP", *FURL::DefaultMap );
	Fs( "DEF_LOCALMAP", *FURL::DefaultLocalMap );
	Fs( "DEF_HOST", *FURL::DefaultHost );
	Fs( "DEF_PORTAL", *FURL::DefaultPortal );
	Fs( "DEF_MAPEXT", *FURL::DefaultMapExt );
	Fs( "DEF_SAVEEXT", *FURL::DefaultSaveExt );
	Fi( "DEF_PORT", FURL::DefaultPort );
}

int main()
{
	// Constructor with a base (Relative/Partial always, Absolute sometimes), then option calls.
	for( int I = 0; I < 1200; I++ )
	{
		std::wstring Cfg[10];
		cfg_random(Cfg);
		cfg_install(Cfg);
		int Type = static_cast<int>(rnd(3));
		BaseSpec B = base_random();
		if( Type == 0 && rnd(2) == 0 )
			B.Present = false;
		std::wstring Text = text_random();
		std::wstring T2 = pick(kOpt);
		std::wstring T3 = pick(kOpt);

		g_line.clear();
		Fr( "KIND", L"U" );
		cfg_emit(Cfg);
		base_emit(B);
		Fi( "TYPE", Type );
		Fs( "TEXT", Text );
		Fs( "T2", T2 );
		Fs( "T3", T3 );

		g_trace.clear();
		FURL Base;
		base_build( Base, B );
		FURL R( B.Present ? &Base : nullptr, Text.c_str(), static_cast<ETravelType>(Type) );
		Fr( "TRACE", g_trace );
		emit_state(R);
		Fi( "HAS", R.HasOption( T2.c_str() ) );
		Fs( "GET", R.GetOption( T2.c_str(), L"DEF" ) );
		FURL R2 = R;
		R2.AddOption( T3.c_str() );
		Fl( "OPS2", ops_of(R2.Op) );
		Fi( "EQSELF", R == R );
		Fi( "EQ2", R == R2 );
		emit_line();
	}

	// Filename constructor.
	for( int I = 0; I < 200; I++ )
	{
		std::wstring Cfg[10];
		cfg_random(Cfg);
		cfg_install(Cfg);
		bool HasName = rnd(4) != 0;
		std::wstring Name = text_random();

		g_line.clear();
		Fr( "KIND", L"N" );
		cfg_emit(Cfg);
		Fi( "HASNAME", HasName );
		Fs( "NAME", Name );
		FURL R( HasName ? Name.c_str() : nullptr );
		emit_state(R);
		emit_line();
	}

	// LoadURLConfig over a mock section.
	for( int I = 0; I < 300; I++ )
	{
		std::wstring Cfg[10];
		cfg_random(Cfg);
		cfg_install(Cfg);
		std::wstring Section = pick(kSection);
		std::vector<std::wstring> Items;
		for( uint32_t N = rnd(5); N; N-- )
			Items.push_back( pick(kItem) );
		g_sections.clear();
		g_sections[Section] = Items;

		FURL R;
		std::vector<std::wstring> OpsIn;
		for( uint32_t N = rnd(3); N; N-- )
		{
			OpsIn.push_back( pick(kBaseOp) );
			new(R.Op) FString( OpsIn.back().c_str() );
		}

		g_line.clear();
		Fr( "KIND", L"C" );
		cfg_emit(Cfg);
		Fs( "SECTION", Section );
		Fl( "SECITEMS", Items );
		Fl( "OPS0", OpsIn );
		g_trace.clear();
		R.LoadURLConfig( Section.c_str() );
		Fr( "TRACE", g_trace );
		emit_state(R);
		emit_line();
	}

	// SaveURLConfig over an op list.
	for( int I = 0; I < 300; I++ )
	{
		std::wstring Cfg[10];
		cfg_random(Cfg);
		cfg_install(Cfg);
		std::wstring Section = rnd(2) ? L"Startup" : L"Pending";
		std::wstring Item = pick(kSaveItem);
		std::vector<std::wstring> OpsIn;

		FURL R;
		for( uint32_t N = rnd(5); N; N-- )
		{
			OpsIn.push_back( pick(kBaseOp) );
			new(R.Op) FString( OpsIn.back().c_str() );
		}

		g_line.clear();
		Fr( "KIND", L"S" );
		cfg_emit(Cfg);
		Fs( "SECTION", Section );
		Fs( "ITEM", Item );
		Fl( "OPS0", OpsIn );
		g_trace.clear();
		R.SaveURLConfig( Section.c_str(), Item.c_str(), nullptr );
		Fr( "TRACE", g_trace );
		Fl( "OPS", ops_of(R.Op) );
		emit_line();
	}

	// appFilePathToFURLToken.
	for( int I = 0; I < 400; I++ )
	{
		std::wstring Cfg[10];
		cfg_random(Cfg);
		cfg_install(Cfg);
		std::wstring Text = text_random();
		long Cap = pick(kCap);
		std::vector<TCHAR> Out( 300, 0 );
		UBOOL Ret = appFilePathToFURLToken( Text.c_str(), Out.data(), static_cast<INT>(Cap) );

		g_line.clear();
		Fr( "KIND", L"F" );
		Fs( "TEXT", Text );
		Fi( "CAP", Cap );
		Fi( "RET", Ret );
		Fs( "OUT", Out.data() );
		emit_line();
	}

	// StaticInit defaults, then StaticExit.
	for( int Mode = 0; Mode < 2; Mode++ )
	{
		for( int I = 0; I < 1; I++ )
		{
			std::wstring Cfg[10];
			cfg_random(Cfg);
			cfg_install(Cfg);
			if( Mode == 1 )
				FURL::StaticExit();
			g_line.clear();
			Fr( "KIND", L"X" );
			Fi( "MODE", Mode );
			cfg_emit(Cfg);
			emit_defaults();
			emit_line();
		}
	}
	return 0;
}
