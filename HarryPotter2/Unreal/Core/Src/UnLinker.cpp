/*=============================================================================
	UnLinker.cpp: Unreal object linker.
	Copyright 2000 Epic Games, Inc. All Rights Reserved.

	Revision history:
		* Created by Tim Sweeney
=============================================================================*/

#include "CorePrivate.h"

/*-----------------------------------------------------------------------------
	Hash function.
-----------------------------------------------------------------------------*/

inline INT HashNames( FName A, FName B, FName C )
{
	if( C==NAME_UnrealShare )//oldver
		C=NAME_UnrealI;
	return A.GetIndex() + 7 * B.GetIndex() + 31*C.GetIndex();
}

/*-----------------------------------------------------------------------------
	FObjectExport.
-----------------------------------------------------------------------------*/

FObjectExport::FObjectExport()
:	_Object			( NULL														)
,	_iHashNext		( INDEX_NONE												)
{}

FObjectExport::FObjectExport( UObject* InObject )
:	ClassIndex		( 0															)
,	SuperIndex		( 0															)
,	PackageIndex	( 0															)
,	ObjectName		( InObject ? (InObject->GetFName()			) : NAME_None	)
,	ObjectFlags		( InObject ? (InObject->GetFlags() & RF_Load) : 0			)
,	SerialSize		( 0															)
,	SerialOffset	( 0															)
,	_Object			( InObject													)
{}

/*-----------------------------------------------------------------------------
	FObjectImport.
-----------------------------------------------------------------------------*/

FObjectImport::FObjectImport()
{}

FObjectImport::FObjectImport( UObject* InObject )
:	ClassPackage	( InObject->GetClass()->GetOuter()->GetFName())
,	ClassName		( InObject->GetClass()->GetFName()		 )
,	PackageIndex	( 0                                      )
,	ObjectName		( InObject->GetFName()					 )
,	XObject			( InObject								 )
,	SourceLinker	( NULL									 )
,	SourceIndex		( INDEX_NONE							 )
{
	if( XObject )
		UObject::GImportCount++;
}

/*----------------------------------------------------------------------------
	Items stored in Unrealfiles.
----------------------------------------------------------------------------*/

FGenerationInfo::FGenerationInfo( INT InExportCount, INT InNameCount )
: ExportCount(InExportCount), NameCount(InNameCount)
{}

FPackageFileSummary::FPackageFileSummary()
{
	appMemzero( this, appCheckedIntSize(sizeof(*this)) );
}

/*----------------------------------------------------------------------------
	ULinker.
----------------------------------------------------------------------------*/

ULinker::ULinker( UObject* InRoot, const TCHAR* InFilename )
:	LinkerRoot( InRoot )
,	Summary()
,	Success( 123456 )
,	Filename( InFilename )
,	_ContextFlags( 0 )
{
	check(LinkerRoot);
	check(InFilename);

	// Set context flags.
	if( GIsEditor ) _ContextFlags |= RF_LoadForEdit;
	if( GIsClient ) _ContextFlags |= RF_LoadForClient;
	if( GIsServer ) _ContextFlags |= RF_LoadForServer;
}

void 
ULinker::Serialize( FArchive& Ar )
{
	guard(ULinker::Serialize);
	Super::Serialize( Ar );

	// Sizes.
	ImportMap	.CountBytes( Ar );
	ExportMap	.CountBytes( Ar );

	// Prevent garbage collecting of linker's names and package.
	Ar << NameMap << LinkerRoot;
	{for( INT i=0; i<ExportMap.Num(); i++ )
	{
		FObjectExport& E = ExportMap(i);
		Ar << E.ObjectName;
	}}
	{for( INT i=0; i<ImportMap.Num(); i++ )
	{
		FObjectImport& I = ImportMap(i);
		Ar << *(UObject**)&I.SourceLinker;
		Ar << I.ClassPackage << I.ClassName;
	}}
	unguard;
}

// ULinker interface.
FString 
ULinker::GetImportFullName( INT i )
{
	guard(ULinkerLoad::GetImportFullName);
	FString S;
	for( INT j=-i-1; j!=0; j=ImportMap(-j-1).PackageIndex )
	{
		if( j != -i-1 )
			S = US + TEXT(".") + S;
		S = FString(*ImportMap(-j-1).ObjectName) + S;
	}
	return FString(*ImportMap(i).ClassName) + TEXT(" ") + S ;
	unguard;
}

FString 
ULinker::GetExportFullName( INT i, const TCHAR* FakeRoot )
{
	guard(ULinkerLoad::GetExportFullName);
	FString S;
	for( INT j=i+1; j!=0; j=ExportMap(j-1).PackageIndex )
	{
		if( j != i+1 )
			S = US + TEXT(".") + S;
		S = FString(*ExportMap(j-1).ObjectName) + S;
	}
	INT ClassIndex = ExportMap(i).ClassIndex;
	FName ClassName = ClassIndex>0 ? ExportMap(ClassIndex-1).ObjectName : ClassIndex<0 ? ImportMap(-ClassIndex-1).ObjectName : NAME_Class;
	return FString(*ClassName) + TEXT(" ") + (FakeRoot ? FakeRoot : LinkerRoot->GetPathName()) + TEXT(".") + S;
	unguard;
}

/*----------------------------------------------------------------------------
	ULinkerLoad.
----------------------------------------------------------------------------*/

ULinkerLoad::ULinkerLoad( UObject* InParent, const TCHAR* InFilename, DWORD InLoadFlags )
:	ULinker( InParent, InFilename )
,	LoadFlags( InLoadFlags )
{
	guard(ULinkerLoad::ULinkerLoad);
#if 0 //Debug added by Legend on 4/12/2000
	debugf( TEXT("%1.1fms Loading: %s"), appSeconds() * 1000.0, InParent->GetFullName() );
#endif
	Loader = GFileManager->CreateFileReader( InFilename, 0, GError );
	if( !Loader )
		appThrowf( LocalizeError("OpenFailed") );
	AltLoader = NULL;

	FPushMemTag Push(TEXT("ULinkerLoad"));

	// Error if linker already loaded.
	{for( INT i=0; i<GObjLoaders.Num(); i++ )
		if( GetLoader(i)->LinkerRoot == LinkerRoot )
			appThrowf( LocalizeError("LinkerExists"), LinkerRoot->GetName() );}

	// Begin.
	GWarn->StatusUpdatef( 0, 0, LocalizeProgress("Loading"), *Filename );

	// Set status info.
	guard(InitAr);
	ArVer       = PACKAGE_FILE_VERSION;
	ArLicenseeVer = PACKAGE_FILE_VERSION_LICENSEE; //LVer added by Legend on 4/12/2000
	ArIsLoading = ArIsPersistent = 1;
	ArForEdit   = GIsEditor;
	ArForClient = 1;
	ArForServer = 1;
	unguard;

	// Read summary from file.
	guard(LoadSummary);
	*this << Summary;
	ArVer = Summary.GetFileVersion(); //LVer added by Legend on 4/12/2000
	ArLicenseeVer = Summary.GetFileVersionLicensee(); //LVer added by Legend on 4/12/2000
	if( Cast<UPackage>(LinkerRoot) )
		Cast<UPackage>(LinkerRoot)->PackageFlags = Summary.PackageFlags;
	unguard;

	// Check tag.
	guard(CheckTag);
	if( Summary.Tag != PACKAGE_FILE_TAG )
	{
		GWarn->Logf( LocalizeError("BinaryFormat"), *Filename );
		throw( LocalizeError("Aborted") );
	}
	unguard;

	// Validate the summary.
	guard(CheckVersion);
	if( Summary.GetFileVersion() < PACKAGE_MIN_VERSION ) //LVer added by Legend on 4/12/2000
		if( !GWarn->YesNof( LocalizeQuery("OldVersion"), *Filename ) )
			throw( LocalizeError("Aborted") );
	unguard;

	// Slack everything according to summary.
	ImportMap   .Empty( Summary.ImportCount   );
	ExportMap   .Empty( Summary.ExportCount   );
	NameMap		.Empty( Summary.NameCount     );

	// Load and map names.
	guard(LoadNames);
	if( Summary.NameCount > 0 )
	{
		Seek( Summary.NameOffset );
		for( INT i=0; i<Summary.NameCount; i++ )
		{
			// Read the name entry from the file.
			FNameEntry NameEntry;
			*this << NameEntry;

			// Add it to the name table if it's needed in this context.				
			NameMap.AddItem( (NameEntry.Flags & _ContextFlags) ? FName( NameEntry.Name, FNAME_Add ) : NAME_None );
		}
	}
	unguard;

	// Load import map.
	guard(LoadImportMap);
	if( Summary.ImportCount > 0 )
	{
		Seek( Summary.ImportOffset );
		for( INT i=0; i<Summary.ImportCount; i++ )
			*this << *new(ImportMap)FObjectImport;
	}
	unguard;

	// Load export map.
	guard(LoadExportMap);
	if( Summary.ExportCount > 0 )
	{
		Seek( Summary.ExportOffset );
		for( INT i=0; i<Summary.ExportCount; i++ )
			*this << *new(ExportMap)FObjectExport;
	}
	unguard;

	// Create export hash.
	//warning: Relies on import & export tables, so must be done here.
	{for( INT i=0; i<ARRAY_COUNT(ExportHash); i++ )
	{
		ExportHash[i] = INDEX_NONE;
	}}
	{for( INT i=0; i<ExportMap.Num(); i++ )
	{
		INT iHash = HashNames( ExportMap(i).ObjectName, GetExportClassName(i), GetExportClassPackage(i) ) & (ARRAY_COUNT(ExportHash)-1);
		ExportMap(i)._iHashNext = ExportHash[iHash];
		ExportHash[iHash] = i;
	}}

	// Add this linker to the object manager's linker array.
	GObjLoaders.AddItem( this );
	if( !(LoadFlags & LOAD_NoVerify) )
		Verify();

	// Success.
	Success = 1;

	unguard;
}

void 
ULinkerLoad::Verify()
{
	guard(ULinkerLoad::Verify);
	if( !Verified )
	{
		if( Cast<UPackage>(LinkerRoot) )
			Cast<UPackage>(LinkerRoot)->PackageFlags &= ~PKG_BrokenLinks;
		try
		{
			// Validate all imports and map them to their remote linkers.
			guard(ValidateImports);
			for( INT i=0; i<Summary.ImportCount; i++ )
				VerifyImport( i );
			unguard;
		}
		// Must match the TCHAR* thrown by appThrowf; a narrower catch type
		// would leave this partially-built linker registered in GObjLoaders.
		catch( TCHAR* Error )
		{
			GObjLoaders.RemoveItem( this );
			throw( Error );
		}
	}
	Verified=1;
	unguard;
}

FName 
ULinkerLoad::GetExportClassPackage( INT i )
{
	guardSlow(ULinkerLoad::GetExportClassPackage);
	FObjectExport& Export = ExportMap( i );
	if( Export.ClassIndex < 0 )
	{
		FObjectImport& Import = ImportMap( -Export.ClassIndex-1 );
		checkSlow(Import.PackageIndex<0);
		return ImportMap( -Import.PackageIndex-1 ).ObjectName;
	}
	else if( Export.ClassIndex > 0 )
	{
		return LinkerRoot->GetFName();
	}
	else
	{
		return NAME_Core;
	}
	unguardSlow;
}

FName 
ULinkerLoad::GetExportClassName( INT i )
{
	guardSlow(GetExportClassName);
	FObjectExport& Export = ExportMap(i);
	if( Export.ClassIndex < 0 )
	{
		return ImportMap( -Export.ClassIndex-1 ).ObjectName;
	}
	else if( Export.ClassIndex > 0 )
	{
		return ExportMap( Export.ClassIndex-1 ).ObjectName;
	}
	else
	{
		return NAME_Class;
	}
	unguardSlow;
}

// Safely verify an import.
void 
ULinkerLoad::VerifyImport( INT i )
{
	guard(ULinkerLoad::VerifyImport);
	SharewareHack://oldver
	FObjectImport& Import = ImportMap(i);
	if
	(	Import.SourceIndex	!= INDEX_NONE
	||	Import.ClassPackage	== NAME_None
	||	Import.ClassName	== NAME_None
	||	Import.ObjectName	== NAME_None )
	{
		// Already verified, or not relevent in this context.
		return;
	}

	// Find or load this import's linker.
	INT Depth=0;
	UObject* Pkg=NULL;
	if( Import.PackageIndex == 0 )
	{
		check(Import.ClassName==NAME_Package);
		check(Import.ClassPackage==NAME_Core);
		UPackage* TmpPkg = CreatePackage( NULL, *Import.ObjectName );
		SharewareKludge://oldver
		try
		{
			Import.SourceLinker = GetPackageLinker( TmpPkg, NULL, LOAD_Throw | (LoadFlags & LOAD_Propagate), NULL, NULL );
		}
		catch( const TCHAR* Error )//oldver
		{
			if( TmpPkg->GetFName()==NAME_UnrealI )
			{
				TmpPkg = CreatePackage( NULL, TEXT("UnrealShare") );
				goto SharewareKludge;
			}
			appThrowf( Error );
		}
	}
	else
	{
		check(Import.PackageIndex<0);
		VerifyImport( -Import.PackageIndex-1 );
		Import.SourceLinker = ImportMap(-Import.PackageIndex-1).SourceLinker;
		check(Import.SourceLinker);
		FObjectImport* Top;
		for
		(	Top = &Import
		;	Top->PackageIndex<0
		;	Top = &ImportMap(-Top->PackageIndex-1),Depth++ );
		Pkg = CreatePackage( NULL, *Top->ObjectName );
	}
	// Find this import within its existing linker.
	UBOOL SafeReplace = 0;
	Rehack://oldver
	//new:
	INT iHash = HashNames( Import.ObjectName, Import.ClassName, Import.ClassPackage) & (ARRAY_COUNT(ExportHash)-1);
	for( INT j=Import.SourceLinker->ExportHash[iHash]; j!=INDEX_NONE; j=Import.SourceLinker->ExportMap(j)._iHashNext )
	//old:
	//for( INT j=0; j<Import.SourceLinker->ExportMap.Num(); j++ )
	{
		FObjectExport& Source = Import.SourceLinker->ExportMap( j );
		UBOOL ClassHack = Import.ClassPackage==NAME_UnrealI && Import.SourceLinker->GetExportClassPackage(j)==NAME_UnrealShare;//oldver
		if
		(	(Source.ObjectName	                          ==Import.ObjectName               )
		&&	(Import.SourceLinker->GetExportClassName   (j)==Import.ClassName                )
		&&  (Import.SourceLinker->GetExportClassPackage(j)==Import.ClassPackage || ClassHack) )
		{
			if( Import.PackageIndex<0 )
			{
				FObjectImport& ParentImport = ImportMap(-Import.PackageIndex-1);
				if( ParentImport.SourceLinker )
				{
					if( ParentImport.SourceIndex==INDEX_NONE )
					{
						if( Source.PackageIndex!=0 )
						{
							continue;
						}
					}
					else if( ParentImport.SourceIndex+1 != Source.PackageIndex )
					{
						if( Source.PackageIndex!=0 )
						{
							continue;
						}
					}
				}
			}
			if( !(Source.ObjectFlags & RF_Public) )
			{
				if( LoadFlags & LOAD_Forgiving )
				{
					if( Cast<UPackage>(LinkerRoot) )
						Cast<UPackage>(LinkerRoot)->PackageFlags |= PKG_BrokenLinks;
					debugf( TEXT("Broken import: %s %s (file %s)"), *Import.ClassName, *GetImportFullName(i), *Import.SourceLinker->Filename );
					return;
				}
				appThrowf( LocalizeError("FailedImportPrivate"), *Import.ClassName, *GetImportFullName(i) );
			}
			Import.SourceIndex = j;
			break;
		}
	}
	if( appStricmp(*Import.ClassName,TEXT("Mesh"))==0 )//oldver
	{
		Import.ClassName=FName(TEXT("LodMesh"));
		goto Rehack;
	}

	// If not found in file, see if it's a public native transient class.
	if( Import.SourceIndex==INDEX_NONE && Pkg!=NULL )
	{
		UObject* ClassPackage = FindObject<UPackage>( NULL, *Import.ClassPackage );
		if( ClassPackage )
		{
			UClass* FindClass = FindObject<UClass>( ClassPackage, *Import.ClassName );
			if( FindClass )
			{
				UObject* FindObject = StaticFindObject( FindClass, Pkg, *Import.ObjectName );
				if
				(	(FindObject)
				&&	(FindObject->GetFlags() & RF_Public)
				&&	(FindObject->GetFlags() & RF_Native)
				&&	(FindObject->GetFlags() & RF_Transient) )
				{
					Import.XObject = FindObject;
					GImportCount++;
				}
				else if( FindClass->ClassFlags & CLASS_SafeReplace )
				{
					if( GCheckConflicts )
						debugf( TEXT("Missing %s %s"), FindClass->GetName(), *GetImportFullName(i) );
					SafeReplace = 1;
				}
				else
				{
					// Look for object in any package.
					FindObject = StaticFindObject( FindClass, ANY_PACKAGE, *Import.ObjectName );
					if( FindObject )
					{
						Import.XObject = FindObject;
						GImportCount++;
					}
				}
			}
		}
		if( !Import.XObject && Pkg!=NULL && Pkg->GetFName()==NAME_UnrealI && Depth==1 )//oldver
		{
			Import.PackageIndex = -ImportMap.Num()-1;
			FObjectImport& New  = *new(ImportMap)FObjectImport;
			New.ClassPackage	= NAME_Core;
			New.ClassName		= NAME_Package;
			New.PackageIndex	= 0;
			New.ObjectName		= NAME_UnrealShare;
			New.XObject			= NULL;
			New.SourceLinker	= NULL;
			New.SourceIndex		= INDEX_NONE;
			VerifyImport(ImportMap.Num()-1);
			goto SharewareHack;
		}
		if( !Import.XObject && !SafeReplace )
		{
			debugf( TEXT("Failed import: %s %s (file %s)"), *Import.ClassName, *GetImportFullName(i), *Import.SourceLinker->Filename );
			if( LoadFlags & LOAD_Forgiving )
			{
				if( Cast<UPackage>(LinkerRoot) )
					Cast<UPackage>(LinkerRoot)->PackageFlags |= PKG_BrokenLinks;
				debugf( TEXT("Broken import: %s %s (file %s)"), *Import.ClassName, *GetImportFullName(i), *Import.SourceLinker->Filename );
				return;
			}
			appThrowf( LocalizeError("FailedImport"), *Import.ClassName, *GetImportFullName(i) );
		}
	}
	unguard;
}

// Load all objects; all errors here are fatal.
void 
ULinkerLoad::LoadAllObjects()
{
	guard(ULinkerLoad::LoadAllObjects);
	for( INT i=0; i<Summary.ExportCount; i++ )
		CreateExport( i );
	unguardobj;
}

// Find the index of a specified object.
//!!without regard to specific package
INT 
ULinkerLoad::FindExportIndex( FName ClassName, FName ClassPackage, FName ObjectName, INT PackageIndex )
{
	guard(ULinkerLoad::FindExportIndex);
Rehack://oldver
	INT iHash = HashNames( ObjectName, ClassName, ClassPackage ) & (ARRAY_COUNT(ExportHash)-1);
	for( INT i=ExportHash[iHash]; i!=INDEX_NONE; i=ExportMap(i)._iHashNext )
	{
		if
		(  (ExportMap(i).ObjectName  ==ObjectName                              )
		&& (ExportMap(i).PackageIndex==PackageIndex || PackageIndex==INDEX_NONE)
		&& (GetExportClassPackage(i) ==ClassPackage                            )
		&& (GetExportClassName   (i) ==ClassName                               ) )
		{
			return i;
		}
	}
	if( appStricmp(*ClassName,TEXT("Mesh"))==0 )//oldver.
	{
		ClassName = FName(TEXT("LodMesh"));
		goto Rehack;
	}
	return INDEX_NONE;
	unguard;
}

// Create a single object.
UObject* 
ULinkerLoad::Create( UClass* ObjectClass, FName ObjectName, DWORD LoadFlags, UBOOL Checked )
{
	guard(ULinkerLoad::Create);
	//old:
	//for( INT i=0; i<ExportMap.Num(); i++ )
	//new:
	INT Index = FindExportIndex( ObjectClass->GetFName(), ObjectClass->GetOuter()->GetFName(), ObjectName, INDEX_NONE );
	if( Index!=INDEX_NONE )
		return (LoadFlags & LOAD_Verify) ? reinterpret_cast<UObject*>(~static_cast<UPTRINT>(0)) : CreateExport(Index);
	if( Checked )
		appThrowf( LocalizeError("FailedCreate"), ObjectClass->GetName(), *ObjectName );
	return NULL;
	unguard;
}

void 
ULinkerLoad::Preload( UObject* Object )
{
	guard(ULinkerLoad::Preload);
	check(IsValid());
	check(Object);
	if( Object->GetFlags() & RF_Preloading )
	{
		// Warning for internal development.
		//debugf( "Object preload reentrancy: %s", Object->GetFullName() );
	}
	if( Object->GetLinker()==this )
	{
		// Preload the object if necessary.
		if( Object->GetFlags() & RF_NeedLoad )
		{
			// If this is a struct, preload its super.
			if(	Object->IsA(UStruct::StaticClass()) )
				if( ((UStruct*)Object)->SuperField )
					Preload( ((UStruct*)Object)->SuperField );

			// Load the local object now.
			guard(LoadObject);
			FObjectExport& Export = ExportMap( Object->_LinkerIndex );
			check(Export._Object==Object);
			INT SavedPos = Loader->Tell();
			Loader->Seek( Export.SerialOffset );
			Loader->Precache( Export.SerialSize );

			// Load the object.
			FPushMemTag Push(Object->Class->GetName());
			Object->ClearFlags ( RF_NeedLoad );
			Object->SetFlags   ( RF_Preloading );
			Object->Serialize  ( *this );
			Object->ClearFlags ( RF_Preloading );
			//debugf(NAME_Log,"    %s: %i", Object->GetFullName(), Export.SerialSize );

			// Make sure we serialized the right amount of stuff.
			if( Tell()-Export.SerialOffset != Export.SerialSize )
				appErrorf( LocalizeError("SerialSize"), Object->GetFullName(), Tell()-Export.SerialOffset, Export.SerialSize );
			Loader->Seek( SavedPos );
			unguardf(( TEXT("(%s %i==%i/%i %i %i)"), Object->GetFullName(), Loader->Tell(), Loader->Tell(), Loader->TotalSize(), ExportMap( Object->_LinkerIndex ).SerialOffset, ExportMap( Object->_LinkerIndex ).SerialSize ));
		}
	}
	else if( Object->GetLinker() )
	{
		// Send to the object's linker.
		Object->GetLinker()->Preload( Object );
	}
	unguard;
}

UObject* 
ULinkerLoad::CreateExport( INT Index )
{
	guard(ULinkerLoad::CreateExport);

	// Map the object into our table.
	FObjectExport& Export = ExportMap( Index );
	if( !Export._Object && (Export.ObjectFlags & _ContextFlags) )
	{
		check(Export.ObjectName!=NAME_None || !(Export.ObjectFlags&RF_Public));

		// Get the object's class.
		UClass* LoadClass = (UClass*)IndexToObject( Export.ClassIndex );
		if( !LoadClass )
			LoadClass = UClass::StaticClass();
		check(LoadClass);
		check(LoadClass->GetClass()==UClass::StaticClass());
		if( LoadClass->GetFName()==NAME_Camera )//oldver
			return NULL;
		Preload( LoadClass );

		// Get the outer object. If that caused the object to load, return it.
		UObject* ThisParent = Export.PackageIndex ? IndexToObject(Export.PackageIndex) : LinkerRoot;
		if( Export._Object )
			return Export._Object;

		//oldver: Move actors from root to level.
		/*if( Ver() <= 61 )
		{
			static UClass* ActorClass = FindObject<UClass>(ANY_PACKAGE,"Actor");
			static UClass* LevelClass = FindObject<UClass>(ANY_PACKAGE,"Level");
			if( ActorClass && LoadClass->IsChildOf(ActorClass) && ThisParent==LinkerRoot )
				ThisParent = StaticFindObjectChecked( LevelClass, LinkerRoot, "MyLevel" );
		}*/

		// Create the export object.
		Export._Object = StaticConstructObject
		(
			LoadClass,
			ThisParent,
			Export.ObjectName,
			(Export.ObjectFlags & RF_Load) | RF_NeedLoad | RF_NeedPostLoad
		);
		Export._Object->SetLinker( this, Index );
		FPushMemTag Push(TEXT("GObjLoaded"));
		GObjLoaded.AddItem( Export._Object );
		debugfSlow( NAME_DevLoad, TEXT("Created %s"), Export._Object->GetFullName() );

		// If it's a struct or class, set its parent.
		if( Export._Object->IsA(UStruct::StaticClass()) && Export.SuperIndex!=0 )
			((UStruct*)Export._Object)->SuperField = (UStruct*)IndexToObject( Export.SuperIndex );

		// If it's a class, bind it to C++.
		if( Export._Object->IsA( UClass::StaticClass() ) )
			((UClass*)Export._Object)->Bind();
	}
	return Export._Object;
	unguardf(( TEXT("(%s %i)"), *ExportMap(Index).ObjectName, Tell() ));
}

// Return the loaded object corresponding to an import index; any errors are fatal.
UObject* 
ULinkerLoad::CreateImport( INT Index )
{
	guard(ULinkerLoad::CreateImport);
	FObjectImport& Import = ImportMap( Index );
	if( !Import.XObject && Import.SourceIndex>=0 )
	{
		//debugf( "Imported new %s %s.%s", *Import.ClassName, *Import.ObjectPackage, *Import.ObjectName );
		check(Import.SourceLinker);
		Import.XObject = Import.SourceLinker->CreateExport( Import.SourceIndex );
		GImportCount++;
	}
	return Import.XObject;
	unguard;
}

// Map an import/export index to an object; all errors here are fatal.
UObject* 
ULinkerLoad::IndexToObject( INT Index )
{
	guard(IndexToObject);
	if( Index > 0 )
	{
		if( !ExportMap.IsValidIndex( Index-1 ) )
			appErrorf( LocalizeError("ExportIndex"), Index-1, ExportMap.Num() );			
		return CreateExport( Index-1 );
	}
	else if( Index < 0 )
	{
		if( !ImportMap.IsValidIndex( -Index-1 ) )
			appErrorf( LocalizeError("ImportIndex"), -Index-1, ImportMap.Num() );
		return CreateImport( -Index-1 );
	}
	else return NULL;
	unguard;
}

// Detach an export from this linker.
void 
ULinkerLoad::DetachExport( INT i )
{
	guard(ULinkerLoad::DetachExport);
	FObjectExport& E = ExportMap( i );
	check(E._Object);
	if( !E._Object->IsValid() )
		appErrorf( TEXT("Linker object %s %s.%s is invalid"), *GetExportClassName(i), LinkerRoot->GetName(), *E.ObjectName );
	if( E._Object->GetLinker()!=this )
		appErrorf( TEXT("Linker object %s %s.%s mislinked"), *GetExportClassName(i), LinkerRoot->GetName(), *E.ObjectName );
	if( E._Object->_LinkerIndex!=i )
		appErrorf( TEXT("Linker object %s %s.%s misindexed"), *GetExportClassName(i), LinkerRoot->GetName(), *E.ObjectName );
	ExportMap(i)._Object->SetLinker( NULL, INDEX_NONE );
	unguard;
}

// UObject interface.
void 
ULinkerLoad::Serialize( FArchive& Ar )
{
	guard(ULinkerLoad::Serialize);
	Super::Serialize( Ar );
	LazyLoaders.CountBytes( Ar );
	unguard;
}

void 
ULinkerLoad::Destroy()
{
	guard(ULinkerLoad::Destroy);
#if 1 //Debug added by Legend on 4/12/2000
	debugf( TEXT("%1.1fms Unloading: %s"), appSeconds() * 1000.0, LinkerRoot->GetFullName() );
#endif

	// Detach all lazy loaders.
	DetachAllLazyLoaders( 0 );

	// Detach all objects linked with this linker.
	for( INT i=0; i<ExportMap.Num(); i++ )
		if( ExportMap(i)._Object )
			DetachExport( i );

	// Remove from object manager, if it has been added.
	GObjLoaders.RemoveItem( this );
	if( Loader )
		delete Loader;
	Loader = NULL;
	if( AltLoader )
		delete AltLoader;
	AltLoader = NULL;

	Super::Destroy();
	unguardobj;
}

// FArchive interface.
void 
ULinkerLoad::AttachLazyLoader( FLazyLoader* LazyLoader )
{
	guard(ULinkerLoad::AttachLazyLoader);
	checkSlow(LazyLoader->SavedAr==NULL);
	checkSlow(LazyLoaders.FindItemIndex(LazyLoader)==INDEX_NONE);

	LazyLoaders.AddItem( LazyLoader );
	LazyLoader->SavedAr  = this;
	LazyLoader->SavedPos = Tell();

	unguard;
}

void 
ULinkerLoad::DetachLazyLoader( FLazyLoader* LazyLoader )
{
	guard(ULinkerLoad::DetachLazyLoader);
	checkSlow(LazyLoader->SavedAr==this);

	INT RemovedCount = LazyLoaders.RemoveItem(LazyLoader);
	if( RemovedCount!=1 )
		appErrorf( TEXT("Detachment inconsistency: %i (%s)"), RemovedCount, *Filename );
	LazyLoader->SavedAr = NULL;
	LazyLoader->SavedPos = 0;

	unguard;
}

void 
ULinkerLoad::DetachAllLazyLoaders( UBOOL Load )
{
	guard(ULinkerLoad::DetachAllLazyLoaders);
	for( INT i=0; i<LazyLoaders.Num(); i++ )
	{
		FLazyLoader* LazyLoader = LazyLoaders( i );
		if( Load )
			LazyLoader->Load();
		LazyLoader->SavedAr  = NULL;
		LazyLoader->SavedPos = 0;
	}
	LazyLoaders.Empty();
	unguard;
}

FArchive* 
ULinkerLoad::StreamAr()
{
	if( AltLoader == NULL )
		AltLoader = GFileManager->CreateFileReader( *Filename, 0, GError );
	return AltLoader;
}

// FArchive interface.
void 
ULinkerLoad::Seek( INT InPos )
{
	guard(ULinkerLoad::Seek);
	Loader->Seek( InPos );
	unguard;
}

INT 
ULinkerLoad::Tell()
{
	guard(ULinkerLoad::Tell);
	return Loader->Tell();
	unguard;
}

INT 
ULinkerLoad::TotalSize()
{
	guard(ULinkerLoad::TotalSize);
	return Loader->TotalSize();
	unguard;
}

void 
ULinkerLoad::Serialize( void* V, INT Length )
{
	guard(ULinkerLoad::Serialize);
	Loader->Serialize( V, Length );
	unguard;
}

/*----------------------------------------------------------------------------
	ULinkerSave.
----------------------------------------------------------------------------*/

ULinkerSave::ULinkerSave( UObject* InParent, const TCHAR* InFilename )
:	ULinker( InParent, InFilename )
,	Saver( NULL )
{
	// Create file saver.
	Saver = GFileManager->CreateFileWriter( InFilename, 0, GThrow );
	if( !Saver )
		appThrowf( LocalizeError("OpenFailed") );

	// Set main summary info.
	Summary.Tag           = PACKAGE_FILE_TAG;
	Summary.SetFileVersions( PACKAGE_FILE_VERSION, PACKAGE_FILE_VERSION_LICENSEE ); //LVer added by Legend on 4/12/2000
	Summary.PackageFlags  = Cast<UPackage>(LinkerRoot) ? Cast<UPackage>(LinkerRoot)->PackageFlags : 0;

	// Set status info.
	ArIsSaving     = 1;
	ArIsPersistent = 1;
	ArForEdit      = GIsEditor;
	ArForClient    = 1;
	ArForServer    = 1;

	// Allocate indices.
	ObjectIndices.AddZeroed( UObject::GObjObjects.Num() );
	NameIndices  .AddZeroed( FName::GetMaxNames() );

	// Success.
	Success=1;
}

void 
ULinkerSave::Destroy()
{
	guard(ULinkerSave::Destroy);
	if( Saver )
		delete Saver;
	Saver = NULL;
	Super::Destroy();
	unguard;
}

// FArchive interface.
INT 
ULinkerSave::MapName( FName* Name )
{
	guardSlow(ULinkerSave::MapName);
	return NameIndices(Name->GetIndex());
	unguardobjSlow;
}

INT 
ULinkerSave::MapObject( UObject* Object )
{
	guardSlow(ULinkerSave::MapObject);
	return Object ? ObjectIndices(Object->GetIndex()) : 0;
	unguardobjSlow;
}

void 
ULinkerSave::Seek( INT InPos )
{
	Saver->Seek( InPos );
}

INT 
ULinkerSave::Tell()
{
	return Saver->Tell();
}

void 
ULinkerSave::Serialize( void* V, INT Length )
{
	Saver->Serialize( V, Length );
}

/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/
