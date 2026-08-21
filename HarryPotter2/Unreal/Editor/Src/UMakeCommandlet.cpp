/*=============================================================================
	UMakeCommandlet.cpp: UnrealEd script recompiler.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Tim Sweeney.
=============================================================================*/

#include "EditorPrivate.h"

/*-----------------------------------------------------------------------------
	UMakeCommandlet.
-----------------------------------------------------------------------------*/

inline INT Compare( const FString& A, const FString& B )
{
	return appStricmp( *A, *B );
}

class UMakeCommandlet : public UCommandlet
{
	DECLARE_CLASS(UMakeCommandlet,UCommandlet,CLASS_Transient,Editor);
	void StaticConstructor()
	{
		guard(StaticConstructor::StaticConstructor);

		LogToStdout     = 0;
		IsClient        = 1;
		IsEditor        = 1;
		IsServer        = 1;
		LazyLoad        = 1;
		ShowErrorCount  = 1;

		unguard;
	}
	INT Main( const TCHAR* Parms )
	{
		guard(UMakeCommandlet::Main);

		// Create the editor class.
		UClass* EditorEngineClass = UObject::StaticLoadClass( UEditorEngine::StaticClass(), NULL, TEXT("ini:Engine.Engine.EditorEngine"), NULL, LOAD_NoFail | LOAD_DisallowFiles, NULL );
		GEditor  = ConstructObject<UEditorEngine>( EditorEngineClass );
		GEditor->UseSound = 0;
		GEditor->InitEditor();
		GIsRequestingExit = 1; // Causes ctrl-c to immediately exit.

		// Iterate through packages.
		bool bAll = appStrfind( Parms, TEXT("-ALL") ) != 0;
		//bool bDep = !appStrfind( Parms, TEXT("-NODEP") );
		bool bHeader = !appStrfind( Parms, TEXT("-NOHEADER") );
		bool bExec = !appStrfind( Parms, TEXT("-NOEXEC") );
		bool bForceSave = appStrfind( Parms, TEXT("-FORCESAVE") ) != 0;

		UClassFactoryUC* ClassFactory = new UClassFactoryUC;
		INT i;
		for( i=0; i<GEditor->EditPackages.Num(); i++ )
		{
			const TCHAR* Pkg = *GEditor->EditPackages( i );
			
			FString Filename = FString(Pkg) + TEXT(".u");
			GWarn->Log( NAME_Heading, FString::Printf(TEXT("%s - %s"),Pkg,ParseParam(appCmdLine(), TEXT("DEBUG"))? TEXT("Debug") : TEXT("Release"))); //DEBUGGER
			GWarn->Log( NAME_Heading, Pkg );
			bool bRebuild = false;

			UPackage* PkgObject = bAll ? 0 : Cast<UPackage>( LoadPackage( NULL, *Filename, LOAD_NoWarn ) );
			UPackage* PkgOrig = PkgObject;
			if( !PkgObject )
			{
				// Create package.
				bRebuild = true;
				GWarn->Logf( TEXT("Building..."), Pkg );
				PkgObject = CreatePackage( NULL, Pkg );

				// Try reading from package's .ini file.
				PkgObject->PackageFlags &= ~(PKG_AllowDownload|PKG_ClientOptional|PKG_ServerSideOnly);
				FString IniName = FString(TEXT("..")) * Pkg * TEXT("Classes") * Pkg + TEXT(".upkg");
				UBOOL B=0;
				if( GConfig->GetBool(TEXT("Flags"), TEXT("AllowDownload"), B, *IniName) && B )
					PkgObject->PackageFlags |= PKG_AllowDownload;
				if( GConfig->GetBool(TEXT("Flags"), TEXT("ClientOptional"), B, *IniName) && B )
					PkgObject->PackageFlags |= PKG_ClientOptional;
				if( GConfig->GetBool(TEXT("Flags"), TEXT("ServerSideOnly"), B, *IniName) && B )
					PkgObject->PackageFlags |= PKG_ServerSideOnly;
			}
			else
			{
				// Check class currency.
				SQWORD PkgTime = GFileManager->GetGlobalTime( *Filename );
				for( TObjectIterator<UClass> It; It; ++It)
				{
					if( It->GetOuter()==PkgObject )
					{
						const TCHAR* ClassName = It->GetName();

						FString FileName =  FString(TEXT("..")) * Pkg * TEXT("Classes")  * It->GetName() + TEXT(".uc");
						SQWORD FileTime = GFileManager->GetGlobalTime( *FileName );

						if( It->ScriptText && FileTime <= 0 )
						{
							#ifdef USE_SUB_DIRECTORIES
							//Make sure it's not in one of the sub directories   ft
							FString Spec = FString(TEXT("..")) * Pkg * TEXT("Classes\\*");
							TArray<FString> Directories = GFileManager->FindFiles( *Spec, 0, 1 );
							for( INT i = 0; i < Directories.Num(); i++ )
							{
								FileName = FString(TEXT("..")) * Pkg * TEXT("Classes") * Directories(i) * It->GetName() + TEXT(".uc");
								FileTime = GFileManager->GetGlobalTime( *FileName );
								if( FileTime > 0 )
									break;
							}
							#endif

							// Class no longer exists?
							if( FileTime <= 0 )
							{
								It->SetFlags( RF_TagGarbage );
								continue;
								bRebuild = true;
							}
						}

						if( FileTime > PkgTime || appStrfind( Parms, ClassName ) )
						{
							// Class is out of date, so re-import it. RF_Parsed will be off.
							ImportObject<UClass>( PkgObject, ClassName, RF_Public|RF_Standalone, *FileName, NULL, ClassFactory );
							bRebuild = true;
						}
					}
				}
			}

			// Get list of class files.
			FString Spec;

			//Check sub folders  ft
			#ifdef USE_SUB_DIRECTORIES
			TArray<FString> Directories;
			Directories = GFileManager->FindFiles( *(FString(TEXT("..")) * Pkg * TEXT("Classes\\*")), 0, 1 );
			new(Directories)FString( TEXT("") );  //This'll make it also search the normal Classes directory.

			for( INT i = 0; i < Directories.Num(); i++ )
			{
				Spec = FString(TEXT("..")) * Pkg * TEXT("Classes") * Directories(i);
			#else
				Spec = FString(TEXT("..")) * Pkg * TEXT("Classes");
			#endif
				FString Spec2 = Spec * TEXT("*.uc");

				TArray<FString> Files = GFileManager->FindFiles( *Spec2, 1, 0 );
				Sort( Files );
				if( Files.Num() == 0 )
					GWarn->Logf( TEXT("No files for package: %s"), *Spec2 );

				// Import any new classes.
				for( INT i=0; i<Files.Num(); i++ )
				{
					FString ClassName = Files(i).LeftChop(3);

					// See whether class currently exists in package and is up to date.
					UClass* Class = FindObject<UClass>( PkgObject, *ClassName, true );
					if( !Class || !PkgOrig )
					{
						//FString FileName = FString(TEXT("..")) * Pkg * TEXT("Classes") * Files(i);
						FString FileName = Spec * Files(i);
						ImportObject<UClass>( PkgObject, *ClassName, RF_Public|RF_Standalone, *FileName, NULL, ClassFactory );
						bRebuild = true;
					}
				}
			#ifdef USE_SUB_DIRECTORIES
			}
			#endif

			//Directories.RemoveItem( Folder );

			// Mark updated classes or dependencies.
			if( GEditor->CheckScripts( GWarn, 0, PkgObject ) )
				bRebuild = true;
			if( bRebuild || bForceSave )
			{
				if( PkgOrig )
					GWarn->Logf( TEXT("Updating..."), Pkg );

				// Verify that all script declared superclasses exist.
				for( TObjectIterator<UClass> ItC; ItC; ++ItC )
					if( ItC->ScriptText && ItC->GetSuperClass() )
						if( !ItC->GetSuperClass()->ScriptText )
							appErrorf( TEXT("Superclass %s of class %s not found"), ItC->GetSuperClass()->GetName(), ItC->GetName() );

				if( bRebuild )
				{
					// Bootstrap-recompile changed scripts.
					GEditor->Bootstrapping = 1;
					GEditor->MakeScripts( GWarn, 0, bExec );
					GEditor->Bootstrapping = 0;
				}

				// Tag native classes in this package for export.
				INT ClassCount=0;
				for( i=0; i<FName::GetMaxNames(); i++ )
					if( FName::GetEntry(i) )
						FName::GetEntry(i)->Flags &= ~RF_TagExp;
				for( TObjectIterator<UClass> It; It; ++It )
					It->ClearFlags( RF_TagImp | RF_TagExp );
				for( It=TObjectIterator<UClass>(); It; ++It )
					if( It->GetOuter()==PkgObject && It->ScriptText && (It->GetFlags()&RF_Native) && !(It->ClassFlags&CLASS_NoExport) )
						ClassCount++, It->SetFlags( RF_TagExp );

				// Export the C++ header.
				if( bRebuild && bHeader && ClassCount )
				{
					Filename = FString(TEXT("..")) * Pkg * TEXT("Inc") * Pkg + TEXT("Classes.h");
					debugf( TEXT("Autogenerating C++ header: %s"), *Filename );
					if( !UExporter::ExportToFile( UObject::StaticClass(), NULL, *Filename, 1, 1 ) )
						appErrorf( TEXT("Failed to export: %s"), *Filename );
				}

				// Save package.
				GWarn->Logf( TEXT("Saving..."), Pkg );
				ULinkerLoad* Conform = NULL;
				if( !ParseParam(appCmdLine(),TEXT("NOCONFORM")) )
				{
					BeginLoad();
					Conform = UObject::GetPackageLinker( CreatePackage(NULL,*(US+Pkg+TEXT("_OLD"))), *(FString(TEXT("..")) * TEXT("SystemConform") * Pkg + TEXT(".u")), LOAD_NoWarn|LOAD_NoVerify, NULL, NULL );
					EndLoad();
					if( Conform )
						debugf( TEXT("Conforming: %s"), Pkg );
				}
				SavePackage( PkgObject, NULL, RF_Standalone, *(FString(Pkg)+TEXT(".u")), GError, Conform );
			}
		}

		GIsRequestingExit=1;
		return 0;

		unguard;
	}

};
IMPLEMENT_CLASS(UMakeCommandlet)

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
