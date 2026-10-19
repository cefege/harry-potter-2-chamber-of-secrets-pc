// Engine-call seam for the pawn_native_files oracle harness.
// Included by the rewritten oracle UnPawn.cpp and by pawn_native_files_harness.cpp,
// so both sides share one declaration per hook.
#ifndef PAWN_NATIVE_FILES_HOOKS_H
#define PAWN_NATIVE_FILES_HOOKS_H

ULevel* PawnFilesGetLevel(const void* self);
UClass* PawnFilesFindObject(const TCHAR* name);
UClass* PawnFilesFindObjectChecked(const TCHAR* name);
UClass* PawnFilesTextureClass();
void PawnFilesRegistryObjects(TArray<FRegistryObjectInfo>& list, UClass* meta);
UProperty* PawnFilesFindProperty(UClass* cls, const TCHAR* name);
INT PawnFilesDefaultsNum(UClass* cls);
void PawnFilesExportText(UProperty* prop, TCHAR* out);
TMultiMap<FString,FString>* PawnFilesCacheSection(const TCHAR* section, UBOOL force, UBOOL cnst, const TCHAR* filename);
UBOOL PawnFilesCacheGetString(const TCHAR* section, const TCHAR* key, TCHAR* value, INT size, const TCHAR* filename);
void PawnFilesCacheSetString(const TCHAR* section, const TCHAR* key, const TCHAR* value, const TCHAR* filename);
void PawnFilesDebugf(const TCHAR* fmt, ...);
UBOOL PawnFilesMove(const TCHAR* dest, const TCHAR* src);
TArray<FString> PawnFilesFindFiles(const TCHAR* pattern, UBOOL files, UBOOL dirs);
FString PawnFilesMapExt();
void PawnFilesURLAddOption(FURL& url, const TCHAR* option);
void PawnFilesURLSaveConfig(const FURL& url, const TCHAR* section, const TCHAR* item, const TCHAR* filename);
void PawnFilesURLLoadConfig(FURL& url, const TCHAR* section, const TCHAR* filename);
const TCHAR* PawnFilesURLGetOption(const FURL& url, const TCHAR* match, const TCHAR* def);
UViewport* PawnFilesCastViewport(UObject* player);
void PawnFilesResetConfig(UClass* cls);
FLOAT PawnFilesFindPathTowardBest(APawn* self, AActor*& best, INT clear, FLOAT min_weight, UBOOL predict);
UBOOL PawnFilesIsProbing(UObject* actor, FName name);
void PawnFilesHandleSpecial(APawn* self, AActor*& best);
UViewport* PawnFilesScreenViewport();
UBOOL PawnFilesMaintainVertical();
FLOAT PawnFilesEffectiveFov(FLOAT authored, INT x, INT y, UBOOL maintain);
void PawnFilesCalcView(AActor*& view, FVector& loc, FRotator& rot);
UBOOL PawnFilesPlayerExec(UObject* player, const TCHAR* cmd, FOutputDevice& ar);
UBOOL PawnFilesEngineExec(UObject* engine, const TCHAR* cmd, FOutputDevice& ar);
FString PawnFilesLastURLMap();
void PawnFilesEntryLevelResult(APlayerPawn* self, void* const result);

#endif
