/*=============================================================================
	FFileManagerUnix.h: POSIX file manager for Unix-family hosts.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.
=============================================================================*/

#ifndef INC_FFILEMANAGERUNIX_H
#define INC_FFILEMANAGERUNIX_H

#include <cerrno>
#include <cstdio>
#include <cstdint>
#include <cstring>
#include <cwctype>
#include <dirent.h>
#include <fnmatch.h>
#include <string>
#include <sys/stat.h>
#include <sys/time.h>
#include <sys/types.h>
#include <strings.h>
#include <unistd.h>
#include <utime.h>
#include <vector>
#include "FFileManagerGeneric.h"

namespace
{
	static UBOOL EncodeUnixArchiveName(
		const TCHAR* Source,
		UNICHAR* Wire,
		INT WireCapacity,
		INT& WireUnits)
	{
		if( !Source || !Wire || WireCapacity <= 0 )
			return 0;
		WireUnits = 0;
		for( INT Index = 0; Source[Index]; ++Index )
		{
			DWORD CodePoint = static_cast<DWORD>(Source[Index]);
			if( sizeof(TCHAR) == sizeof(UNICHAR) )
			{
				if( CodePoint >= 0xd800u && CodePoint <= 0xdbffu )
				{
					const DWORD Low = static_cast<DWORD>(Source[++Index]);
					if( Low < 0xdc00u || Low > 0xdfffu || WireUnits > WireCapacity - 3 )
						return 0;
					Wire[WireUnits++] = static_cast<UNICHAR>(CodePoint);
					Wire[WireUnits++] = static_cast<UNICHAR>(Low);
					continue;
				}
				if( CodePoint >= 0xdc00u && CodePoint <= 0xdfffu )
					return 0;
			}
			else if( CodePoint >= 0xd800u && CodePoint <= 0xdfffu )
			{
				return 0;
			}

			if( CodePoint <= 0xffffu )
			{
				if( WireUnits > WireCapacity - 2 )
					return 0;
				Wire[WireUnits++] = static_cast<UNICHAR>(CodePoint);
			}
			else if( CodePoint <= 0x10ffffu )
			{
				if( WireUnits > WireCapacity - 3 )
					return 0;
				CodePoint -= 0x10000u;
				Wire[WireUnits++] = static_cast<UNICHAR>(0xd800u + (CodePoint >> 10));
				Wire[WireUnits++] = static_cast<UNICHAR>(0xdc00u + (CodePoint & 0x3ffu));
			}
			else
			{
				return 0;
			}
		}
		Wire[WireUnits++] = 0;
		return 1;
	}

	static UBOOL DecodeUnixArchiveName(
		const UNICHAR* Wire,
		INT WireUnits,
		TCHAR* Dest,
		INT DestCapacity)
	{
		if( !Wire || !Dest || WireUnits <= 0 || Wire[WireUnits - 1] != 0
			|| DestCapacity <= 0 )
			return 0;
		INT DestUnits = 0;
		for( INT Index = 0; Index < WireUnits - 1; ++Index )
		{
			DWORD CodePoint = static_cast<DWORD>(Wire[Index]);
			if( CodePoint >= 0xd800u && CodePoint <= 0xdbffu )
			{
				if( Index + 1 >= WireUnits - 1 )
					return 0;
				const DWORD Low = static_cast<DWORD>(Wire[++Index]);
				if( Low < 0xdc00u || Low > 0xdfffu )
					return 0;
				if( sizeof(TCHAR) == sizeof(UNICHAR) )
				{
					if( DestUnits > DestCapacity - 3 )
						return 0;
					Dest[DestUnits++] = static_cast<TCHAR>(CodePoint);
					Dest[DestUnits++] = static_cast<TCHAR>(Low);
					continue;
				}
				CodePoint = 0x10000u
					+ ((CodePoint - 0xd800u) << 10)
					+ (Low - 0xdc00u);
			}
			else if( CodePoint >= 0xdc00u && CodePoint <= 0xdfffu )
			{
				return 0;
			}
			if( DestUnits > DestCapacity - 2 )
				return 0;
			Dest[DestUnits++] = static_cast<TCHAR>(CodePoint);
		}
		Dest[DestUnits] = 0;
		return 1;
	}
}


class FArchiveFileReaderUnix : public FArchive
{
public:
	FArchiveFileReaderUnix( FILE* InFile, FOutputDevice* InError, off_t InSize )
	: File(InFile), Error(InError ? InError : GNull), Size(InSize), Pos(0)
	{
		ArIsLoading = ArIsPersistent = 1;
	}
	~FArchiveFileReaderUnix() { if( File ) Close(); }
	void Seek( INT InPos )
	{
		if( InPos < 0 || (off_t)InPos > Size ) { Fail(TEXT("Invalid file-reader seek")); return; }
		if( fseeko(File,(off_t)InPos,SEEK_SET) != 0 ) { Fail(TEXT("POSIX file-reader seek failed")); return; }
		Pos = (off_t)InPos;
	}
	INT Tell() { return Narrow(Pos,TEXT("File-reader position exceeds the archive INT range")); }
	INT TotalSize() { return Narrow(Size,TEXT("File-reader size exceeds the archive INT range")); }
	UBOOL Close()
	{
		if( File )
		{
			if( fclose(File) != 0 ) Fail(TEXT("POSIX file-reader close failed"));
			File = NULL;
		}
		return !ArIsError;
	}
	void Serialize( void* V, INT Length )
	{
		if( ArIsError ) return;
		if( Length < 0 || Pos < 0 || Pos > (off_t)MAXINT || (off_t)Length > (off_t)MAXINT-Pos )
			{ Fail(TEXT("File-reader request exceeds the archive INT range")); return; }
		if( Pos+(off_t)Length > Size ) { Fail(TEXT("File-reader request extends beyond end of file")); return; }
		if( Length == 0 ) return;
		size_t Done = fread(V,1,(size_t)Length,File);
		Pos += (off_t)Done;
		if( Done != (size_t)Length ) Fail(ferror(File) ? TEXT("POSIX file-reader read failed") : TEXT("Unexpected end of file"));
	}
	FArchive& operator<<( FName& Name )
	{
		UNICHAR Wire[NAME_SIZE];
		TCHAR Dest[NAME_SIZE];
		INT WireBytes = 0;
		ByteOrderSerialize(&WireBytes,static_cast<INT>(sizeof(WireBytes)));
		if( ArIsError )
			return *this;
		if( WireBytes < static_cast<INT>(sizeof(UNICHAR))
			|| (WireBytes % static_cast<INT>(sizeof(UNICHAR))) != 0
			|| WireBytes > static_cast<INT>(sizeof(Wire)) )
		{
			Fail(TEXT("Invalid serialized FName length"));
			return *this;
		}
		const INT WireUnits = WireBytes / static_cast<INT>(sizeof(UNICHAR));
		for( INT Index = 0; Index < WireUnits; ++Index )
			ByteOrderSerialize(&Wire[Index],static_cast<INT>(sizeof(Wire[Index])));
		if( ArIsError )
			return *this;
		if( !DecodeUnixArchiveName(Wire,WireUnits,Dest,ARRAY_COUNT(Dest)) )
		{
			Fail(TEXT("Invalid serialized FName text"));
			return *this;
		}
		FName LoadedName(Dest);
		if( !LoadedName.IsValid() )
			Fail(TEXT("Serialized FName is not valid"));
		else
			Name = LoadedName;
		return *this;
	}
private:
	INT Narrow( off_t Value, const TCHAR* Message )
	{
		if( Value < 0 || Value > (off_t)MAXINT ) { Fail(Message); return INDEX_NONE; }
		return (INT)Value;
	}
	void Fail( const TCHAR* Message )
	{
		ArIsError = 1;
		if( Error ) Error->Logf(TEXT("%s (errno=%i)"),Message,errno);
	}
	FILE* File;
	FOutputDevice* Error;
	off_t Size;
	off_t Pos;
};

class FArchiveFileWriterUnix : public FArchive
{
public:
	FArchiveFileWriterUnix( FILE* InFile, FOutputDevice* InError, off_t InPos )
	: File(InFile), Error(InError ? InError : GNull), Pos(InPos)
	{
		ArIsSaving = ArIsPersistent = 1;
	}
	~FArchiveFileWriterUnix() { if( File ) Close(); }
	void Seek( INT InPos )
	{
		if( InPos < 0 ) { Fail(TEXT("Invalid file-writer seek")); return; }
		Flush();
		if( ArIsError ) return;
		if( fseeko(File,(off_t)InPos,SEEK_SET) != 0 ) { Fail(TEXT("POSIX file-writer seek failed")); return; }
		Pos = (off_t)InPos;
	}
	INT Tell()
	{
		if( Pos < 0 || Pos > (off_t)MAXINT ) { Fail(TEXT("File-writer position exceeds the archive INT range")); return INDEX_NONE; }
		return (INT)Pos;
	}
	UBOOL Close()
	{
		if( File )
		{
			Flush();
			if( fclose(File) != 0 ) Fail(TEXT("POSIX file-writer close failed"));
			File = NULL;
		}
		return !ArIsError;
	}
	void Serialize( void* V, INT Length )
	{
		if( ArIsError ) return;
		if( Length < 0 || Pos < 0 || Pos > (off_t)MAXINT || (off_t)Length > (off_t)MAXINT-Pos )
			{ Fail(TEXT("File-writer request exceeds the archive INT range")); return; }
		if( Length == 0 ) return;
		size_t Done = fwrite(V,1,(size_t)Length,File);
		Pos += (off_t)Done;
		if( Done != (size_t)Length ) Fail(TEXT("POSIX file-writer write failed"));
	}
	void Flush() { if( File && fflush(File) != 0 ) Fail(TEXT("POSIX file-writer flush failed")); }
	FArchive& operator<<( FName& Name )
	{
		UNICHAR Wire[NAME_SIZE];
		INT WireUnits = 0;
		if( !EncodeUnixArchiveName(*Name,Wire,ARRAY_COUNT(Wire),WireUnits) )
		{
			Fail(TEXT("FName does not fit the serialized wire format"));
			return *this;
		}
		INT WireBytes = WireUnits * static_cast<INT>(sizeof(UNICHAR));
		ByteOrderSerialize(&WireBytes,static_cast<INT>(sizeof(WireBytes)));
		for( INT Index = 0; Index < WireUnits && !ArIsError; ++Index )
			ByteOrderSerialize(&Wire[Index],static_cast<INT>(sizeof(Wire[Index])));
		return *this;
	}
private:
	void Fail( const TCHAR* Message )
	{
		ArIsError = 1;
		if( Error ) Error->Logf(TEXT("%s (errno=%i)"),Message,errno);
	}
	FILE* File;
	FOutputDevice* Error;
	off_t Pos;
};

class FFileManagerUnix : public FFileManagerGeneric
{
	typedef std::basic_string<TCHAR> FNativeString;
public:
	FArchive* CreateFileReader( const TCHAR* Filename, DWORD Flags=0, FOutputDevice* Error=GNull )
	{
		std::string Path;
		if( !ToPath(Filename,Path) ) return ReaderFailure(Filename,Flags,Error,TEXT("Invalid Unicode file name"));
		FILE* File = fopen(Path.c_str(),"rb");
		if( !File && MissingError() )
		{
			std::string Recovered; UBOOL Ambiguous=0;
			if( RecoverPath(Path,0,Recovered,Ambiguous) ) File=fopen(Recovered.c_str(),"rb");
		}
		if( !File ) return ReaderFailure(Filename,Flags,Error,TEXT("Failed to open file for reading"));
		if( fseeko(File,0,SEEK_END) != 0 ) { fclose(File); return ReaderFailure(Filename,Flags,Error,TEXT("Failed to size file")); }
		off_t Size=ftello(File);
		if( Size < 0 || Size > (off_t)MAXINT || fseeko(File,0,SEEK_SET) != 0 )
			{ fclose(File); return ReaderFailure(Filename,Flags,Error,TEXT("File exceeds the archive INT contract")); }
		return new(TEXT("UnixFileReader")) FArchiveFileReaderUnix(File,Error,Size);
	}
	FArchive* CreateFileWriter( const TCHAR* Filename, DWORD Flags=0, FOutputDevice* Error=GNull )
	{
		std::string Path;
		if( !ToPath(Filename,Path) ) return WriterFailure(Filename,Flags,Error,TEXT("Invalid Unicode file name"));
		std::string Existing; UBOOL Ambiguous=0; UBOOL Exists=ExistingPath(Path,Existing,Ambiguous);
		if( Ambiguous ) return WriterFailure(Filename,Flags,Error,TEXT("Ambiguous case-insensitive file name"));
		if( (Flags&FILEWRITE_NoReplaceExisting) && Exists ) return NULL;
		if( (Flags&FILEWRITE_EvenIfReadOnly) && Exists ) MakeWritable(Existing);
		std::string WritePath;
		if( Exists )
			WritePath=Existing;
		else if( !RecoverPath(Path,1,WritePath,Ambiguous) || Ambiguous )
			return WriterFailure(Filename,Flags,Error,TEXT("Failed to resolve output file name"));
		const char* Mode=(Flags&FILEWRITE_Append) ? "ab" : "wb";
		FILE* File=fopen(WritePath.c_str(),Mode);
		if( !File ) return WriterFailure(Filename,Flags,Error,TEXT("Failed to open file for writing"));
		if( Flags&FILEWRITE_Unbuffered ) setvbuf(File,NULL,_IONBF,0);
		off_t Pos=0;
		if( Flags&FILEWRITE_Append )
		{
			if( fseeko(File,0,SEEK_END) != 0 || (Pos=ftello(File)) < 0 || Pos > (off_t)MAXINT )
				{ fclose(File); return WriterFailure(Filename,Flags,Error,TEXT("Append position exceeds archive INT contract")); }
		}
		return new(TEXT("UnixFileWriter")) FArchiveFileWriterUnix(File,Error,Pos);
	}
	INT FileSize( const TCHAR* Filename )
	{
		std::string Path; struct stat Info;
		if( !ToPath(Filename,Path) || !StatExisting(Path,Info,Path) ) return -1;
		if( Info.st_size < 0 || Info.st_size > (off_t)MAXINT )
		{
			if( GLog ) GLog->Logf(NAME_Warning,TEXT("File size exceeds archive INT contract: %s"),Filename);
			return -1;
		}
		return (INT)Info.st_size;
	}
	UBOOL Copy( const TCHAR* DestFile, const TCHAR* SrcFile, UBOOL ReplaceExisting=1, UBOOL EvenIfReadOnly=0, UBOOL Attributes=0, void (*Progress)(FLOAT)=NULL )
	{
		if( Progress ) Progress(0.f);
		FArchive* Src=CreateFileReader(SrcFile,0,GNull);
		if( !Src ) { if(Progress) Progress(1.f); return 0; }
		FArchive* Dest=CreateFileWriter(DestFile,(ReplaceExisting?0:FILEWRITE_NoReplaceExisting)|(EvenIfReadOnly?FILEWRITE_EvenIfReadOnly:0),GNull);
		if( !Dest ) { Src->Close(); delete Src; if(Progress) Progress(1.f); return 0; }
		INT Size=Src->TotalSize(), Total=0; std::vector<BYTE> Buffer(64*1024);
		while( Total<Size && !Src->IsError() && !Dest->IsError() )
		{
			INT Count=Min(Size-Total,(INT)Buffer.size());
			Src->Serialize(&Buffer[0],Count);
			if( !Src->IsError() ) Dest->Serialize(&Buffer[0],Count);
			Total+=Count;
			if( Progress && Size ) Progress((FLOAT)Total/(FLOAT)Size);
		}
		UBOOL Success=Total==Size && !Src->IsError() && !Dest->IsError();
		Success=Dest->Close() && Success; Success=Src->Close() && Success;
		delete Dest; delete Src;
		if( !Success ) Delete(DestFile,0,1); else if(Attributes) Success=CopyAttributes(DestFile,SrcFile);
		if( Progress ) Progress(1.f);
		return Success;
	}
	UBOOL Move( const TCHAR* Dest, const TCHAR* Src, UBOOL Replace=1, UBOOL EvenIfReadOnly=0, UBOOL Attributes=0 )
	{
		std::string S,D;
		if( !ToPath(Src,S) || !ToPath(Dest,D) ) return 0;
		std::string ExistingS,ExistingD; UBOOL Ambiguous=0;
		if( !ExistingPath(S,ExistingS,Ambiguous) || Ambiguous ) return 0;
		UBOOL Exists=ExistingPath(D,ExistingD,Ambiguous);
		if( Ambiguous || (!Replace&&Exists) ) return 0;
		if( EvenIfReadOnly&&Exists ) MakeWritable(ExistingD);
		if( !Exists && (!RecoverPath(D,1,ExistingD,Ambiguous)||Ambiguous) ) return 0;
		if( rename(ExistingS.c_str(),ExistingD.c_str())==0 ) return 1;
		if( errno==EXDEV ) return Copy(Dest,Src,Replace,EvenIfReadOnly,Attributes,NULL)&&Delete(Src,1,1);
		return 0;
	}
	UBOOL Delete( const TCHAR* Filename, UBOOL RequireExists=0, UBOOL EvenReadOnly=0 )
	{
		std::string Path;
		if( !ToPath(Filename,Path) ) return 0;
		if( EvenReadOnly ) MakeWritable(Path);
		if( unlink(Path.c_str())==0 ) return 1;
		if( !MissingError() ) return 0;
		std::string Recovered; UBOOL Ambiguous=0;
		if( !RecoverPath(Path,0,Recovered,Ambiguous) ) return !RequireExists&&!Ambiguous;
		if( EvenReadOnly ) MakeWritable(Recovered);
		return unlink(Recovered.c_str())==0 || (!RequireExists&&errno==ENOENT);
	}
	SQWORD GetGlobalTime( const TCHAR* Filename )
	{
		std::string Path; struct stat Info;
		return ToPath(Filename,Path)&&StatExisting(Path,Info,Path) ? (SQWORD)Info.st_mtime : 0;
	}
	UBOOL SetGlobalTime( const TCHAR* Filename )
	{
		std::string Path;
		if( !ToPath(Filename,Path) ) return 0;
		if( utime(Path.c_str(),NULL)==0 ) return 1;
		if( !MissingError() ) return 0;
		std::string Recovered; UBOOL Ambiguous=0;
		return RecoverPath(Path,0,Recovered,Ambiguous)&&utime(Recovered.c_str(),NULL)==0;
	}
	UBOOL MakeDirectory( const TCHAR* Path, UBOOL Tree=0 )
	{
		std::string Utf8;
		if( !ToPath(Path,Utf8) ) return 0;
		if( Tree ) return MakeDirectoryTree(Utf8);
		if( mkdir(Utf8.c_str(),DirectoryMode())==0 ) return 1;
		if( errno==EEXIST&&IsDirectory(Utf8) ) return 1;
		if( !MissingError() ) return 0;
		std::string Recovered; UBOOL Ambiguous=0;
		return RecoverPath(Utf8,0,Recovered,Ambiguous)&&IsDirectory(Recovered);
	}
	UBOOL DeleteDirectory( const TCHAR* Path, UBOOL RequireExists=0, UBOOL Tree=0 )
	{
		std::string Utf8;
		if( !ToPath(Path,Utf8) ) return 0;
		if( !Tree&&rmdir(Utf8.c_str())==0 ) return 1;
		if( !Tree&&!MissingError() ) return 0;
		std::string Recovered; UBOOL Ambiguous=0;
		if( !RecoverPath(Utf8,0,Recovered,Ambiguous) ) return !RequireExists&&!Ambiguous;
		return Tree ? RemoveDirectoryTree(Recovered) : (rmdir(Recovered.c_str())==0||(!RequireExists&&errno==ENOENT));
	}
	TArray<FString> FindFiles( const TCHAR* Filename, UBOOL Files, UBOOL Directories )
	{
		TArray<FString> Result; std::string Spec;
		if( !ToPath(Filename,Spec) ) return Result;
		std::string::size_type Slash=Spec.find_last_of('/');
		std::string Dir=Slash==std::string::npos?".":(Slash==0?"/":Spec.substr(0,Slash));
		std::string Pattern=Slash==std::string::npos?Spec:Spec.substr(Slash+1);
		if( Pattern.empty() ) Pattern="*";
		DIR* Handle=opendir(Dir.c_str());
		if( !Handle&&MissingError() )
		{
			std::string Recovered; UBOOL Ambiguous=0;
			if( RecoverPath(Dir,0,Recovered,Ambiguous) ) { Dir=Recovered; Handle=opendir(Dir.c_str()); }
		}
		if( !Handle ) return Result;
		while( struct dirent* Entry=readdir(Handle) )
		{
			if( !strcmp(Entry->d_name,".")||!strcmp(Entry->d_name,"..")||fnmatch(Pattern.c_str(),Entry->d_name,0)!=0 ) continue;
			struct stat Info; std::string Full=Join(Dir,Entry->d_name);
			if( lstat(Full.c_str(),&Info)!=0 ) continue;
			UBOOL IsDir=S_ISDIR(Info.st_mode);
			if( (IsDir&&!Directories)||(!IsDir&&!Files) ) continue;
			FNativeString Native;
			if( Utf8ToNative(Entry->d_name,Native) ) new(Result) FString(Native.c_str());
		}
		closedir(Handle); return Result;
	}
	UBOOL SetDefaultDirectory( const TCHAR* Filename )
	{
		std::string Path;
		if( !ToPath(Filename,Path) ) return 0;
		if( chdir(Path.c_str())==0 ) return 1;
		if( !MissingError() ) return 0;
		std::string Recovered; UBOOL Ambiguous=0;
		return RecoverPath(Path,0,Recovered,Ambiguous)&&chdir(Recovered.c_str())==0;
	}
	FString GetDefaultDirectory()
	{
		std::vector<char> Buffer(256);
		for(;;)
		{
			if( getcwd(&Buffer[0],Buffer.size()) )
			{
				FNativeString Native;
				return Utf8ToNative(&Buffer[0],Native)?FString(Native.c_str()):FString(TEXT(""));
			}
			if( errno!=ERANGE ) return FString(TEXT(""));
			Buffer.resize(Buffer.size()*2);
		}
	}
private:
	static FArchive* ReaderFailure( const TCHAR* Name, DWORD Flags, FOutputDevice* Error, const TCHAR* Message )
	{
		if( Flags&FILEREAD_NoFail ) appErrorf(TEXT("%s: %s"),Message,Name);
		if( Error ) Error->Logf(TEXT("%s: %s"),Message,Name);
		return NULL;
	}
	static FArchive* WriterFailure( const TCHAR* Name, DWORD Flags, FOutputDevice* Error, const TCHAR* Message )
	{
		if( Flags&FILEWRITE_NoFail ) appErrorf(TEXT("%s: %s"),Message,Name);
		if( Error ) Error->Logf(TEXT("%s: %s"),Message,Name);
		return NULL;
	}
	static UBOOL MissingError() { return errno==ENOENT||errno==ENOTDIR; }
	static mode_t DirectoryMode() { return S_IRUSR|S_IWUSR|S_IXUSR|S_IRGRP|S_IXGRP|S_IROTH|S_IXOTH; }
	static UBOOL ToPath( const TCHAR* Input, std::string& Output )
	{
		return appToUtf8NativePath(Input,Output);
	}
	static UBOOL Utf8ToNative( const char* Input, FNativeString& Output )
	{
		Output.clear(); if(!Input) return 0;
#if UNICODE
		const unsigned char* P=(const unsigned char*)Input;
		while(*P)
		{
			std::uint32_t C=0,Min=0; INT Extra=0;
			if(*P<0x80) C=*P++;
			else if((*P&0xe0)==0xc0) { C=*P++&31; Min=0x80; Extra=1; }
			else if((*P&0xf0)==0xe0) { C=*P++&15; Min=0x800; Extra=2; }
			else if((*P&0xf8)==0xf0) { C=*P++&7; Min=0x10000; Extra=3; }
			else return 0;
			for(INT I=0;I<Extra;++I) { if((*P&0xc0)!=0x80) return 0; C=(C<<6)|(*P++&63); }
			if(C<Min||C>0x10ffffu||(C>=0xd800u&&C<=0xdfffu)) return 0;
			if(sizeof(TCHAR)==sizeof(UNICHAR) && C>0xffffu)
			{
				C-=0x10000u;
				Output.push_back((TCHAR)(0xd800u+(C>>10)));
				Output.push_back((TCHAR)(0xdc00u+(C&0x3ffu)));
			}
			else Output.push_back((TCHAR)C);
		}
#else
		Output.assign(Input);
#endif
		return 1;
	}
	static UBOOL CaseEqual( const char* A, const char* B )
	{
#if UNICODE
		FNativeString L,R;
		if(!Utf8ToNative(A,L)||!Utf8ToNative(B,R)||L.size()!=R.size()) return 0;
		for(size_t I=0;I<L.size();++I) if(std::towlower((wint_t)L[I])!=std::towlower((wint_t)R[I])) return 0;
		return 1;
#else
		return strcasecmp(A,B)==0;
#endif
	}
	static std::vector<std::string> Parts( const std::string& Path )
	{
		std::vector<std::string> Out; size_t P=0;
		while(P<Path.size()) { while(P<Path.size()&&Path[P]=='/') ++P; size_t E=P; while(E<Path.size()&&Path[E]!='/') ++E; if(E>P) Out.push_back(Path.substr(P,E-P)); P=E; }
		return Out;
	}
	static std::string Join( const std::string& Dir, const std::string& Name )
	{
		if(Dir=="/") return Dir+Name;
		return Dir.empty()?Name:Dir+"/"+Name;
	}
	static void LogAmbiguous( const std::string& Dir, const std::string& Part )
	{
		if(!GLog) return; FNativeString D,P;
		if(Utf8ToNative(Dir.c_str(),D)&&Utf8ToNative(Part.c_str(),P)) GLog->Logf(NAME_Warning,TEXT("Ambiguous case-insensitive match for '%s' in '%s'"),P.c_str(),D.c_str());
		else GLog->Logf(NAME_Warning,TEXT("Ambiguous case-insensitive filesystem match"));
	}
	static UBOOL RecoverPath( const std::string& Input, UBOOL MissingLeaf, std::string& Output, UBOOL& Ambiguous )
	{
		Ambiguous=0; std::vector<std::string> Ps=Parts(Input); std::string Current=!Input.empty()&&Input[0]=='/'?"/":".";
		for(size_t I=0;I<Ps.size();++I)
		{
			const std::string& Part=Ps[I];
			if(Part=="."||Part=="..") { Current=Join(Current,Part); continue; }
			std::string Candidate=Join(Current,Part); struct stat Info;
			if(lstat(Candidate.c_str(),&Info)==0) { Current=Candidate; continue; }
			DIR* H=opendir(Current.c_str()); if(!H) return 0;
			std::string Match; INT Count=0;
			while(struct dirent* E=readdir(H)) if(CaseEqual(E->d_name,Part.c_str())) { Match=E->d_name; ++Count; }
			closedir(H);
			if(Count>1) { Ambiguous=1; LogAmbiguous(Current,Part); return 0; }
			if(Count==1) { Current=Join(Current,Match); continue; }
			if(MissingLeaf&&I+1==Ps.size()) { Current=Candidate; continue; }
			return 0;
		}
		Output=Current; return 1;
	}
	static UBOOL ExistingPath( const std::string& Path, std::string& Existing, UBOOL& Ambiguous )
	{
		struct stat Info; if(lstat(Path.c_str(),&Info)==0) { Existing=Path; Ambiguous=0; return 1; }
		return RecoverPath(Path,0,Existing,Ambiguous);
	}
	static UBOOL StatExisting( const std::string& Path, struct stat& Info, std::string& Existing )
	{
		if(stat(Path.c_str(),&Info)==0) { Existing=Path; return 1; }
		if(!MissingError()) return 0; UBOOL Ambiguous=0;
		return RecoverPath(Path,0,Existing,Ambiguous)&&stat(Existing.c_str(),&Info)==0;
	}
	static UBOOL IsDirectory( const std::string& Path ) { struct stat I; return stat(Path.c_str(),&I)==0&&S_ISDIR(I.st_mode); }
	static void MakeWritable( const std::string& Path ) { struct stat I; if(stat(Path.c_str(),&I)==0) chmod(Path.c_str(),I.st_mode|S_IRUSR|S_IWUSR); }
	static UBOOL MakeDirectoryTree( const std::string& Input )
	{
		std::vector<std::string> Ps=Parts(Input); std::string Current=!Input.empty()&&Input[0]=='/'?"/":".";
		for(size_t I=0;I<Ps.size();++I)
		{
			if(Ps[I]=="."||Ps[I]=="..") { Current=Join(Current,Ps[I]); continue; }
			std::string C=Join(Current,Ps[I]); struct stat Info;
			if(stat(C.c_str(),&Info)==0) { if(!S_ISDIR(Info.st_mode)) return 0; Current=C; continue; }
			std::string R; UBOOL A=0;
			if(RecoverPath(C,0,R,A)) { if(!IsDirectory(R)) return 0; Current=R; continue; }
			if(A||mkdir(C.c_str(),DirectoryMode())!=0) return 0; Current=C;
		}
		return 1;
	}
	static UBOOL RemoveDirectoryTree( const std::string& Dir )
	{
		DIR* H=opendir(Dir.c_str()); if(!H) return 0; UBOOL Ok=1;
		while(Ok) { struct dirent* E=readdir(H); if(!E) break; if(!strcmp(E->d_name,".")||!strcmp(E->d_name,"..")) continue; std::string C=Join(Dir,E->d_name); struct stat I; if(lstat(C.c_str(),&I)!=0) Ok=0; else Ok=S_ISDIR(I.st_mode)?RemoveDirectoryTree(C):unlink(C.c_str())==0; }
		if(closedir(H)!=0) Ok=0; return Ok&&rmdir(Dir.c_str())==0;
	}
	static UBOOL CopyAttributes( const TCHAR* DestFile, const TCHAR* SrcFile )
	{
		std::string D,S,RD,RS; UBOOL A=0;
		if(!ToPath(DestFile,D)||!ToPath(SrcFile,S)||!ExistingPath(D,RD,A)||A||!ExistingPath(S,RS,A)||A) return 0;
		struct stat I; if(stat(RS.c_str(),&I)!=0||chmod(RD.c_str(),I.st_mode&07777)!=0) return 0;
		struct timeval T[2];
#if defined(__APPLE__)
		T[0].tv_sec=I.st_atimespec.tv_sec; T[0].tv_usec=static_cast<__darwin_suseconds_t>(I.st_atimespec.tv_nsec/1000);
		T[1].tv_sec=I.st_mtimespec.tv_sec; T[1].tv_usec=static_cast<__darwin_suseconds_t>(I.st_mtimespec.tv_nsec/1000);
#else
		T[0].tv_sec=I.st_atime; T[0].tv_usec=0; T[1].tv_sec=I.st_mtime; T[1].tv_usec=0;
#endif
		return utimes(RD.c_str(),T)==0;
	}
};

#endif
