#ifndef _INC_HP2URLSERVICES_H
#define _INC_HP2URLSERVICES_H

// C callbacks the Rust UnURL port (crates/hp2-engine/src/url.rs) calls.
// Field order must match `UrlServices` on the Rust side.
// FURL objects, FString objects and TCHAR strings are typed `void` here so this
// header stays free of Engine includes. Strings are NUL-terminated and point into
// FString or config storage that stays valid until the next mutation.
//
// Static ids (FURL::Default*): 0 Protocol, 1 ProtocolDescription, 2 Name, 3 Map,
//   4 LocalMap, 5 Host, 6 Portal, 7 MapExt, 8 SaveExt.
// Field ids (FURL member FStrings): 0 Protocol, 1 Host, 2 Map, 3 Portal.
struct Hp2UrlServices
{
	// FURL::Default<Id> data ("" when empty).
	const void* (*static_chars)( int Id );
	// FURL::Default<Id> = Chars.
	void        (*static_set_chars)( int Id, const void* Chars );
	// FURL::DefaultPort.
	int         (*default_port)( void );
	// FURL::DefaultPort = Port.
	void        (*default_port_set)( int Port );
	// GConfig->GetStr(Section, Key).
	const void* (*config_get_str)( const void* Section, const void* Key );
	// GConfig->GetSection(Section, Value, Size, Filename).
	void        (*config_get_section)( const void* Section, void* Value, int Size, const void* Filename );
	// GConfig->SetString(Section, Key, Value, Filename).
	void        (*config_set_string)( const void* Section, const void* Key, const void* Value, const void* Filename );
	// appStricmp(A, B).
	int         (*str_icmp)( const void* A, const void* B );
	// appStrnicmp(A, B, Count).
	int         (*str_nicmp)( const void* A, const void* B, int Count );
	// appAtoi(Str).
	int         (*str_atoi)( const void* Str );
	// FURL instance reads (Url is FURL*).
	const void* (*url_field)( const void* Url, int Field );
	int         (*url_port)( const void* Url );
	int         (*url_valid)( const void* Url );
	int         (*url_op_count)( const void* Url );
	const void* (*url_op)( const void* Url, int Index );
	// FURL instance writes.
	void        (*url_set_field)( void* Url, int Field, const void* Chars );
	void        (*url_set_port)( void* Url, int Port );
	void        (*url_set_valid)( void* Url, int Valid );
	void        (*url_op_clear)( void* Url );
	// new(Op)FString(Chars).
	void        (*url_op_push)( void* Url, const void* Chars );
	// Op(Index) = Chars.
	void        (*url_op_set)( void* Url, int Index, const void* Chars );
	// FString& Str = Chars (Str is FString*).
	void        (*fstr_set)( void* Str, const void* Chars );
	// debugf(TEXT("URL: Adding default option %s"), Chars).
	void        (*log_default_option)( const void* Chars );
};

extern const Hp2UrlServices GHp2UrlServices;

#endif
