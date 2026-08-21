/*=============================================================================
	UPkgCommandlet.cpp: Imports/Exports data to/from packages.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Warren Marshall.
=============================================================================*/

#include "EditorPrivate.h"

/*-----------------------------------------------------------------------------
	UPkgCommandlet.
-----------------------------------------------------------------------------*/

enum eTYPE {
	eTYPE_TEXTURE	= 0,
	eTYPE_SOUND		= 1,
} GType;

class UPkgCommandlet : public UCommandlet
{
	DECLARE_CLASS(UPkgCommandlet,UCommandlet,CLASS_Transient,Editor);
	void StaticConstructor()
	{
		guard(StaticConstructor::StaticConstructor);

		LogToStdout     = 0;
		IsClient        = 1;
		IsEditor        = 1;
		IsServer        = 1;
		LazyLoad        = 1;
		ShowErrorCount  = 0;

		unguard;
	}
	INT Main( const TCHAR* Parms )
	{
		guard(UPkgCommandlet::Main);

		// Create the editor class.
		UClass* EditorEngineClass = UObject::StaticLoadClass( UEditorEngine::StaticClass(), NULL, TEXT("ini:Engine.Engine.EditorEngine"), NULL, LOAD_NoFail | LOAD_DisallowFiles, NULL );
		GEditor  = ConstructObject<UEditorEngine>( EditorEngineClass );
		GEditor->UseSound = 1;
		GEditor->Init();
		GIsRequestingExit = 1; // Causes ctrl-c to immediately exit.

		FString Command, Type, PkgPath, PkgName, PkgDir, Dir, Compress, LipSync;

		// The first param is still the package name when run from the debugger - kill it.  TG ALPHA
		//
#if 0
		FString Dummy;
		if( !ParseToken(Parms, Dummy, 0) )
			appErrorf(TEXT("Command not specified."));
#endif

		// Make sure we got all params.
		if( !ParseToken(Parms,Command,0) )
			appErrorf(TEXT("Command not specified."));

		if( !ParseToken(Parms,Type,0) )
			appErrorf(TEXT("Type not specified"));
		if( !ParseToken(Parms,PkgPath,0) )
			appErrorf(TEXT("Package not specified"));
		if( !ParseToken(Parms,Dir,0) )
			appErrorf(TEXT("Directory not specified"));

		if (Type == TEXT("Sound"))
		{
			//	Add support for compression and lip sync data -tg
			//
			Compress = TEXT("nocompress");
			LipSync = TEXT("nolipsync");

			if( !ParseToken(Parms, Compress, 0) )
				GWarn->Logf( TEXT("Compression parameter not specified - Defaulting to no compression") );
			if( !ParseToken(Parms, LipSync, 0) )
				GWarn->Logf( TEXT("LipSync parameter not specified - Defaulting to no lip sync data") );
		}

		// Parse the package name 
		//
		if( PkgPath.InStr( TEXT("\\") ) != -1 )
		{
			PkgName = PkgPath.Right( PkgPath.Len() - PkgPath.InStr( TEXT("\\"), 1 ) - 1 );
			PkgDir = PkgPath.Left( PkgPath.InStr( TEXT("\\"), 1 ) );
			if( !PkgDir.Len() )
				PkgDir = TEXT(".");
		}
		else
		{
			PkgName = PkgPath;
			PkgDir = TEXT(".");
		}

		UClass* Class = FindObjectChecked<UClass>( ANY_PACKAGE, *Type );

		// Make sure options are valid.
		//
		if( Command != TEXT("import") && Command != TEXT("export") )
			appErrorf(TEXT("Command not valid."));
		if( !Class )
			appErrorf(TEXT("Type not valid."));

		//	Make sure optional sound options are valid	-tg
		//
		//	NOTE:  In the future, may want to add support for parameter like "compress=xa" to enable
		//	different compression technologies.  For now, always use XA is "compress" specified
		//
		if (Type == TEXT("Sound"))
		{
			if( Compress != TEXT("compress") && Compress != TEXT("nocompress") )
				appErrorf(TEXT("Compression parameter  not valid - use \"compress\" or \"nocompress\"."));
			if( LipSync != TEXT("lipsync") && LipSync != TEXT("nolipsync") )
				appErrorf(TEXT("Lip Sync parameter  not valid - use \"lipsync\" or \"nolipsync\"."));
		}

		FString Ext, PkgExt;
		if( Type == TEXT("Texture") )		{ GType = eTYPE_TEXTURE;	Ext = TEXT("pcx");	PkgExt = TEXT("utx"); }
		else if( Type == TEXT("Sound") )	{ GType = eTYPE_SOUND;		Ext = TEXT("wav");	PkgExt = TEXT("uax"); }

		// Do it.
		if( Command == TEXT("export") )
		{
			UObject* Package = LoadPackage(NULL,*(PkgDir + TEXT("\\") + PkgName + TEXT(".") + PkgExt),LOAD_NoFail);

			GFileManager->MakeDirectory( *Dir );

			for( TObjectIterator<UObject> It; It; ++It )
			{
				if( It->IsA(Class) && It->IsIn(Package) )
				{
					FString Filename;
					FString Params;
					FString Group = It->GetFullName();

					switch( GType )
					{
						case eTYPE_TEXTURE:
							{
								Params = FString::Printf( TEXT("mipmap=1 masked=%d"), (Cast<UTexture>(*It)->PolyFlags & PF_Masked)?1:0 );
								GConfig->SetString( TEXT("ImportInfo"), It->GetName(), *Params, *(Dir + TEXT("\\") + TEXT("spec.ini") ) );
							}
							break;

						case eTYPE_SOUND:
							break;
					}

					if( Group.InStr( TEXT(".") ) != Group.InStr( TEXT("."), 1 ) )
					{
						Group = Group.Right( Group.Len() - Group.InStr( TEXT(".") ) - 1 );
						Group = Group.Left( Group.InStr( TEXT(".") ) );

						GFileManager->MakeDirectory( *(Dir + TEXT("\\") + Group) );

						Filename = Dir + TEXT("\\") + Group + TEXT("\\") + It->GetName() + TEXT(".") + Ext;
					}
					else
						Filename = Dir + TEXT("\\") + It->GetName() + TEXT(".") + Ext;

					if( UExporter::ExportToFile(*It, NULL, *Filename, 1, 0) )
						GWarn->Logf( TEXT("Exported %s"), *Filename );
					else
						appErrorf(TEXT("Can't export %s "), *Filename );
				}
			}
		}
		else if( Command == TEXT("import") )
		{
			// Grab a list of directories inside of the specified dir.  These will become group names.
			TArray<FString> Dirs = GFileManager->FindFiles( *(Dir + TEXT("\\") + TEXT("*")), 0, 1 );

			// We also include the base directory
			int dir = Dirs.Num();
			Dirs.AddZeroed ();
			Dirs(dir) = TEXT(".");	

			// Import all the filenames in the dir and one level of sub dirs.
			TArray<FString> Files;
			for( dir = 0 ; dir < Dirs.Num() ; dir++ )
			{
				Files = GFileManager->FindFiles( *(Dir + TEXT("\\") + Dirs(dir) + TEXT("\\") + TEXT("*.") + Ext), 1, 0 );

				for( int file = 0 ; file < Files.Num() ; file++ )
				{
					FString Filename = Dir + TEXT("\\") + Dirs(dir) + TEXT("\\") + Files(file);

					FString File = Files(file);
					File = File.Left( File.InStr( TEXT(".") ) );

					FString Params;
					GConfig->GetString( TEXT("ImportInfo"), *File, Params, *(Dir + TEXT("\\") + TEXT("spec.ini") ) );

					switch( GType )
					{
						case eTYPE_TEXTURE:
							{
								UBOOL bMipmaps = 1, bMasked = 0;
								Parse( *Params, TEXT("mipmap="), bMipmaps );
								Parse( *Params, TEXT("masked="), bMasked );

								if( Dirs(dir) == TEXT(".") )
								{
									GWarn->Logf( TEXT("Importing : %s"), *File );
									GEditor->Exec( *FString::Printf( TEXT("TEXTURE IMPORT FILE=\"%s\" NAME=\"%s\" PACKAGE=\"%s\" MIPS=%d FLAGS=%d"),
										*Filename, *File, *PkgName,
										bMipmaps, (bMasked?PF_Masked:0) ) );
								}
								else
								{
									GWarn->Logf( TEXT("Importing : %s.%s"), *Dirs(dir), *File );
									GEditor->Exec( *FString::Printf( TEXT("TEXTURE IMPORT FILE=\"%s\" NAME=\"%s\" PACKAGE=\"%s\" GROUP=\"%s\" MIPS=%d FLAGS=%d"),
										*Filename, *File, *PkgName, *Dirs(dir),
										bMipmaps, (bMasked?PF_Masked:0) ) );
								}
							}
							break;

						case eTYPE_SOUND:
							{
								if( Dirs(dir) == TEXT(".") )
								{
									GWarn->Logf( TEXT("Importing : %s"), *File );
									GEditor->Exec( *FString::Printf( TEXT("AUDIO IMPORT FILE=\"%s\" NAME=\"%s\" PACKAGE=\"%s\" COMPRESS=\"%s\" LIPSYNC=\"%s\""),
										*Filename, *File, *PkgName, *Compress, *LipSync ) );
								}
								else
								{
									GWarn->Logf( TEXT("Importing : %s.%s"), *Dirs(dir), *File );
									GEditor->Exec( *FString::Printf( TEXT("AUDIO IMPORT FILE=\"%s\" NAME=\"%s\" PACKAGE=\"%s\" GROUP=\"%s\" COMPRESS=\"%s\" LIPSYNC=\"%s\""),
										*Filename, *File, *PkgName, *Dirs(dir), *Compress, *LipSync ) );
								}
							}
							break;
					}
				}
			}

			GWarn->Logf( TEXT("Saving : %s"), *(PkgDir + TEXT("\\") + PkgName + TEXT(".") + PkgExt) );
			GEditor->Exec( *FString::Printf( TEXT("OBJ SAVEPACKAGE PACKAGE=\"%s\" FILE=\"%s\""),
				*PkgName, *(PkgDir + TEXT("\\") + PkgName + TEXT(".") + PkgExt) ) );
		}

		GIsRequestingExit=1;
		return 0;

		unguard;
	}
};
IMPLEMENT_CLASS(UPkgCommandlet)

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/