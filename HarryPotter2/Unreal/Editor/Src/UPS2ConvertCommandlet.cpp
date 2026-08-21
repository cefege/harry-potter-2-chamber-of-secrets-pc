/*=============================================================================
	UPS2ConvertCommandlet.cpp: ADPCM sound compression utility.
	Copyright 2000 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Brandon Reinhart
=============================================================================*/

#include "EditorPrivate.h"

/*-----------------------------------------------------------------------------
	UPS2ConvertCommandlet
-----------------------------------------------------------------------------*/

class UPS2ConvertCommandlet : public UCommandlet
{
	DECLARE_CLASS(UPS2ConvertCommandlet,UCommandlet,CLASS_Transient,Editor);
	void StaticConstructor()
	{
		guard(UPS2ConvertCommandlet::StaticConstructor);

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
		guard(UPS2ConvertCommandlet::Main);
		FString WildCard;
		if( !ParseToken(Parms,WildCard,0) )
			appErrorf(TEXT("Package search mask not specified."));
		UClass* Class = FindObjectChecked<UClass>( ANY_PACKAGE, TEXT("Sound") );
		TArray<FString> FilesFound = GFileManager->FindFiles( *WildCard, 1, 0 );
		for (INT i=0; i<FilesFound.Num(); i++)
		{
			FString Pkg = FilesFound(i);
			GWarn->Logf( TEXT("Package %s..."), *Pkg );
			GWarn->Logf( TEXT("  Loading") );
			UObject* Package = LoadPackage(NULL,*Pkg,LOAD_NoWarn);
			if (Package != NULL)
			{
				GWarn->Logf( TEXT("  Converting") );
				for( TObjectIterator<UObject> It; It; ++It )
				{
					if( It->IsA(Class) && It->IsIn(Package) )
					{
						USound* Sound = (USound*) *It;
						Sound->PS2Convert();
					}
				}
				GWarn->Logf( TEXT("  Saving") );
				SavePackage( Package, NULL, RF_Standalone, *Pkg, GError, NULL );
			}
		}

		GIsRequestingExit=1;
		return 0;
		unguard;
	}
};
IMPLEMENT_CLASS(UPS2ConvertCommandlet)

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
