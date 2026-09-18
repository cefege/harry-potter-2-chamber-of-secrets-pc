/*=============================================================================
	UnEngineWin.h: Unreal engine windows-specific code.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Revision history:
	* Created by Tim Sweeney.
=============================================================================*/
#pragma DISABLE_OPTIMIZATION /* Avoid VC++ code generation bug */


/*-----------------------------------------------------------------------------
	Splash screen.
-----------------------------------------------------------------------------*/

//
// Splash screen, implemented with old-style Windows code so that it
// can be opened super-fast before initialization.
//


	BOOL UseD3DDriver()
	{
		TCHAR	tcsSection[256], tcsSection2[256];
		FString fs;
		TCHAR	tcs[256];
		int		iMem;
		BOOL	bUseD3D = FALSE;
		FString fsVendor = GConfig->GetStr(TEXT("D3DDrv.D3DRenderDevice"),TEXT("dwVendorId"));
		FString fsDevice = GConfig->GetStr(TEXT("D3DDrv.D3DRenderDevice"),TEXT("dwDeviceId"));
		
		appSprintf( tcsSection, TEXT("%s"), fsVendor );
		appSprintf( tcsSection2, TEXT("%s"), fsDevice );
		appStrcat(tcsSection, TEXT("."));
		appStrcat(tcsSection, tcsSection2);

		GConfig->GetInt(TEXT("CARDCAP"),TEXT("VRAM"), iMem);

		if (iMem < 4096)		//Not enough memory -- force software
		{
			bUseD3D = FALSE;
		}
		else if (!GConfig->GetString( tcsSection, TEXT("Softwareonly"), fs ))	// no entry -- assume D3D-compatible
		{
			bUseD3D = TRUE;
		}

		if (bUseD3D == FALSE)
		{
			// Here's where we will play with .ini values
			if( GConfig->GetString( tcsSection, TEXT("topRes"), fs ) )
			{
				appSprintf( tcs, TEXT("%s"), fs );
				GConfig->SetString(TEXT("optionsettings"), TEXT("MaxRes"), tcs);
			}
			if( GConfig->GetString( tcsSection, TEXT("shadows"), fs ) )
			{
				appSprintf( tcs, TEXT("%s"), fs );
				GConfig->SetString(TEXT("optionsettings"), TEXT("shadowsOn"), tcs);
			}
			if( GConfig->GetString( tcsSection, TEXT("Softwareonly"), fs ) )
			{
				if ((fs == TEXT("FALSE"))  &&  (iMem >= 4096))
				{
					bUseD3D = TRUE;
				}
			}
			GConfig->Flush(0);
		}

		return bUseD3D;
	}


BOOL CALLBACK SplashDialogProc( HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam )
{
	if( uMsg==WM_DESTROY )
		PostQuitMessage(0);
	return 0;
}
HWND    hWndSplash = NULL;
HBITMAP hBitmap    = NULL;
INT     BitmapX    = 0;
INT     BitmapY    = 0;
DWORD   ThreadId   = 0;
HANDLE  hThread    = 0;
INT Reconfig;
DWORD WINAPI ThreadProc( VOID* Parm )
{
	hWndSplash = TCHAR_CALL_OS(CreateDialogW(hInstance,MAKEINTRESOURCEW(IDDIALOG_Splash), NULL, SplashDialogProc),CreateDialogA(hInstance, MAKEINTRESOURCEA(IDDIALOG_Splash), NULL, SplashDialogProc) );
	if( hWndSplash )
	{
		HWND hWndLogo = GetDlgItem(hWndSplash,IDC_Logo);
		if( hWndLogo )
		{
			SetWindowPos(hWndSplash,HWND_TOPMOST,(GetSystemMetrics(SM_CXSCREEN)-BitmapX)/2,(GetSystemMetrics(SM_CYSCREEN)-BitmapY)/2,BitmapX,BitmapY,SWP_SHOWWINDOW);
			SetWindowPos(hWndSplash,HWND_NOTOPMOST,0,0,0,0,SWP_NOMOVE|SWP_NOSIZE);
			SendMessageX( hWndLogo, STM_SETIMAGE, IMAGE_BITMAP, (LPARAM)hBitmap );
			UpdateWindow( hWndSplash );
			MSG Msg;
			while( TCHAR_CALL_OS(GetMessageW(&Msg,NULL,0,0),GetMessageA(&Msg,NULL,0,0)) )
				DispatchMessageX(&Msg);
		}
	}
	return 0;
}
void InitSplash( const TCHAR* Filename )
{
	FWindowsBitmap Bitmap(1);
	if( Filename )
	{
		verify(Bitmap.LoadFile(Filename) );
		hBitmap = Bitmap.GetBitmapHandle();
		BitmapX = Bitmap.SizeX;
		BitmapY = Bitmap.SizeY;
	}
	hThread=CreateThread(NULL,0,&ThreadProc,NULL,0,&ThreadId);
}
void ExitSplash()
{
	if( ThreadId )
		TCHAR_CALL_OS(PostThreadMessageW(ThreadId,WM_QUIT,0,0),PostThreadMessageA(ThreadId,WM_QUIT,0,0));
}

/*-----------------------------------------------------------------------------
	System Directories.
-----------------------------------------------------------------------------*/

TCHAR SysDir[256]=TEXT(""), WinDir[256]=TEXT(""), ThisFile[256]=TEXT("");
void InitSysDirs()
{
#if UNICODE
	if( !GUnicodeOS )
	{
		ANSICHAR ASysDir[256]="", AWinDir[256]="", AThisFile[256]="";
		GetSystemDirectoryA( ASysDir, ARRAY_COUNT(ASysDir) );
		GetWindowsDirectoryA( AWinDir, ARRAY_COUNT(AWinDir) );
		GetModuleFileNameA( NULL, AThisFile, ARRAY_COUNT(AThisFile) );
		appStrcpy( SysDir, ANSI_TO_TCHAR(ASysDir) );
		appStrcpy( WinDir, ANSI_TO_TCHAR(AWinDir) );
		appStrcpy( ThisFile, ANSI_TO_TCHAR(AThisFile) );
	}
	else
#endif
	{
		GetSystemDirectory( SysDir, ARRAY_COUNT(SysDir) );
		GetWindowsDirectory( WinDir, ARRAY_COUNT(WinDir) );
		GetModuleFileName( NULL, ThisFile, ARRAY_COUNT(ThisFile) );
	}
	if( !appStricmp( &ThisFile[appStrlen(ThisFile) - 4], TEXT(".ICD") ) )
		appStrcpy( &ThisFile[appStrlen(ThisFile) - 4], TEXT(".EXE") );
}

static void CreateDetected()
{
	GConfig->Flush( 1 );
	if( !ParseParam(appCmdLine(),TEXT("nodetect")) )
	{
		FString Detected = FString(appUserDir()) * TEXT("Detected.ini");
		GFileManager->Delete(*Detected);
		ShellExecuteX( NULL, TEXT("open"), ThisFile, TEXT("testrendev=D3DDrv.D3DRenderDevice log=Detected.log"), appBaseDir(), SW_SHOWNORMAL );
		for( INT MSec=80000; MSec>0 && GFileManager->FileSize(*Detected)<0; MSec-=100 )
			Sleep(100);
	}
}

/*-----------------------------------------------------------------------------
	Front End
-----------------------------------------------------------------------------*/

// *************************
// *** FRONT END DEFINES ***
// *************************

// Save Slot files
const TCHAR SAVE_FILENAME[]							= TEXT("Save0.usa");
const TCHAR SAVE_FILENAME_THUMBNAIL[]				= TEXT("Save0.bmp");

// Hlep files
const TCHAR BITMAP_FILENAME_BACKGROUND[]			= TEXT("..\\Help\\Background.bmp");
const TCHAR BITMAP_FILENAME_BUTTON_UP[]				= TEXT("..\\Help\\ButtonUp.bmp");
const TCHAR BITMAP_FILENAME_BUTTON_THUMB_UP[]		= TEXT("..\\Help\\ButtonThumbUp.bmp");

// Command line tokens							
const TCHAR NEWGAME_COMMAND_LINE_TOKEN[]			= TEXT("PrivetDr.unr");
const TCHAR LOADGAME_COMMAND_LINE_TOKEN[]			= TEXT("-LOAD=0");
	
// SaveSlot defines
const int NUM_SAVE_SLOTS							= 7;

struct SaveSlotData
{
	bool		bEmpty;		// is this slot empty?
	FString		Dir;		// Directory path
	CBitmap		Thumbnail;	// thumbnail screenshot for this slot
	
	SaveSlotData() : bEmpty(1) { }
};


class WFrontEnd : public WFrontEndDialog
{
	DECLARE_WINDOWCLASS(WFrontEnd,WFrontEndDialog,Startup)
	
	//
	UBOOL			Cancel;
	FString			Title;
	WLabel			Instructions;
	
	// Bitmaps
	FWindowsBitmap	BitmapButtonUp, BitmapButtonDown, BitmapThumbnailUp, BitmapThumbnailDown;

	// SaveSlot Data
	SaveSlotData	SaveSlots[NUM_SAVE_SLOTS];	// array of SaveSlotData

	WFrontEnd()
	: Cancel		( 0 )
	, Instructions	( this, IDC_Instructions )
	{
		InitSysDirs();
	}
	
	void OnInitDialog()
	{
		guard(WFrontEnd::OnInitDialog);
		
		// --- Load our bitmaps
		
		// Button bitmaps
		BitmapButtonUp.LoadFile(	 BITMAP_FILENAME_BUTTON_UP 	     );
		BitmapThumbnailUp.LoadFile(  BITMAP_FILENAME_BUTTON_THUMB_UP );
		
		// Background bitmap
		BackgroundBitmap.LoadFile( BITMAP_FILENAME_BACKGROUND  );
		
		// --- Init our SaveSlots
		for(int i=1; i < NUM_SAVE_SLOTS; ++i )
		{
			// Set our SaveSlotDir
			TCHAR itoa[32];
			appSprintf( itoa, TEXT("%d"), i );
			SaveSlots[i].Dir = GSys->SavePath * TEXT("Slot") + itoa;
			GFileManager->MakeDirectory( *SaveSlots[i].Dir, 0 );
			
			// See if this saveSlot has a save game file
			SaveSlots[i].bEmpty = (GFileManager->FileSize(*(SaveSlots[i].Dir*SAVE_FILENAME)) > 0 ) ? false:true;
			
			// If this slot is empty clean out its files! 
			// ( just to make sure, as we can't have any pa files lingering )
			if( SaveSlots[i].bEmpty )
			{
				CleanSaveSlotDir( i );
			}
			else
			{
				// Load our thumbnail bitmap
				SaveSlots[i].Thumbnail.LoadFile( TCHAR_TO_ANSI(*(SaveSlots[i].Dir * SAVE_FILENAME_THUMBNAIL)) );
			}
		}
		
		SetText( Localize(TEXT("IDDIALOG_FrontEndPageNewGame"),TEXT("IDC_Title"),TEXT("Startup")) );
		
		WFrontEndDialog::OnInitDialog();
		
		
		// Set our Ok and Back button's bitmaps
		OkButton.SetBitmap(  BitmapButtonUp.GetBitmapHandle() );
		BackButton.SetBitmap( BitmapButtonUp.GetBitmapHandle() );
		
		SetForegroundWindow( hWnd );
		SetActiveWindow( hWnd );
		UpdateWindow( hWnd );
		ShowWindow( hWnd, SW_RESTORE );
		
		unguard;
	}
	
	void OnDestroy()
	{
		guard(WFrontEnd::OnDestroy);
		
		for(int i=0; i < NUM_SAVE_SLOTS; ++i )
			SaveSlots[i].Thumbnail.Destroy();
		
		WFrontEndDialog::OnDestroy();
		
		unguard;
	}

	void CleanSaveSlotDir( INT iSlot )
	{
		// Clean out directory
		if( SaveSlots[iSlot].Dir != TEXT("") )
		{
			FString Spec = SaveSlots[iSlot].Dir * TEXT("*.*");
			TArray<FString> Files = GFileManager->FindFiles( *Spec, true, false );
			
			for( INT j=0; Files.IsValidIndex(j); j++ )
			{
				if( GFileManager->FileSize( *(SaveSlots[iSlot].Dir * Files(j)) ) )
				{
					// Delete found file
					GFileManager->Delete( *(SaveSlots[iSlot].Dir * Files(j)), false, true );
				}
			}
		}
		
		// Set the empty boolean for this slot
		SaveSlots[iSlot].bEmpty = true;
	}

	void LaunchNewGame( INT iSlot )
	{
		FString CmdLine = appCmdLine();
		CmdLine += NEWGAME_COMMAND_LINE_TOKEN;
		
		TCHAR SlotToken[32];	
		appSprintf( SlotToken, TEXT(" -SAVESLOT=%d"), iSlot );	
		CmdLine += SlotToken;
		
		// Launch the game and End the Front-End
		ShellExecuteX( NULL, TEXT("open"), ThisFile, *CmdLine, appBaseDir(), SW_SHOWNORMAL );
		EndDialog(0);
	}

	void LaunchLoadGame( INT iSlot )
	{
		FString CmdLine = appCmdLine();
		CmdLine += LOADGAME_COMMAND_LINE_TOKEN;
		
		TCHAR SlotToken[32];	
		appSprintf( SlotToken, TEXT(" -SAVESLOT=%d"), iSlot );	
		CmdLine += SlotToken;
		
		// Launch the game and End the Front-End
		ShellExecuteX( NULL, TEXT("open"), ThisFile, *CmdLine, appBaseDir(), SW_SHOWNORMAL );
		EndDialog(0);
	}
};


class WFrontEndPageNewGame : public WFrontEndPage
{
	DECLARE_WINDOWCLASS(WFrontEndPageNewGame,WFrontEndPage,Startup)
	WFrontEnd*		Owner;
	WBitmapButton	SlotButton1,SlotButton2,SlotButton3,SlotButton4,SlotButton5,SlotButton6;	
	
	WFrontEndPageNewGame( WFrontEnd* InOwner )
	: WFrontEndPage  ( TEXT("FrontEndPageNewGame"), IDDIALOG_FrontEndPageSlotMenu, InOwner )
	, SlotButton1	 ( this, IDC_Slot1,	FDelegate(this,(TDelegate)OnSlot1),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton2	 ( this, IDC_Slot2,	FDelegate(this,(TDelegate)OnSlot2),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton3	 ( this, IDC_Slot3,	FDelegate(this,(TDelegate)OnSlot3),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton4	 ( this, IDC_Slot4,	FDelegate(this,(TDelegate)OnSlot4),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton5	 ( this, IDC_Slot5,	FDelegate(this,(TDelegate)OnSlot5),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton6	 ( this, IDC_Slot6,	FDelegate(this,(TDelegate)OnSlot6),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, Owner          ( InOwner )
	{ }
	
	WBitmapButton& GetButton( int i )
	{
		switch( i )
		{
			case 0: return SlotButton1;
			case 1: return SlotButton1;
			case 2: return SlotButton2;
			case 3: return SlotButton3;
			case 4: return SlotButton4;
			case 5: return SlotButton5;
			case 6: return SlotButton6;
		}
		return SlotButton1;
	}
	
	void OnCurrent()
	{	
		// Setup title and instruction labels
		Owner->SetText( Localize(TEXT("IDDIALOG_FrontEndPageNewGame"),TEXT("IDC_Title"),TEXT("Startup")) );
		Owner->Instructions.SetText( Localize(TEXT("IDDIALOG_FrontEndPageNewGame"),TEXT("IDC_Instructions"),TEXT("Startup")) );
	}

	void OnInitDialog()
	{
		int i;

		WFrontEndPage::OnInitDialog();
		
		// Setup our background bitmap
		SetBitmap( Owner->BackgroundBitmap.GetBitmapHandle() );	
				
		// Setup the Save Slot buttons
		for(i=1; i<NUM_SAVE_SLOTS; ++i )
		{
			TCHAR text[512];
			if( Owner->SaveSlots[i].bEmpty )
			{
				appSprintf( text, TEXT("%d - %s"), i, Localize(TEXT("IDDIALOG_FrontEndPageNewGame"),TEXT("IDC_SlotEmpty"),TEXT("Startup")) );
				GetButton(i).SetBitmap( Owner->BitmapThumbnailUp.GetBitmapHandle() );
			}
			else
			{
				appSprintf( text, TEXT("%d - %s"), i, Localize(TEXT("IDDIALOG_FrontEndPageNewGame"),TEXT("IDC_SlotUsed"),TEXT("Startup")) );
				GetButton(i).SetBitmap( &Owner->SaveSlots[i].Thumbnail );
			}
			GetButton(i).SetText( text );
		}
	}

	const TCHAR* GetOkText()
	{
		return NULL;
	}

	void OnSlot1()	{ OnSlot( 1 ); }
	void OnSlot2()	{ OnSlot( 2 ); }
	void OnSlot3()	{ OnSlot( 3 ); }
	void OnSlot4()	{ OnSlot( 4 ); }
	void OnSlot5()	{ OnSlot( 5 ); }
	void OnSlot6()	{ OnSlot( 6 ); }
	void OnSlot( INT iSlot )
	{
		// If this slot isn't empty, make sure its ok to delete the files before continuing
		if( !Owner->SaveSlots[iSlot].bEmpty )
		{
			// Ask the user if he is sure he wants to use this slot!
			if( ::MessageBox( hWnd, 
				Localize(TEXT("All"),TEXT("Select_Game_0001"),TEXT("HPMenu") ),
				Localize(TEXT("All"),TEXT("Select_Game_0002"),TEXT("HPMenu") ), MB_YESNO ) == IDNO )
				return;
		}
		
		// Clean out all the old files for this slot, we are starting a new game
		Owner->CleanSaveSlotDir( iSlot );
				
		// Launch our game using the specified slot!
		Owner->LaunchNewGame( iSlot );	
	}
};

class WFrontEndPageLoadGame : public WFrontEndPage
{
	DECLARE_WINDOWCLASS(WFrontEndPageLoadGame,WFrontEndPage,Startup)
	WFrontEnd*		Owner;
	WLabel			Instructions;
	WBitmapButton	SlotButton1,SlotButton2,SlotButton3,SlotButton4,SlotButton5,SlotButton6;

	WFrontEndPageLoadGame( WFrontEnd* InOwner )
	: WFrontEndPage  ( TEXT("FrontEndPageLoadGame"), IDDIALOG_FrontEndPageSlotMenu, InOwner )
	, SlotButton1	 ( this, IDC_Slot1,	FDelegate(this,(TDelegate)OnSlot1),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton2	 ( this, IDC_Slot2,	FDelegate(this,(TDelegate)OnSlot2),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton3	 ( this, IDC_Slot3,	FDelegate(this,(TDelegate)OnSlot3),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton4	 ( this, IDC_Slot4,	FDelegate(this,(TDelegate)OnSlot4),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton5	 ( this, IDC_Slot5,	FDelegate(this,(TDelegate)OnSlot5),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SlotButton6	 ( this, IDC_Slot6,	FDelegate(this,(TDelegate)OnSlot6),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, Owner          ( InOwner )
	{ }
	
	WBitmapButton& GetButton( int i )
	{
		switch( i )
		{
			case 0: return SlotButton1;
			case 1: return SlotButton1;
			case 2: return SlotButton2;
			case 3: return SlotButton3;
			case 4: return SlotButton4;
			case 5: return SlotButton5;
			case 6: return SlotButton6;
		}
		return SlotButton1;
	}

	void OnCurrent()
	{	
		// Setup title and instruction labels
		Owner->SetText( Localize(TEXT("IDDIALOG_FrontEndPageLoadGame"),TEXT("IDC_Title"),TEXT("Startup")) );
		Owner->Instructions.SetText( Localize(TEXT("IDDIALOG_FrontEndPageLoadGame"),TEXT("IDC_Instructions"),TEXT("Startup")) );
	}

	void OnInitDialog()
	{
		int i;

		WFrontEndPage::OnInitDialog();
		
		// Setup our background bitmap
		SetBitmap( Owner->BackgroundBitmap.GetBitmapHandle() );
		
		// Setup the Save Slot buttons
		for(i=1; i<NUM_SAVE_SLOTS; ++i )
		{
			TCHAR text[512];
			if( Owner->SaveSlots[i].bEmpty )
			{
				appSprintf( text, TEXT("%d - %s"), i, Localize(TEXT("IDDIALOG_FrontEndPageLoadGame"),TEXT("IDC_SlotEmpty"),TEXT("Startup")) );

				// disable button
				EnableWindow( GetButton(i).hWnd, false );
			}
			else
			{
				appSprintf( text, TEXT("%d - %s"), i, Localize(TEXT("IDDIALOG_FrontEndPageLoadGame"),TEXT("IDC_SlotUsed"),TEXT("Startup")) );
				GetButton(i).SetBitmap( &Owner->SaveSlots[i].Thumbnail );
			}
			
			GetButton(i).SetText( text );
		}
	}

	const TCHAR* GetOkText()
	{
		return NULL;
	}

	void OnSlot1()	{ OnSlot( 1 ); }
	void OnSlot2()	{ OnSlot( 2 ); }
	void OnSlot3()	{ OnSlot( 3 ); }
	void OnSlot4()	{ OnSlot( 4 ); }
	void OnSlot5()	{ OnSlot( 5 ); }
	void OnSlot6()	{ OnSlot( 6 ); }
	void OnSlot( INT iSlot )
	{
		// error checking
		if( Owner->SaveSlots[iSlot].bEmpty )
			return;
		
		// Launch our game using the specified slot!
		Owner->LaunchLoadGame( iSlot );	
	}
};


class WFrontEndPageConfigSound : public WFrontEndPage
{
	DECLARE_WINDOWCLASS(WFrontEndPageConfigSound,WFrontEndPage,Startup)
	WFrontEnd*		Owner;
	WLabel			LogoStatic;
	FWindowsBitmap	LogoBitmap;
	
	WButton			NoSoundButton, No3DSoundHardware;
	WLabel			SoundVolume, MusicVolume;
	WLabel			MLow,MHi,SLow,SHi;
	WTrackBar		SoundVolumeSlider, MusicVolumeSlider;

	WFrontEndPageConfigSound( WFrontEnd* InOwner )
	: WFrontEndPage		( TEXT("FrontEndPageConfigSound"), IDDIALOG_FrontEndPageConfigSound, InOwner )
	, LogoStatic		( this, IDC_Logo )
	, NoSoundButton		( this, IDC_NoSound )
	, No3DSoundHardware	( this, IDC_No3DSound )
	, SoundVolume		( this, IDC_SoundVolume )
	, SoundVolumeSlider ( this, IDC_SoundVolumeSlider )
	, MusicVolume		( this, IDC_MusicVolume )
	, MusicVolumeSlider	( this, IDC_MusicVolumeSlider )
	, MLow				( this, IDC_MLow )
	, MHi				( this, IDC_MHi  )
	, SLow				( this, IDC_SLow )
	, SHi				( this, IDC_SHi  )
	, Owner				( InOwner )
	{}
	
	void OnCurrent()
	{
		// Setup title and instruction labels
		Owner->SetText( Localize(TEXT("IDDIALOG_FrontEndPageConfigSound"),TEXT("IDC_Title"),TEXT("Startup")) );
		Owner->Instructions.SetText( Localize(TEXT("IDDIALOG_FrontEndPageConfigSound"),TEXT("IDC_Instructions"),TEXT("Startup")) );
		Owner->SetBackgroundColor( 0, false );
	}

	void OnInitDialog()
	{
		WFrontEndPage::OnInitDialog();
		
		// Set our logoBitmap
		LogoBitmap.LoadFile( TEXT("..\\Help\\Logo.bmp") );
		SendMessageX( LogoStatic, STM_SETIMAGE, IMAGE_BITMAP, (LPARAM)LogoBitmap.GetBitmapHandle() );
		
		// Setup NoSoundButton
		FString UseSound = GConfig->GetStr(TEXT("Engine.GameEngine"),TEXT("UseSound"));
		if( UseSound == TEXT("True") )
			SendMessageX( NoSoundButton,   BM_SETCHECK, 0, 0 );
		else
			SendMessageX( NoSoundButton,   BM_SETCHECK, 1, 0 );
		
		// Setup UseEAX
		FString UseEAX = GConfig->GetStr(TEXT("ALAudio.ALAudioSubsystem"),TEXT("UseEAX"));
		if( UseEAX == TEXT("True") )
			SendMessageX( No3DSoundHardware,   BM_SETCHECK, 0, 0 );
		else
			SendMessageX( No3DSoundHardware,   BM_SETCHECK, 1, 0 );
		
		// Setup sound volume slider
		FString CurSoundVolume = GConfig->GetStr(TEXT("ALAudio.ALAudioSubsystem"),TEXT("SoundVolume"));
		SoundVolumeSlider.SetTicFreq( 1 );
		SoundVolumeSlider.SetRange( 0, 100 );
		SoundVolumeSlider.SetPos( appAtof(*CurSoundVolume)*100 );
		SoundVolumeSlider.ThumbTrackDelegate    = FDelegate(this, (TDelegate)OnSoundVolumeSliderTrack);
		SoundVolumeSlider.ThumbPositionDelegate = FDelegate(this, (TDelegate)OnSoundVolumeSliderTrack);
		
		// Setup sound volume label
		TCHAR buffer[512];
		appSprintf(buffer, TEXT("%s - %d %%"), 
			Localize(TEXT("IDDIALOG_FrontEndPageConfigSound"),TEXT("IDC_SoundVolume"),TEXT("Startup")), 
			SoundVolumeSlider.GetPos() );
		SoundVolume.SetText( buffer );
		SLow.SetText( Localize(TEXT("All"),TEXT("Options_0005"),TEXT("HPMenu")) ); // Low label
		SHi.SetText(  Localize(TEXT("All"),TEXT("Options_0002"),TEXT("HPMenu")) ); // Hi  label
		
		// Setup music volume slider
		FString CurMusicVolume = GConfig->GetStr(TEXT("ALAudio.ALAudioSubsystem"),TEXT("MusicVolume"));
		MusicVolumeSlider.SetTicFreq( 1 );
		MusicVolumeSlider.SetRange( 0, 100 );
		MusicVolumeSlider.SetPos( appAtof(*CurMusicVolume)*100 );
		MusicVolumeSlider.ThumbTrackDelegate    = FDelegate(this, (TDelegate)OnMusicVolumeSliderTrack);
		MusicVolumeSlider.ThumbPositionDelegate = FDelegate(this, (TDelegate)OnMusicVolumeSliderTrack);
		
		// Setup music volume label
		appSprintf(buffer, TEXT("%s - %d %%"), 
			Localize(TEXT("IDDIALOG_FrontEndPageConfigSound"),TEXT("IDC_MusicVolume"), TEXT("Startup")), 
			MusicVolumeSlider.GetPos() );
		MusicVolume.SetText( buffer );
		MLow.SetText( Localize(TEXT("All"),TEXT("Options_0005"),TEXT("HPMenu")) ); // Low label
		MHi.SetText(  Localize(TEXT("All"),TEXT("Options_0002"),TEXT("HPMenu")) ); // Hi  label
	}
	
	void OnSoundVolumeSliderTrack()
	{
		TCHAR buffer[512];
		appSprintf(buffer, TEXT("%s - %d %%"), 
			Localize(TEXT("IDDIALOG_FrontEndPageConfigSound"),TEXT("IDC_SoundVolume"),TEXT("Startup")), 
			SoundVolumeSlider.GetPos() );
		SoundVolume.SetText( buffer );
	}
	
	void OnMusicVolumeSliderTrack()
	{
		TCHAR buffer[512];
		appSprintf(buffer, TEXT("%s - %d %%"), 
			Localize(TEXT("IDDIALOG_FrontEndPageConfigSound"),TEXT("IDC_MusicVolume"),TEXT("Startup")), 
			MusicVolumeSlider.GetPos() );
		MusicVolume.SetText( buffer );
	}

	void OnOk()
	{
		guard(WFrontEndPageConfigSound::OnBack);
		
		// --- Set UseSound
		if( SendMessageX(NoSoundButton,BM_GETCHECK,0,0)==BST_CHECKED )
			GConfig->SetString(TEXT("Engine.GameEngine"),TEXT("UseSound"),TEXT("False") );
		else
			GConfig->SetString(TEXT("Engine.GameEngine"),TEXT("UseSound"),TEXT("True") );
		
		// --- Set Use3DSound
		if( SendMessageX(No3DSoundHardware,BM_GETCHECK,0,0)==BST_CHECKED )
			GConfig->SetString(TEXT("ALAudio.ALAudioSubsystem"),TEXT("UseEAX"),TEXT("False") );
		else
			GConfig->SetString(TEXT("ALAudio.ALAudioSubsystem"),TEXT("UseEAX"),TEXT("True") );
		
		// --- Set Sound and music volume
		TCHAR buffer[32];
		appSprintf(buffer, TEXT("%f"), ((float)SoundVolumeSlider.GetPos()/100) );
		GConfig->SetString(TEXT("ALAudio.ALAudioSubsystem"),TEXT("SoundVolume"),buffer );
		
		appSprintf(buffer, TEXT("%f"), ((float)MusicVolumeSlider.GetPos()/100) );
		GConfig->SetString(TEXT("ALAudio.ALAudioSubsystem"),TEXT("MusicVolume"),buffer );
		
		unguard;
	}
};


class WFrontEndPageConfigVideo : public WFrontEndPage
{
	DECLARE_WINDOWCLASS(WFrontEndPageConfigVideo,WFrontEndPage,Startup)
	WFrontEnd* Owner;
	WLabel			LogoStatic;
	FWindowsBitmap	LogoBitmap;
	WLabel			Resolution;
	WListBox		ResolutionList;
	WButton			D3DRenderer, SoftwareRenderer;
	WButton			WindowButton;
	UBOOL			bStartupFullscreen;
	
	FString			GameRenderDriver;

	WFrontEndPageConfigVideo( WFrontEnd* InOwner )
	: WFrontEndPage		( TEXT("FrontEndPageConfigVideo"), IDDIALOG_FrontEndPageConfigVideo, InOwner )
	, Owner				( InOwner )
	, LogoStatic		( this, IDC_Logo )
	, D3DRenderer		( this, IDC_D3DRenderer,FDelegate(this,(TDelegate)OnD3DRenderer) )
	, SoftwareRenderer	( this, IDC_SoftwareRenderer,FDelegate(this,(TDelegate)OnSoftwareRenderer) )
	, Resolution		( this, IDC_Resolution )
	, ResolutionList	( this, IDC_ResolutionList )
	, WindowButton		( this, IDC_Window )
	{}
	
	void OnCurrent()
	{
		// Setup title and instruction labels
		Owner->SetText( Localize(TEXT("IDDIALOG_FrontEndPageConfigVideo"),TEXT("IDC_Title"),TEXT("Startup")) );
		Owner->Instructions.SetText( Localize(TEXT("IDDIALOG_FrontEndPageConfigVideo"),TEXT("IDC_Instructions"),TEXT("Startup")) );
		Owner->SetBackgroundColor( 0, false );
	}
	
	void OnInitDialog()
	{
		WFrontEndPage::OnInitDialog();
		
		// Set our logoBitmap
		LogoBitmap.LoadFile( TEXT("..\\Help\\Logo.bmp") );
		SendMessageX( LogoStatic, STM_SETIMAGE, IMAGE_BITMAP, (LPARAM)LogoBitmap.GetBitmapHandle() );

		GameRenderDriver = GConfig->GetStr(TEXT("Engine.Engine"),TEXT("GameRenderDevice"));
		
		Resolution.SetText( Localize(TEXT("IDDIALOG_FrontEndPageConfigVideo"),TEXT("IDC_Resolution"),TEXT("Startup")) );
		
		// Call UseD3DDriver() which will detect if we can use hardware or not.
		if( !UseD3DDriver() )
		{
			// *** FORCE SOFTWARE MODE

			// Disable the D3DRenderer button (this user can't use hardware)
			EnableWindow( D3DRenderer.hWnd, false );
			
			// check the software renderer mode checkbox
			SendMessageX(SoftwareRenderer,BM_SETCHECK,BST_CHECKED,0);
			
			// setup our .ini file as though we are using software mode
			GameRenderDriver = TEXT("SoftDrv.SoftwareRenderDevice");
			OnOk();

		}
		else // detect what the current settings are
		{
			// Initilize the current render device
			if( GameRenderDriver==TEXT("SoftDrv.SoftwareRenderDevice") )
			{
				SendMessageX(SoftwareRenderer,BM_SETCHECK,BST_CHECKED,0);
			}
			else if( GameRenderDriver==TEXT("D3DDrv.D3DRenderDevice") )
			{
				SendMessageX(D3DRenderer,BM_SETCHECK,BST_CHECKED,0);			
			}
		}
		
		// Setup our fullscreen checkBox
		FString StartupFullscreen = GConfig->GetStr(TEXT("WinDrv.WindowsClient"),TEXT("StartupFullscreen"));

		if( StartupFullscreen == TEXT("True") )
			SendMessageX( WindowButton,   BM_SETCHECK, 0, 0 );
		else
			SendMessageX( WindowButton,   BM_SETCHECK, 1, 0 );
		
		// if we are not in debug mode, hide the (windowed) checkbox
		FString InDebugMode = GConfig->GetStr(TEXT("HGame.baseConsole"),TEXT("bDebugMode"));
		if( InDebugMode != TEXT("True") )
		{
			ShowWindow( WindowButton.hWnd, SW_HIDE );
		}
		
		RefreshList();
	}

	void OnD3DRenderer()
	{
		// Set D3DRenderer
		GameRenderDriver = TEXT("D3DDrv.D3DRenderDevice");
		RefreshList();
	}
	
	void OnSoftwareRenderer()
	{
		// Set SoftwareRenderer
		GameRenderDriver = TEXT("SoftDrv.SoftwareRenderDevice");
		RefreshList();
	}
	
	void RefreshList( )
	{
		ResolutionList.Empty();

		if( GameRenderDriver==TEXT("D3DDrv.D3DRenderDevice") )
		{
			ResolutionList.InsertString( 0, TEXT("640x480")   );
			ResolutionList.InsertString( 1, TEXT("800x600")   );
			ResolutionList.InsertString( 2, TEXT("1024x768")  );
			ResolutionList.InsertString( 3, TEXT("1280x1024") );
		}
		else if( GameRenderDriver==TEXT("SoftDrv.SoftwareRenderDevice") )
		{
//			ResolutionList.InsertString( 0, TEXT("320x240") );
			ResolutionList.InsertString( 0, TEXT("512x384") );
		}
		else
		{
			ResolutionList.InsertString( 0, TEXT("640x480")   );
		}

		FString strDefault;
		if( SendMessageX(WindowButton,BM_GETCHECK,0,0)==BST_CHECKED )
		{
			strDefault =  GConfig->GetStr(TEXT("WinDrv.WindowsClient"),TEXT("WindowedViewportX"));
			strDefault += TEXT("x");
			strDefault += GConfig->GetStr(TEXT("WinDrv.WindowsClient"),TEXT("WindowedViewportY"));
		}
		else
		{
			strDefault =  GConfig->GetStr(TEXT("WinDrv.WindowsClient"),TEXT("FullscreenViewportX"));
			strDefault += TEXT("x");
			strDefault += GConfig->GetStr(TEXT("WinDrv.WindowsClient"),TEXT("FullscreenViewportY"));
		}
		
		int DefaultIndex = 0;
		if( strDefault!=TEXT("") )
		{
			INT Result = SendMessageLX( ResolutionList, LB_FINDSTRING, -1, *strDefault );
			if( Result >= 0 )
				DefaultIndex = Result;
		}
		ResolutionList.SetCurrent(DefaultIndex, 1);
	}

	void OnOk()
	{
		guard(WFrontEndPageConfigVideo::OnBack);		
		// *** Set Video Settins
		
		// --- Set StartupFullscreen
		if( SendMessageX(WindowButton,BM_GETCHECK,0,0)==BST_CHECKED )
			GConfig->SetString(TEXT("WinDrv.WindowsClient"),TEXT("StartupFullscreen"),TEXT("False") );
		else
			GConfig->SetString(TEXT("WinDrv.WindowsClient"),TEXT("StartupFullscreen"),TEXT("True") );
		
		// --- Get Resolution settings
		FString ResX, ResY;
		if( GameRenderDriver == TEXT("D3DDrv.D3DRenderDevice") )
		{
			switch( ResolutionList.GetCurrent() )
			{
				default:
				case 0: ResX = TEXT("640");  ResY = TEXT("480");  break;
				case 1: ResX = TEXT("800");  ResY = TEXT("600");  break;
				case 2: ResX = TEXT("1024"); ResY = TEXT("768");  break;
				case 3: ResX = TEXT("1280"); ResY = TEXT("1024"); break;
			}
		}
		else if( GameRenderDriver == TEXT("SoftDrv.SoftwareRenderDevice") )
		{
			switch( ResolutionList.GetCurrent() )
			{
				default:
//				case 0: ResX = TEXT("320");  ResY = TEXT("240");  break;
				case 0: ResX = TEXT("512");  ResY = TEXT("384");  break;
			}

			// --- Set BitDepth ( default to 16-bit with software just like last year )
			GConfig->SetString(TEXT("WinDrv.WindowsClient"),TEXT("FullscreenColorBits"), TEXT("16") );
			GConfig->SetString(TEXT("WinDrv.WindowsClient"),TEXT("WindowedColorBits"),   TEXT("16") );
		}
		else
		{
			// Error ( unsupported driver )
			return;
		}

		// --- Set Resolution settings
		GConfig->SetString(TEXT("WinDrv.WindowsClient"),TEXT("FullscreenViewportX"), *ResX );
		GConfig->SetString(TEXT("WinDrv.WindowsClient"),TEXT("FullscreenViewportY"), *ResY );
		GConfig->SetString(TEXT("WinDrv.WindowsClient"),TEXT("WindowedViewportX"),   *ResX );
		GConfig->SetString(TEXT("WinDrv.WindowsClient"),TEXT("WindowedViewportY"),   *ResY );
		
		// --- Set Renderer
		GConfig->SetString(TEXT("Engine.Engine"),TEXT("GameRenderDevice"),*GameRenderDriver);
		
		unguard;
	}
};

class WFrontEndPageConfig : public WFrontEndPage
{
	DECLARE_WINDOWCLASS(WFrontEndPageConfig,WFrontEndPage,Startup)
	WFrontEnd*		Owner;
	WBitmapButton	VideoConfigButton,SoundConfigButton,JoystickConfigButton;

	WFrontEndPageConfig( WFrontEnd* InOwner )
	: WFrontEndPage			( TEXT("FrontEndPageConfig"), IDDIALOG_FrontEndPageConfig, InOwner )
	, VideoConfigButton		( this, IDC_VideoConfig,	FDelegate(this,(TDelegate)OnVideo),		CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, SoundConfigButton		( this, IDC_SoundConfig,	FDelegate(this,(TDelegate)OnSound),		CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, JoystickConfigButton	( this, IDC_JoystickConfig,	FDelegate(this,(TDelegate)OnJoystick),	CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, Owner					( InOwner )
	{ }
	
	void OnCurrent()
	{
		// Setup title and instruction labels
		Owner->SetText( Localize(TEXT("IDDIALOG_FrontEndPageConfig"),TEXT("IDC_Title"),TEXT("Startup")) );
		Owner->Instructions.SetText( Localize(TEXT("IDDIALOG_FrontEndPageConfig"),TEXT("IDC_Instructions"),TEXT("Startup")) );
		Owner->SetBackgroundColor( RGB(9, 3, 11) );
	}

	void OnInitDialog()
	{
		WFrontEndPage::OnInitDialog();
		
		// Setup our background bitmap
		SetBitmap( Owner->BackgroundBitmap.GetBitmapHandle() );
		
		VideoConfigButton.SetBitmap( Owner->BitmapButtonUp.GetBitmapHandle() );
		SoundConfigButton.SetBitmap( Owner->BitmapButtonUp.GetBitmapHandle() );
		JoystickConfigButton.SetBitmap( Owner->BitmapButtonUp.GetBitmapHandle() );
				
		VideoConfigButton.SetText(	 Localize(TEXT("IDDIALOG_FrontEndPageConfig"),TEXT("IDC_VideoConfig"),TEXT("Startup"))       );
		SoundConfigButton.SetText(	 Localize(TEXT("IDDIALOG_FrontEndPageConfig"),TEXT("IDC_SoundConfig"),TEXT("Startup"))      );
		JoystickConfigButton.SetText(Localize(TEXT("IDDIALOG_FrontEndPageConfig"),TEXT("IDC_JoystickConfig"),TEXT("Startup")) );	
	}
	
	const TCHAR* GetOkText()
	{
		return NULL;
	}
	
	void OnVideo()	
	{ 
		Owner->Advance( new WFrontEndPageConfigVideo(Owner) );
	}
	
	void OnSound()	
	{
		Owner->Advance( new WFrontEndPageConfigSound(Owner) );
	}
	
	void OnJoystick()	
	{
		ShellExecuteX( NULL, TEXT("open"), TEXT("control.exe"), TEXT("joy.cpl"), appBaseDir(), SW_SHOWNORMAL );
	}
};

class WFrontEndPageMainMenu : public WFrontEndPage
{
	DECLARE_WINDOWCLASS(WFrontEndPageMainMenu,WFrontEndPage,Startup)
	WFrontEnd*		Owner;
	WBitmapButton	LoadGameButton, NewGameButton, ConfigButton;

	WFrontEndPageMainMenu( WFrontEnd* InOwner )
	: WFrontEndPage  ( TEXT("FrontEndPageMainMenu"), IDDIALOG_FrontEndPageMainMenu, InOwner )
	, Owner          ( InOwner )
	, NewGameButton  ( this, IDC_NewGame,   FDelegate(this,(TDelegate)OnNewGame),  CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, LoadGameButton ( this, IDC_LoadGame,  FDelegate(this,(TDelegate)OnLoadGame), CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	, ConfigButton	 ( this, IDC_Config,	FDelegate(this,(TDelegate)OnConfig)  , CBFF_ShowOver|CBFF_DimAway, 0xFFFFFF )
	{}
	
	void OnCurrent()
	{
		// Setup title and instruction labels
		Owner->SetText( Localize(TEXT("IDDIALOG_FrontEndPageMainMenu"),TEXT("IDC_Title"),TEXT("Startup")) );
		Owner->Instructions.SetText( Localize(TEXT("IDDIALOG_FrontEndPageMainMenu"),TEXT("IDC_Instructions"),TEXT("Startup")) );		
	}
	
	void OnInitDialog()
	{
		WFrontEndPage::OnInitDialog();
		
		SetBitmap( Owner->BackgroundBitmap.GetBitmapHandle() );

		NewGameButton.SetBitmap(  Owner->BitmapButtonUp.GetBitmapHandle() );
		LoadGameButton.SetBitmap( Owner->BitmapButtonUp.GetBitmapHandle() );
		ConfigButton.SetBitmap(   Owner->BitmapButtonUp.GetBitmapHandle() );

		//	See if we need to detect the user's video config.  We need to do this before we display
		//	the video options page.
		//
		INT	iVendorID;
		UBOOL bFound = GConfig->GetInt(TEXT("D3DDrv.D3DRenderDevice"),TEXT("dwVendorId"), iVendorID);
		if (!bFound)
			CreateDetected();
		
		NewGameButton.SetText(  Localize(TEXT("IDDIALOG_FrontEndPageMainMenu"),TEXT("IDC_NewGame"),TEXT("Startup"))       );
		LoadGameButton.SetText( Localize(TEXT("IDDIALOG_FrontEndPageMainMenu"),TEXT("IDC_LoadGame"),TEXT("Startup"))      );
		ConfigButton.SetText(   Localize(TEXT("IDDIALOG_FrontEndPageMainMenu"),TEXT("IDC_Options"),TEXT("Startup")) );
	}

	const TCHAR* GetOkText()
	{
		return TEXT("&Quit");
	}
	
	void OnOk()
	{
		// close our front end
		OnClose();
	}
	
	void OnNewGame()
	{
		Owner->Advance( new WFrontEndPageNewGame(Owner) );
	}
	void OnLoadGame()
	{
		Owner->Advance( new WFrontEndPageLoadGame(Owner) );
	}
	void OnConfig()
	{
		Owner->Advance( new WFrontEndPageConfig(Owner) );
	}
};



/*-----------------------------------------------------------------------------
	Config wizard.
-----------------------------------------------------------------------------*/

class WConfigWizard : public WWizardDialog
{
	DECLARE_WINDOWCLASS(WConfigWizard,WWizardDialog,Startup)
	WLabel LogoStatic;
	FWindowsBitmap LogoBitmap;
	UBOOL Cancel;
	FString Title;
	WConfigWizard()
	: LogoStatic(this,IDC_Logo)
	, Cancel(0)
	{
		InitSysDirs();
	}
	void OnInitDialog()
	{
		guard(WStartupWizard::OnInitDialog);
		WWizardDialog::OnInitDialog();
		SendMessageX( *this, WM_SETICON, ICON_BIG, (WPARAM)LoadIconIdX(hInstance,IDICON_Mainframe) );
		LogoBitmap.LoadFile( TEXT("..\\Help\\Logo.bmp") );
		SendMessageX( LogoStatic, STM_SETIMAGE, IMAGE_BITMAP, (LPARAM)LogoBitmap.GetBitmapHandle() );
		SetText( *Title );
		SetForegroundWindow( hWnd );
		unguard;
	}
};

class WConfigPageFirstTime : public WWizardPage
{
	DECLARE_WINDOWCLASS(WConfigPageFirstTime,WWizardPage,Startup)
	WConfigWizard* Owner;
	WConfigPageFirstTime( WConfigWizard* InOwner )
	: WWizardPage( TEXT("ConfigPageFirstTime"), IDDIALOG_ConfigPageFirstTime, InOwner )
	, Owner(InOwner)
	{}
	const TCHAR* GetNextText()
	{
		return LocalizeGeneral(TEXT("Run"),TEXT("Startup"));
	}
	WWizardPage* GetNext()
	{
		Owner->EndDialog(1);
		return NULL;
	}	
};

class WConfigPageSafeOptions : public WWizardPage
{
	DECLARE_WINDOWCLASS(WConfigPageSafeOptions,WWizardPage,Startup)
	WConfigWizard* Owner;
	WButton NoSoundButton, No3DSoundButton, No3DVideoButton, WindowButton, ResButton, ResetConfigButton, NoProcessorButton, NoJoyButton;
	WConfigPageSafeOptions( WConfigWizard* InOwner )
	: WWizardPage		( TEXT("ConfigPageSafeOptions"), IDDIALOG_ConfigPageSafeOptions, InOwner )
	, Owner				(InOwner)
	, NoSoundButton		(this,IDC_NoSound)
	, No3DSoundButton	(this,IDC_No3DSound)
	, No3DVideoButton	(this,IDC_No3dVideo)
	, WindowButton		(this,IDC_Window)
	, ResButton			(this,IDC_Res)
	, ResetConfigButton	(this,IDC_ResetConfig)
	, NoProcessorButton	(this,IDC_NoProcessor)
	, NoJoyButton		(this,IDC_NoJoy)
	{}
	void OnInitDialog()
	{
		WWizardPage::OnInitDialog();
		SendMessageX( NoSoundButton,     BM_SETCHECK, 1, 0 );
		SendMessageX( No3DSoundButton,   BM_SETCHECK, 1, 0 );
		SendMessageX( No3DVideoButton,   BM_SETCHECK, 1, 0 );
		SendMessageX( WindowButton,      BM_SETCHECK, 1, 0 );
		SendMessageX( ResButton,         BM_SETCHECK, 1, 0 );
		SendMessageX( ResetConfigButton, BM_SETCHECK, 0, 0 );
		SendMessageX( NoProcessorButton, BM_SETCHECK, 1, 0 );
		SendMessageX( NoJoyButton,       BM_SETCHECK, 1, 0 );
	}
	const TCHAR* GetNextText()
	{
		return LocalizeGeneral(TEXT("Run"),TEXT("Startup"));
	}
	WWizardPage* GetNext()
	{
		FString CmdLine;
		if( SendMessageX(NoSoundButton,BM_GETCHECK,0,0)==BST_CHECKED )
			CmdLine+=TEXT(" -nosound");
		if( SendMessageX(No3DSoundButton,BM_GETCHECK,0,0)==BST_CHECKED )
			CmdLine+=TEXT(" -no3dsound");
		if( SendMessageX(No3DSoundButton,BM_GETCHECK,0,0)==BST_CHECKED )
			CmdLine+=TEXT(" -nohard");
		if( SendMessageX(No3DSoundButton,BM_GETCHECK,0,0)==BST_CHECKED )
			CmdLine+=TEXT(" -nohard -noddraw");
		if( SendMessageX(No3DSoundButton,BM_GETCHECK,0,0)==BST_CHECKED )
			CmdLine+=TEXT(" -defaultres");
		if( SendMessageX(NoProcessorButton,BM_GETCHECK,0,0)==BST_CHECKED )
			CmdLine+=TEXT(" -nommx -nokni -nok6");
		if( SendMessageX(NoJoyButton,BM_GETCHECK,0,0)==BST_CHECKED )
			CmdLine+=TEXT(" -nojoy");
		if( SendMessageX(ResetConfigButton,BM_GETCHECK,0,0)==BST_CHECKED )
			GFileManager->Delete( *(FString(appPackage())+TEXT(".ini")) );
		ShellExecuteX( NULL, TEXT("open"), ThisFile, *CmdLine, appBaseDir(), SW_SHOWNORMAL );
		Owner->EndDialog(0);
		return NULL;
	}
};

class WConfigPageDetail : public WWizardPage
{
	DECLARE_WINDOWCLASS(WConfigPageDetail,WWizardPage,Startup)
	WConfigWizard* Owner;
	WEdit DetailEdit;
	WConfigPageDetail( WConfigWizard* InOwner )
	: WWizardPage( TEXT("ConfigPageDetail"), IDDIALOG_ConfigPageDetail, InOwner )
	, Owner(InOwner)
	, DetailEdit(this,IDC_DetailEdit)
	{}
	void OnInitDialog()
	{
		WWizardPage::OnInitDialog();
		FString Info;

		INT DescFlags=0;
		FString Driver = GConfig->GetStr(TEXT("Engine.Engine"),TEXT("GameRenderDevice"));
		GConfig->GetInt(*Driver,TEXT("DescFlags"),DescFlags);

		// Frame rate dependent LOD.
		if( Driver==TEXT("SoftDrv.SoftwareRenderDevice") || 280.0*1000.0*1000.0*GSecondsPerCycle>1.f )
		{
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("MinDesiredFrameRate"), TEXT("20") );
		}
		else if( Driver==TEXT("D3DDrv.D3DRenderDevice") )
		{
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("MinDesiredFrameRate"), TEXT("28") );
		}

		// Sound quality.
		if( !GIsMMX || GPhysicalMemory <= 32*1024*1024 )
		{
			Info = Info + LocalizeGeneral(TEXT("SoundLow"),TEXT("Startup")) + TEXT("\r\n");
			GConfig->SetString( TEXT("ALAudio.ALAudioSubsystem"), TEXT("UseEAX"),				TEXT("False") );
			GConfig->SetString( TEXT("ALAudio.ALAudioSubsystem"), TEXT("CompatibilityMode"),	TEXT("True") );
			GConfig->SetString( TEXT("ALAudio.ALAudioSubsystem"), TEXT("UsePrecache"),			TEXT("False") );
			GConfig->SetString( TEXT("Botpack.TournamentPlayer"), TEXT("AnnouncerVolume"),		TEXT("false") );
			GConfig->SetString( TEXT("Botpack.TournamentPlayer"), TEXT("bNoVoiceTaunts"),		TEXT("true") );		
		}
		else
		{
			Info = Info + LocalizeGeneral(TEXT("SoundHigh"),TEXT("Startup")) + TEXT("\r\n");
		}

		// Skins.
		if( (GPhysicalMemory < 96*1024*1024) || (DescFlags&RDDESCF_LowDetailSkins) )
		{
			Info = Info + LocalizeGeneral(TEXT("SkinsLow"),TEXT("Startup")) + TEXT("\r\n");
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("SkinDetail"), TEXT("Medium") );
		}
		else
		{
			Info = Info + LocalizeGeneral(TEXT("SkinsHigh"),TEXT("Startup")) + TEXT("\r\n");
		}

		// World.
		if( (GPhysicalMemory < 64*1024*1024) || (DescFlags&RDDESCF_LowDetailWorld) )
		{
			Info = Info + LocalizeGeneral(TEXT("WorldLow"),TEXT("Startup")) + TEXT("\r\n");
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("TextureDetail"), TEXT("Medium") );
		}
		else
		{
			Info = Info + LocalizeGeneral(TEXT("WorldHigh"),TEXT("Startup")) + TEXT("\r\n");
		}

		// Resolution.
		if( (!GIsMMX || !GIsPentiumPro) && Driver==TEXT("SoftDrv.SoftwareRenderDevice") )
		{
			Info = Info + LocalizeGeneral(TEXT("ResLow"),TEXT("Startup")) + TEXT("\r\n");
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("WindowedViewportX"),  TEXT("512") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("WindowedViewportY"),  TEXT("384") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("WindowedColorBits"),  TEXT("16") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("FullscreenViewportX"), TEXT("512") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("FullscreenViewportY"), TEXT("384") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("FullscreenColorBits"), TEXT("16") );
		}
		else if( Driver==TEXT("SoftDrv.SoftwareRenderDevice") || (DescFlags&RDDESCF_LowDetailWorld) )
		{
			Info = Info + LocalizeGeneral(TEXT("ResLow"),TEXT("Startup")) + TEXT("\r\n");
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("WindowedViewportX"),  TEXT("512") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("WindowedViewportY"),  TEXT("384") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("WindowedColorBits"),  TEXT("16") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("FullscreenViewportX"), TEXT("512") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("FullscreenViewportY"), TEXT("384") );
			GConfig->SetString( TEXT("WinDrv.WindowsClient"), TEXT("FullscreenColorBits"), TEXT("16") );
		}
		else
		{
			Info = Info + LocalizeGeneral(TEXT("ResHigh"),TEXT("Startup")) + TEXT("\r\n");
		}
		DetailEdit.SetText(*Info);
	}
	WWizardPage* GetNext()
	{
		return new WConfigPageFirstTime(Owner);
	}
};

class WConfigPageDriver : public WWizardPage
{
	DECLARE_WINDOWCLASS(WConfigPageDriver,WWizardPage,Startup)
	WConfigWizard* Owner;
	WUrlButton WebButton;
	WLabel Card;
	WConfigPageDriver( WConfigWizard* InOwner )
	: WWizardPage( TEXT("ConfigPageDriver"), IDDIALOG_ConfigPageDriver, InOwner )
	, Owner(InOwner)
	, WebButton(this,LocalizeGeneral(TEXT("Direct3DWebPage"),TEXT("Startup")),IDC_WebButton)
	, Card(this,IDC_Card)
	{}
	void OnInitDialog()
	{
		WWizardPage::OnInitDialog();
		FString CardName=GConfig->GetStr(TEXT("D3DDrv.D3DRenderDevice"),TEXT("Description"));
		if( CardName!=TEXT("") )
			Card.SetText(*CardName);
	}
	WWizardPage* GetNext()
	{
		return new WConfigPageDetail(Owner);
	}	
};

class WConfigPageRenderer : public WWizardPage
{
	DECLARE_WINDOWCLASS(WConfigPageRenderer,WWizardPage,Startup)
	WConfigWizard* Owner;
	WListBox RenderList;
	WButton ShowCompatible, ShowAll;
	WLabel RenderNote;
	INT First;
	TArray<FRegistryObjectInfo> Classes;
	WConfigPageRenderer( WConfigWizard* InOwner )
	: WWizardPage( TEXT("ConfigPageRenderer"), IDDIALOG_ConfigPageRenderer, InOwner )
	, Owner(InOwner)
	, RenderList(this,IDC_RenderList)
	, RenderNote(this,IDC_RenderNote)
//	, ShowCompatible(this,IDC_Compatible,FDelegate(this,(TDelegate)RefreshConfig))
	, First(0)
	{}

	void RefreshConfig()
	{
		

		GConfig->GetInt( TEXT("FirstRun"), TEXT("Reconfig"), Reconfig );
		Reconfig=Reconfig^1;
		GConfig->SetInt( TEXT("FirstRun"), TEXT("Reconfig"), Reconfig );

	}
	void RefreshList()
	{
		RenderList.Empty();
		INT All=1, BestPriority=0;
		FString Default;
		Classes.Empty();
		BOOL	UseHardware = FALSE;	//commented out for EA change	UseD3DDriver();
		UObject::GetRegistryObjects( Classes, UClass::StaticClass(), URenderDevice::StaticClass(), 0 );
		for( TArray<FRegistryObjectInfo>::TIterator It(Classes); It; ++It )
		{
			FString Path=It->Object, Left, Right, Temp;
			if( Path.Split(TEXT("."),&Left,&Right) )
			{
				INT DoShow=All, Priority=0;
				INT DescFlags=0;
				GConfig->GetInt(*Path,TEXT("DescFlags"),DescFlags);
				if ((Left != TEXT("D3DDrv"))  &&  (Left != TEXT("SoftDrv")))
				{
					DoShow = 0;
				}
				else if
				(	It->Autodetect!=TEXT("")
				&& (GFileManager->FileSize(*FString::Printf(TEXT("%s\\%s"), SysDir, *It->Autodetect))>=0
				||  GFileManager->FileSize(*FString::Printf(TEXT("%s\\%s"), WinDir, *It->Autodetect))>=0) )
					DoShow = Priority = 3;
				else if( DescFlags & RDDESCF_Certified )
					DoShow = Priority = 2;
				else if( Path==TEXT("SoftDrv.SoftwareRenderDevice") )
				{
					if (UseHardware)	// this will be a test for various cards
					{
						DoShow = Priority = 1;
					}
					else
					{
						DoShow = Priority = 4;
					}
				}

				if( DoShow )
				{
					RenderList.AddString( *(Temp=Localize(*Right,TEXT("ClassCaption"),*Left)) );
					if( Priority>=BestPriority )
						{Default=Temp; BestPriority=Priority;}
				}
				
			}
		}
		if( Default!=TEXT("") )
			RenderList.SetCurrent(RenderList.FindStringChecked(*Default),1);
		CurrentChange();
	}
	void CurrentChange()
	{
	//	RenderNote.SetText(Localize(TEXT("Descriptions"),*CurrentDriver(),TEXT("Startup"),NULL,1)); 
	}
	void OnPaint()
	{
		if( !First++ )
		{
			UpdateWindow( *this );
			CreateDetected();
			RefreshList();
		}
	}
	void OnCurrent()
	{
		guard(WFilerPageInstallProgress::OnCurrent);
		unguard;
	}
	void OnInitDialog()
	{
		WWizardPage::OnInitDialog();
	//	SendMessageX(ShowCompatible,BM_SETCHECK,BST_CHECKED,0);
		RenderList.SelectionChangeDelegate = FDelegate(this,(TDelegate)CurrentChange);
		RenderList.DoubleClickDelegate = FDelegate(Owner,(TDelegate)WWizardDialog::OnNext);
		RenderList.AddString( LocalizeGeneral(TEXT("Detecting"),TEXT("Startup")) );
	}
	FString CurrentDriver()
	{
		if( RenderList.GetCurrent()>=0 )
		{
			FString Name = RenderList.GetString(RenderList.GetCurrent());
			for( TArray<FRegistryObjectInfo>::TIterator It(Classes); It; ++It )
			{
				FString Path=It->Object, Left, Right, Temp;
				if( Path.Split(TEXT("."),&Left,&Right) )
					if( Name==Localize(*Right,TEXT("ClassCaption"),*Left) )
						return Path;
			}
		}
		return TEXT("");
	}


	const TCHAR* GetNextText()
	{
		//return LocalizeGeneral("NextButton",TEXT("Window"));
		return NULL;
	}

	const TCHAR* GetFinishText()
	{
		FString lang = UObject::GetLanguage();
		if(lang==TEXT("Jap"))
		{
			return LocalizeGeneral(TEXT("FinishText"),TEXT("Startup"));
		//	return LocalizeGeneral(TEXT("FirstTime"),TEXT("Startup"));
		}
		return TEXT("_Start_");//Localize(TEXT("TEXT"),TEXT("main_menu_03"),TEXT("HGame"),NULL,1);
	//	return LocalizeGeneral("Main_Menu_03",TEXT("HPMENU"));
	}

	void OnFinish()
	{
	//	guard(WWizardDialog::OnFinish);
		if( CurrentDriver()!=TEXT("") )
			GConfig->SetString(TEXT("Engine.Engine"),TEXT("GameRenderDevice"),*CurrentDriver());
		
//		unguard;
	}


	WWizardPage* GetNext()
	{
		if( CurrentDriver()!=TEXT("") )
			GConfig->SetString(TEXT("Engine.Engine"),TEXT("GameRenderDevice"),*CurrentDriver());
		if( CurrentDriver()==TEXT("D3DDrv.D3DRenderDevice") )
			return new WConfigPageDriver(Owner);
		else
			return new WConfigPageDetail(Owner);
	}
};




class WConfigPageSafeMode : public WWizardPage
{
	DECLARE_WINDOWCLASS(WConfigPageSafeMode,WWizardPage,Startup)
	WConfigWizard* Owner;
	WBitmapButton RunButton, VideoButton, SafeModeButton, WebButton;
	WConfigPageSafeMode( WConfigWizard* InOwner )
	: WWizardPage    ( TEXT("ConfigPageSafeMode"), IDDIALOG_ConfigPageSafeMode, InOwner )
	, RunButton      ( this, IDC_Run,      FDelegate(this,(TDelegate)OnRun) )
	, VideoButton    ( this, IDC_Video,    FDelegate(this,(TDelegate)OnVideo) )
	, SafeModeButton ( this, IDC_SafeMode, FDelegate(this,(TDelegate)OnSafeMode) )
	, WebButton      ( this, IDC_Web,      FDelegate(this,(TDelegate)OnWeb) )
	, Owner          (InOwner)
	{}
	void OnRun()
	{
		Owner->EndDialog(1);
	}
	void OnVideo()
	{
		Owner->Advance( new WConfigPageRenderer(Owner) );
	}
	void OnSafeMode()
	{
		Owner->Advance( new WConfigPageSafeOptions(Owner) );
	}
	void OnWeb()
	{
		ShellExecuteX( *this, TEXT("open"), LocalizeGeneral(TEXT("WebPage"),TEXT("Startup")), TEXT(""), appBaseDir(), SW_SHOWNORMAL );
		Owner->EndDialog(0);
	}
	const TCHAR* GetNextText()
	{
		return NULL;
	}
};

/*-----------------------------------------------------------------------------
	Launch mplayer.com.
	- by Jack Porter
	- Based on mp_launch2.c by Rich Rice --rich@mpath.com
-----------------------------------------------------------------------------*/

#define MPI_FILE TEXT("mput.mpi")
#define MPLAYNOW_EXE TEXT("mplaynow.exe")

static int GetMplayerDirectory(TCHAR *mplayer_directory)
{
	HKEY hkey;
//	HKEY key = HKEY_LOCAL_MACHINE;
	HKEY key = HKEY_CURRENT_USER;
	TCHAR subkey[]=TEXT("software\\mpath\\mplayer\\main");
	TCHAR valuename[]=TEXT("root directory");
	TCHAR buffer[MAX_PATH];
	DWORD dwType, dwSize;
	
	if( RegOpenKeyExX(key, subkey, 0, KEY_READ, &hkey) == ERROR_SUCCESS )
	{
		dwSize = MAX_PATH;
		if( RegQueryValueExX(hkey, valuename, 0, &dwType, (LPBYTE) buffer, &dwSize) == ERROR_SUCCESS )
		{
			appSprintf(mplayer_directory, TEXT("%s"), buffer);
			return 1;
		}
		RegCloseKey(hkey);
	}

	return 0;
}

static void LaunchMplayer()
{
	TCHAR mplaunch_exe[MAX_PATH], mplayer_directory[MAX_PATH];

	if( GetMplayerDirectory(mplayer_directory) )
	{
		appSprintf( mplaunch_exe, TEXT("%s\\programs\\mplaunch.exe"), mplayer_directory );
		if( GFileManager->FileSize(mplaunch_exe)>0 )
		{
			appLaunchURL( mplaunch_exe, MPI_FILE );
			return;
		}
	}

	appLaunchURL( MPLAYNOW_EXE, TEXT("") );
}

#undef MPI_FILE
#undef MPLAYNOW_EXE

/*-----------------------------------------------------------------------------
	Exec hook.
-----------------------------------------------------------------------------*/

// FExecHook.
class FExecHook : public FExec, public FNotifyHook
{
private:
	WConfigProperties* Preferences;
	void NotifyDestroy( void* Src )
	{
		if( Src==Preferences )
			Preferences = NULL;
	}
	UBOOL Exec( const TCHAR* Cmd, FOutputDevice& Ar )
	{
		guard(FExecHook::Exec);
		if( ParseCommand(&Cmd,TEXT("ShowLog")) )
		{
			if( GLogWindow )
			{
				GLogWindow->Show(1);
				SetFocus( *GLogWindow );
				GLogWindow->Display.ScrollCaret();
			}
			return 1;
		}
		else if( ParseCommand(&Cmd,TEXT("TakeFocus")) )
		{
			TObjectIterator<UEngine> EngineIt;
			if
			(	EngineIt
			&&	EngineIt->Client
			&&	EngineIt->Client->Viewports.Num() )
				SetForegroundWindow( (HWND)EngineIt->Client->Viewports(0)->GetWindow() );
			return 1;
		}
		else if( ParseCommand(&Cmd,TEXT("EditActor")) )
		{
#if 1 //Fix added by Legend on 4/12/2000
			UClass* Class;
			FName ActorName;
			TObjectIterator<UEngine> EngineIt;

			AActor* Found = NULL;

			if( EngineIt && ParseObject<UClass>( Cmd, TEXT("Class="), Class, ANY_PACKAGE ) )
			{
				AActor* Player  = EngineIt->Client ? EngineIt->Client->Viewports(0)->Actor : NULL;
				FLOAT   MinDist = 999999.0;
				for( TObjectIterator<AActor> It; It; ++It )
				{
					FLOAT Dist = Player ? FDist(It->Location,Player->Location) : 0.0f;
					if
					(	(!Player || It->GetLevel()==Player->GetLevel())
					&&	(!It->bDeleteMe)
					&&	(It->IsA( Class) )
					&&	(Dist<MinDist) )
					{
						MinDist = Dist;
						Found   = *It;
					}
				}
			}
			else if( EngineIt && Parse( Cmd, TEXT("Name="), ActorName ) )
			{
				// look for actor by name
				for( TObjectIterator<AActor> It; It; ++It )
				{
					if( !It->bDeleteMe && It->GetName() == *ActorName )
					{
						Found = *It;
						break;
					}
				}
			}

			if( Found )
			{
				WObjectProperties* P = new WObjectProperties( TEXT("EditActor"), 0, TEXT(""), NULL, 1 );
				P->OpenWindow( (HWND)EngineIt->Client->Viewports(0)->GetWindow() );
				P->Root.SetObjects( (UObject**)&Found, 1 );
				P->Show(1);
			}
			else Ar.Logf( TEXT("Bad or missing class or name") );
#else
			UClass* Class;
			TObjectIterator<UEngine> EngineIt;
			if( EngineIt && ParseObject<UClass>( Cmd, TEXT("Class="), Class, ANY_PACKAGE ) )
			{
				AActor* Player  = EngineIt->Client ? EngineIt->Client->Viewports(0)->Actor : NULL;
				AActor* Found   = NULL;
				FLOAT   MinDist = 999999.0;
				for( TObjectIterator<AActor> It; It; ++It )
				{
					FLOAT Dist = Player ? FDist(It->Location,Player->Location) : 0.0;
					if
					(	(!Player || It->GetLevel()==Player->GetLevel())
					&&	(!It->bDeleteMe)
					&&	(It->IsA( Class) )
					&&	(Dist<MinDist) )
					{
						MinDist = Dist;
						Found   = *It;
					}
				}
				if( Found )
				{
					WObjectProperties* P = new WObjectProperties( TEXT("EditActor"), 0, TEXT(""), NULL, 1 );
					P->OpenWindow( (HWND)EngineIt->Client->Viewports(0)->GetWindow() );
					P->Root.SetObjects( (UObject**)&Found, 1 );
					P->Show(1);
				}
				else Ar.Logf( TEXT("Actor not found") );
			}
			else Ar.Logf( TEXT("Missing class") );
#endif
			return 1;
		}
		else if( ParseCommand(&Cmd,TEXT("HideLog")) )
		{
			if( GLogWindow )
				GLogWindow->Show(0);
			return 1;
		}
		else if( ParseCommand(&Cmd,TEXT("Preferences")) && !GIsClient )
		{
			if( !Preferences )
			{
				Preferences = new WConfigProperties( TEXT("Preferences"), LocalizeGeneral("AdvancedOptionsTitle",TEXT("Window")) );
				Preferences->SetNotifyHook( this );
				Preferences->OpenWindow( GLogWindow ? GLogWindow->hWnd : NULL );
				Preferences->ForceRefresh();
			}
			Preferences->Show(1);
			SetFocus( *Preferences );
			return 1;
		}
		else if( ParseCommand(&Cmd,TEXT("MPLAYER")) && GIsClient )
		{
			LaunchMplayer();			
			return 1;			
		}
		else if( ParseCommand(&Cmd,TEXT("HEAT")) && GIsClient )
		{
			appLaunchURL( TEXT("GotoHEAT.exe"), TEXT("5193") );
			return 1;			
		}
		else return 0;
		unguard;
	}
public:
	FExecHook()
	: Preferences( NULL )
	{}
};

/*-----------------------------------------------------------------------------
	Startup and shutdown.
-----------------------------------------------------------------------------*/


//
// Initialize.
//
#ifndef _EDITOR_
static UEngine* InitEngine()
{
	guard(InitEngine);
	FPushMemTag Push(TEXT("InitEngine"));
	FTime LoadTime[2] = { appSeconds(), appProcessSeconds() };

	// Set exec hook.
	static FExecHook GLocalHook;
	GExec = &GLocalHook;

	// Create mutex so installer knows we're running.
	CreateMutexX( NULL, 0, TEXT("UnrealIsRunning"));
#ifndef _DEBUG
//	UBOOL AlreadyRunning = (GetLastError()==ERROR_ALREADY_EXISTS);
#else
//	UBOOL AlreadyRunning = 1;
#endif

	// First-run menu.
	INT FirstRun=0;
	INT ForceSoftware=0;
	GConfig->GetInt( TEXT("FirstRun"), TEXT("FirstRun"), FirstRun );
	GConfig->GetInt( TEXT("FirstRun"), TEXT("Reconfig"), Reconfig );
	GConfig->GetInt( TEXT("FirstRun"), TEXT("ForceSoftware"), ForceSoftware );
//	if(Reconfig)
//		FirstRun=0;		//gk causes dialog right away

#if 0	// Obsolete code.
	if( FirstRun<220 )
	{
		// Migrate savegames.
		TArray<FString> Saves = GFileManager->FindFiles( *(FString(appUserDir()) * TEXT("*.usa")), 1, 0 );
		for( TArray<FString>::TIterator It(Saves); It; ++It )
		{
			INT Pos = appAtoi(**It+4);
			FString Section = TEXT("UnrealShare.UnrealSlotMenu");
			FString Key     = FString::Printf(TEXT("SlotNames[%i]"),Pos);
			if( appStricmp(GConfig->GetStr(*Section,*Key,TEXT("user")),TEXT(""))==0 )
				GConfig->SetString(*Section,*Key,TEXT("Saved game"),TEXT("user"));
		}
	}
#endif

	// Commandline (for mplayer/heat)
	FString Command;
	if( Parse(appCmdLine(),TEXT("consolecommand="), Command) )
	{
		debugf(TEXT("Executing console command %s"),*Command);
		GExec->Exec( *Command, *GLog );
		return NULL;
	}

	// Test render device.
	FString Device;
	if( Parse(appCmdLine(),TEXT("testrendev="),Device) )
	{
		debugf(TEXT("Detecting %s"),*Device);
		try
		{
			UClass* Cls = LoadClass<URenderDevice>( NULL, *Device, NULL, 0, NULL );
			GConfig->SetInt(*Device,TEXT("DescFlags"),RDDESCF_Incompatible);
			GConfig->Flush(0);
			if( Cls )
			{
				URenderDevice* RenDev = ConstructObject<URenderDevice>(Cls);
				if( RenDev )
				{
					if( RenDev->Init(NULL,0,0,0,0) )
					{
						debugf(TEXT("Successfully detected %s"),*Device);
					}
					else delete RenDev;
				}
			}
		} catch( ... ) {}

		FArchive* Ar = GFileManager->CreateFileWriter( *(FString(appUserDir()) * TEXT("Detected.ini")), 0 );
		if( Ar )
			delete Ar;
		return NULL;
	}

	// Config UI.
	guard(ConfigUI);
	if( (!GIsEditor )&& GIsClient )
	{
		if(!GDebugger)		//gk  3/10/02 if debugger then needs to keep running
		{
			// Create our FrontEnd
			WFrontEnd FE;
			WFrontEndPage* Page = NULL;
			
			FString cmdLine = appCmdLine();

			// See if a saveSlot is specified, if not then load the FrontEnd
			if( !ParseParam( appCmdLine(), TEXT("SAVESLOT=")  )  && 
				!ParseParam( appCmdLine(), TEXT("NOFRONTEND") ) )
			{ 
				Page = new WFrontEndPageMainMenu(&FE); 
				FE.Title=LocalizeGeneral(TEXT("MainMenu"),TEXT("Startup")); 
			}
			
			if( Page )
			{
				ExitSplash();
				FE.Advance( Page );
				if( !FE.DoModal() )
					return NULL;

				InitSplash(NULL);
			}

/* *** Commented out Old HP1 code ***

			WConfigWizard D;
			WWizardPage* Page = NULL;

			if( ParseParam(appCmdLine(),TEXT("safe")) || appStrfind(appCmdLine(),TEXT("readini")) )
	//		{Page = new WConfigPageSafeOptions(&D); D.Title=LocalizeGeneral(TEXT("SafeMode"),TEXT("Startup"));}
			{Page = new WConfigPageDetail(&D); D.Title=LocalizeGeneral(TEXT("SafeMode"),TEXT("Startup"));}
			
//			else if( FirstRun<ENGINE_VERSION )
//				{Page = new WConfigPageRenderer(&D); D.Title=LocalizeGeneral(TEXT("FirstTime"),TEXT("Startup"));}
//			else if( ParseParam(appCmdLine(),TEXT("changevideo")) )
//				{Page = new WConfigPageRenderer(&D); D.Title=LocalizeGeneral(TEXT("Video"),TEXT("Startup"));}
//			else if( !AlreadyRunning && GFileManager->FileSize(TEXT("Running.ini"))>=0 )
//				{Page = new WConfigPageSafeMode(&D); D.Title=LocalizeGeneral(TEXT("RecoveryMode"),TEXT("Startup"));}

			if( Page )
			{
				ExitSplash();
				D.Advance( Page );
				if( !D.DoModal() )
					return NULL;

				InitSplash(NULL);
			}
  *** End old hp1 code ***
*/
		}
	}

	unguard;
	
	if( !GIsEditor )
	{
		// Show HP splash screen during load time
		UBOOL   ShowLog  = ParseParam(appCmdLine(),TEXT("LOG"));
		FString Filename = FString(TEXT("..\\Help")) * TEXT("splashint.bmp");
		if( GFileManager->FileSize(*Filename)<0 )
			Filename = TEXT("..\\Help\\splashint.bmp");
		if( !ShowLog && !ParseParam(appCmdLine(),TEXT("server")) && !appStrfind(appCmdLine(),TEXT("TestRenDev")) )
			InitSplash( *Filename );
	}
	
	// Create is-running semaphore file.
	FArchive* Ar = GFileManager->CreateFileWriter( *(FString(appUserDir()) * TEXT("Running.ini")), 0 );
	if( Ar )
		delete Ar;

	// Update first-run.
	if( FirstRun<ENGINE_VERSION )
		FirstRun = ENGINE_VERSION;
	GConfig->SetInt( TEXT("FirstRun"), TEXT("FirstRun"), FirstRun );

	// Cd check.
	FString CdPath;
	GConfig->GetString( TEXT("Engine.Engine"), TEXT("CdPath"), CdPath );
	if
	(	CdPath!=TEXT("")
	&&	GFileManager->FileSize(TEXT("..\\Textures\\Palettes.utx"))<=0 )//oldver
	{
		FString Check = CdPath * TEXT("Textures\\Palettes.utx");
		while( !GIsEditor && GFileManager->FileSize(*Check)<=0 )
		{
			if( MessageBox
			(
				NULL,
				LocalizeGeneral("InsertCdText",TEXT("Window")),
				LocalizeGeneral("InsertCdTitle",TEXT("Window")),
				MB_TASKMODAL|MB_OKCANCEL
			)==IDCANCEL )
			{
				GIsCriticalError = 1;
				ExitProcess( 0 );
			}
		}
	}

#if ENGINE_VERSION<230
	// Display the damn story to appease the German censors.
	UBOOL CanModifyGore=1;
	GConfig->GetBool( TEXT("UnrealI.UnrealGameOptionsMenu"), TEXT("bCanModifyGore"), CanModifyGore );
	if( !CanModifyGore && !GIsEditor )
	{
		FString S;
		if( appLoadFileToString( S, TEXT("Story.txt") ) )
		{
			WTextScrollerDialog Dlg( TEXT("The Story"), *S );
			Dlg.DoModal();
		}
	}
#endif

	// Create the global engine object.
	UClass* EngineClass;
	if( !GIsEditor )
	{
		// Create game engine.
		EngineClass = UObject::StaticLoadClass( UGameEngine::StaticClass(), NULL, TEXT("ini:Engine.Engine.GameEngine"), NULL, LOAD_NoFail, NULL );
	}
	else
	{
		// Editor.
		EngineClass = UObject::StaticLoadClass( UEngine::StaticClass(), NULL, TEXT("ini:Engine.Engine.EditorEngine"), NULL, LOAD_NoFail, NULL );
	}
	UEngine* Engine = ConstructObject<UEngine>( EngineClass );
	Engine->Init();
	debugf( TEXT("Startup time: %f seconds total, %f app"), appSeconds()-LoadTime[0], appProcessSeconds()-LoadTime[1] );

	return Engine;
	unguard;
}

//
// Unreal's main message loop.  All windows in Unreal receive messages
// somewhere below this function on the stack.
//
static void MainLoop( UEngine* Engine )
{
	guard(MainLoop);
	check(Engine);

	// Enter main loop.
	guard(EnterMainLoop);
	if( GLogWindow )
		GLogWindow->SetExec( Engine );
	unguard;

	// Loop while running.
	GIsRunning = 1;
	DWORD ThreadId = GetCurrentThreadId();
	HANDLE hThread = GetCurrentThread();
	FTime OldTime = appSeconds();
	while( GIsRunning && !GIsRequestingExit )
	{
		// Update the world.
		guard(UpdateWorld);
		FTime NewTime   = appSeconds();
		FLOAT DeltaTime = NewTime - OldTime;
		Engine->CurrentTickRate = 1.f / DeltaTime;
		Engine->Tick( DeltaTime );
		if( GWindowManager )
			GWindowManager->Tick( DeltaTime );
		OldTime = NewTime;
		unguard;

		// Enforce optional maximum tick rate.
		guard(EnforceTickRate);
		FLOAT MaxTickRate = Engine->GetMaxTickRate();
		if( MaxTickRate>0.0 )
		{
			FLOAT Delta = (1.0f/MaxTickRate) - (appSeconds()-OldTime);
			appSleep( Max(0.f,Delta) );
		}
		unguard;

		// Handle all incoming messages.
		guard(MessagePump);
		MSG Msg;
		while( PeekMessageX(&Msg,NULL,0,0,PM_REMOVE) )
		{
			if( Msg.message == WM_QUIT )
				GIsRequestingExit = 1;

			guard(TranslateMessage);
			TranslateMessage( &Msg );
			unguardf(( TEXT("%08X %i"), (INT)Msg.hwnd, Msg.message ));

			guard(DispatchMessage);
			DispatchMessageX( &Msg );
			unguardf(( TEXT("%08X %i"), (INT)Msg.hwnd, Msg.message ));
		}
		unguard;

		// If editor thread doesn't have the focus, don't suck up too much CPU time.
		if( GIsEditor )
		{
			guard(ThrottleEditor);
			static UBOOL HadFocus=1;
			UBOOL HasFocus = (GetWindowThreadProcessId(GetForegroundWindow(),NULL) == ThreadId );
			if( HadFocus && !HasFocus )
			{
				// Drop our priority to speed up whatever is in the foreground.
				SetThreadPriority( hThread, THREAD_PRIORITY_BELOW_NORMAL );
			}
			else if( HasFocus && !HadFocus )
			{
				// Boost our priority back to normal.
				SetThreadPriority( hThread, THREAD_PRIORITY_NORMAL );
			}
			if( !HasFocus )
			{
				// Surrender the rest of this timeslice.
				Sleep(0);
			}
			HadFocus = HasFocus;
			unguard;
		}
	}
	GIsRunning = 0;

	// Exit main loop.
	guard(ExitMainLoop);
	if( GLogWindow )
		GLogWindow->SetExec( NULL );
	GExec = NULL;
	unguard;

	unguard;
}
#endif

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
