/*=============================================================================
	Main.cpp: UnrealEd Windows startup.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

    Revision history:
		* Created by Tim Sweeney.

    Work-in-progress todo's:

=============================================================================*/

#define  _ED_


enum eLASTDIR {
	eLASTDIR_UNR	= 0,
	eLASTDIR_UTX	= 1,
	eLASTDIR_PCX	= 2,
	eLASTDIR_UAX	= 3,
	eLASTDIR_WAV	= 4,
	eLASTDIR_BRUSH	= 5,
	eLASTDIR_2DS	= 6,
	eLASTDIR_MAX	= 7
};

enum eBROWSER {
	eBROWSER_MESH		= 0,
	eBROWSER_MUSIC		= 1,
	eBROWSER_SOUND		= 2,
	eBROWSER_ACTOR		= 3,
	eBROWSER_GROUP		= 4,
	eBROWSER_TEXTURE	= 5,
	eBROWSER_MAX		= 6,
};

#pragma warning( disable : 4201 )
#define STRICT
#define _WIN32_IE 0x0200
#include <windows.h>
#include <commctrl.h>
#include <commdlg.h>
#include <shlobj.h>

#include "Engine.h"
#include "UnRender.h"
#include "Window.h"
#include "..\..\Editor\Src\EditorPrivate.h"
#include "Res\resource.h"
#include "UnEngineWin.h"
#include "MRUList.h"
#include "DlgProgress.h"
#include "DlgRename.h"
#include "DlgSearchActors.h"
#include "Browser.h"
#include "BrowserMaster.h"
WBrowserMaster* GBrowserMaster = NULL;
#include "CodeFrame.h"
#include "DlgTexProp.h"
#include "DlgBrushBuilder.h"
#include "DlgAddSpecial.h"
#include "DlgScaleLights.h"
#include "DlgTexReplace.h"
#include "SurfPropSheet.h"
#include "MatineeSheet.h"
#include "BuildSheet.h"
#include "DlgBrushImport.h"
#include "DlgViewportConfig.h"
#include "DlgMapImport.h"
#include "TwoDeeShapeEditor.h"
#include "Extern.h"
#include "BrowserSound.h"
#include "BrowserMusic.h"
#include "BrowserGroup.h"
#include "BrowserTexture.h"
#include "BrowserMesh.h"
#include "..\..\core\inc\unmsg.h"

extern HWND GhwndBSPages[eBS_MAX];
// milestone 4
__declspec(dllimport) bool GPawnsLabel;
__declspec(dllimport) bool GTriggersLabel;
__declspec(dllimport) bool GLightsLabel;
__declspec(dllimport) bool GMoversLabel;
__declspec(dllimport) bool GSlabelLabel;


//milestone 4 addition gk
UClass*	FavClass[5]={0,0,0,0,0};
int		FavCount[5]={0,0,0,0,0};


// Just to keep track of the last viewport to get the focus.  The main editor
// app uses this to draw a white outline around the current viewport.
MRUList* GMRUList;
HWND GCurrentViewportFrame = NULL;
int GScrollBarWidth = GetSystemMetrics(SM_CXVSCROLL);
HWND GhwndEditorFrame = NULL;

enum EViewportStyle
{
	VSTYLE_Floating		= 0,
	VSTYLE_Fixed		= 1,
};

class WViewportFrame;
typedef struct {
	int RendMap;
	float PctLeft, PctTop, PctRight, PctBottom;	// Percentages of the parent window client size (VSTYLE_Fixed)
	float Left, Top, Right, Bottom;				// Literal window positions (VSTYLE_Floatin)
	WViewportFrame* m_pViewportFrame;
} VIEWPORTCONFIG;

// This is a list of all the viewport configs that are currently in effect.
TArray<VIEWPORTCONFIG> GViewports;

// Prefebbed viewport configs.  These should be in the same order as the buttons in DlgViewportConfig.
VIEWPORTCONFIG GTemplateViewportConfigs[4][4] =
{
	// 0
	REN_OrthXY,		0,		0,		.65f,		.50f,		0, 0, 0, 0,		NULL,
	REN_OrthXZ,		.65f,	0,		.35f,		.50f,		0, 0, 0, 0,		NULL,
	REN_DynLight,	0,		.50f,	.65f,		.50f,		0, 0, 0, 0,		NULL,
	REN_OrthYZ,		.65f,	.50f,	.35f,		.50f,		0, 0, 0, 0,		NULL,

	// 1
	REN_OrthXY,		0,		0,		.40f,		.40f,		0, 0, 0, 0,		NULL,
	REN_OrthXZ,		.40f,	0,		.30f,		.40f,		0, 0, 0, 0,		NULL,
	REN_OrthYZ,		.70f,	0,		.30f,		.40f,		0, 0, 0, 0,		NULL,
	REN_DynLight,	0,		.40f,	1.0f,		.60f,		0, 0, 0, 0,		NULL,

	// 2
	REN_DynLight,	0,		0,		.70f,		1.0f,		0, 0, 0, 0,		NULL,
	REN_OrthXY,		.70f,	0,		.30f,		.40f,		0, 0, 0, 0,		NULL,
	REN_OrthXZ,		.70f,	.40f,	.30f,		.30f,		0, 0, 0, 0,		NULL,
	REN_OrthYZ,		.70f,	.70f,	.30f,		.30f,		0, 0, 0, 0,		NULL,

	// 3
	REN_OrthXY,		0,		0,		1.0f,		.40f,		0, 0, 0, 0,		NULL,
	REN_DynLight,	0,		.40f,	1.0f,		.60f,		0, 0, 0, 0,		NULL,
	-1,	0, 0, 0, 0, 0, 0, 0, 0, NULL,
	-1,	0, 0, 0, 0, 0, 0, 0, 0, NULL,
};

int GViewportStyle, GViewportConfig;

#include "ViewportFrame.h"

FString GLastDir[eLASTDIR_MAX];
FString GMapExt;
HMENU GMainMenu = NULL;

extern "C" {HINSTANCE hInstance;}
extern "C" {TCHAR GPackage[64]=TEXT("UnrealEd");}

// Brushes.
HBRUSH hBrushMode = CreateSolidBrush( RGB(0,96,0) );

extern FString GLastText;
extern FString GMapExt;

// Forward declarations
void UpdateMenu();
// Classes.
class WMdiClient;
class WMdiFrame;
class WEditorFrame;
class WMdiDockingFrame;
class WLevelFrame;

// Memory allocator.
#include "FMallocWindows.h"
FMallocWindows Malloc;

// Log file.
#include "FOutputDeviceFile.h"
FOutputDeviceFile Log;

// Error handler.
#include "FOutputDeviceWindowsError.h"
FOutputDeviceWindowsError Error;

// Feedback.
#include "FFeedbackContextWindows.h"
FFeedbackContextWindows Warn;

// File manager.
#include "FFileManagerWindows.h"
FFileManagerWindows FileManager;

// Config.
#include "FConfigCacheIni.h"

WCodeFrame* GCodeFrame = NULL;
#include "BrowserActor.h"

WEditorFrame* GEditorFrame = NULL;
WLevelFrame* GLevelFrame = NULL;
W2DShapeEditor* G2DShapeEditor = NULL;
WMatineeSheet* GMatineeSheet = NULL;
TSurfPropSheet* GSurfPropSheet = NULL;
TBuildSheet* GBuildSheet = NULL;
WBrowserSound* GBrowserSound = NULL;
WBrowserMusic* GBrowserMusic = NULL;
WBrowserGroup* GBrowserGroup = NULL;
WBrowserActor* GBrowserActor = NULL;
WBrowserTexture* GBrowserTexture = NULL;
WBrowserMesh* GBrowserMesh = NULL;
WDlgAddSpecial* GDlgAddSpecial = NULL;
WDlgScaleLights* GDlgScaleLights = NULL;
WDlgProgress* GDlgProgress = NULL;
WDlgSearchActors* GDlgSearchActors = NULL;
WDlgTexReplace* GDlgTexReplace = NULL;

#include "ButtonBar.h"
#include "BottomBar.h"
#include "TopBar.h"
WButtonBar* GButtonBar;
WBottomBar* GBottomBar;
WTopBar* GTopBar;

void FileOpen( HWND hWnd );

void RefreshEditor()
{
	guard(RefreshEditor);
	GBrowserMaster->RefreshAll();
	GBuildSheet->RefreshStats();
	unguard;
}

/*-----------------------------------------------------------------------------
	Document manager crappy abstraction.
-----------------------------------------------------------------------------*/

struct FDocumentManager
{
	virtual void OpenLevelView()=0;
} *GDocumentManager=NULL;

/*-----------------------------------------------------------------------------
	WMdiClient.
-----------------------------------------------------------------------------*/

// An MDI client window.
class WMdiClient : public WControl
{
	DECLARE_WINDOWSUBCLASS(WMdiClient,WControl,UnrealEd)
	WMdiClient( WWindow* InOwner )
	: WControl( InOwner, 0, SuperProc )
	{}
	void OpenWindow( CLIENTCREATESTRUCT* ccs )
	{
		guard(WMdiFrame::OpenWindow);
		//must make nccreate work!! GetWindowClassName(),
		//!! WS_VSCROLL | WS_HSCROLL
        HWND hWndCreated = TCHAR_CALL_OS(CreateWindowEx(0,TEXT("MDICLIENT"),NULL,WS_CHILD|WS_CLIPCHILDREN | WS_CLIPSIBLINGS,0,0,0,0,OwnerWindow->hWnd,(HMENU)0xCAC,hInstance,ccs),CreateWindowExA(0,"MDICLIENT",NULL,WS_CHILD|WS_CLIPCHILDREN|WS_CLIPSIBLINGS,0,0,0,0,OwnerWindow->hWnd,(HMENU)0xCAC,hInstance,ccs));
		check(hWndCreated);
		check(!hWnd);
		_Windows.AddItem( this );
		hWnd = hWndCreated;
		Show( 1 );
		unguard;
	}
};
WNDPROC WMdiClient::SuperProc;

/*-----------------------------------------------------------------------------
	WDockingFrame.
-----------------------------------------------------------------------------*/

// One of four docking frame windows on a MDI frame.
class WDockingFrame : public WWindow
{
	DECLARE_WINDOWCLASS(WDockingFrame,WWindow,UnrealEd)

	// Variables.
	INT DockDepth;
	WWindow* Child;

	// Functions.
	WDockingFrame( FName InPersistentName, WMdiFrame* InFrame, INT InDockDepth )
	:	WWindow			( InPersistentName, (WWindow*)InFrame )
	,   DockDepth       ( InDockDepth )
	,	Child			( NULL )
	{}
	void OpenWindow()
	{
		guard(WDockingFrame::OpenWindow);
		PerformCreateWindowEx
		(
			0,
			NULL,
			WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
			0, 0, 0, 0,
			OwnerWindow->hWnd,
			NULL,
			hInstance
		);
		Show(1);
		unguard;
	}
	void Dock( WWindow* InChild )
	{
		guard(WDockingFrame::Dock);
		Child = InChild;
		unguard;
	}
	void OnSize( DWORD Flags, INT InX, INT InY )
	{
		guard(WDockingFrame::OnSize);
		if( Child )
			Child->MoveWindow( GetClientRect(), TRUE );
		unguard;
	}
	void OnPaint()
	{
		guard(WDockingFrame::OnPaint);
		PAINTSTRUCT PS;
		HDC hDC = BeginPaint( *this, &PS );
		HBRUSH brushBack = CreateSolidBrush( RGB(128,128,128) );

		FRect Rect = GetClientRect();
		FillRect( hDC, Rect, brushBack );
		MyDrawEdge( hDC, Rect, 1 );

		EndPaint( *this, &PS );

		DeleteObject( brushBack );
		unguard;
	}
};

/*-----------------------------------------------------------------------------
	WMdiFrame.
-----------------------------------------------------------------------------*/

// An MDI frame window.
class WMdiFrame : public WWindow
{
	DECLARE_WINDOWCLASS(WMdiFrame,WWindow,UnrealEd)

	// Variables.
	WMdiClient MdiClient;
	WDockingFrame LeftFrame, BottomFrame, TopFrame;

	// Functions.
	WMdiFrame( FName InPersistentName )
	:	WWindow		( InPersistentName )
	,	MdiClient	( this )
	,	BottomFrame	( TEXT("MdiFrameBottom"), this, 32 )
	,	LeftFrame	( TEXT("MdiFrameLeft"), this, 68 + GScrollBarWidth )
	,	TopFrame	( TEXT("MdiFrameTop"), this, 32 )
	{}
	INT CallDefaultProc( UINT Message, UINT wParam, LONG lParam )
	{
		return DefFrameProcX( hWnd, MdiClient.hWnd, Message, wParam, lParam );
	}
	void OnCreate()
	{
		guard(WMdiFrame::OnCreate);
		WWindow::OnCreate();

		// Create docking frames.
		BottomFrame.OpenWindow();
		LeftFrame.OpenWindow();
		TopFrame.OpenWindow();

		GSurfPropSheet = new TSurfPropSheet;
		GSurfPropSheet->OpenWindow( hInstance, hWnd );
		GSurfPropSheet->Show( FALSE );

		GBuildSheet = new TBuildSheet;
		GBuildSheet->OpenWindow( hInstance, hWnd );
		GBuildSheet->Show( FALSE );

		unguard;
	}
	virtual void RepositionClient()
	{
		guard(WMdiFrame::RepositionClient);

		// Reposition docking frames.
		FRect Client = GetClientRect();
		BottomFrame.MoveWindow( FRect(LeftFrame.DockDepth, Client.Max.Y-BottomFrame.DockDepth, Client.Max.X, Client.Max.Y), 1 );
		LeftFrame  .MoveWindow( FRect(0, TopFrame.DockDepth, LeftFrame.DockDepth, Client.Max.Y), 1 );
		TopFrame.MoveWindow( FRect(0, 0, Client.Max.X, TopFrame.DockDepth), 1 );

		// Reposition MDI client window.
		MdiClient.MoveWindow( FRect(LeftFrame.DockDepth, TopFrame.DockDepth, Client.Max.X, Client.Max.Y-BottomFrame.DockDepth), 1 );

		unguard;
	}
	void OnSize( DWORD Flags, INT NewX, INT NewY )
	{
		guard(WMdiFrame::OnSize);
		RepositionClient();
		throw TEXT("NoRoute");
		unguard;
	}
	void OpenWindow()
	{
		guard(WMdiFrame::OpenWindow);
		TCHAR Title[256];
		appSprintf( Title, LocalizeGeneral(TEXT("FrameWindow"),TEXT("UnrealEd")), LocalizeGeneral(TEXT("Product"),TEXT("Core")) );
		PerformCreateWindowEx
		(
			WS_EX_APPWINDOW,
			Title,
			WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_SIZEBOX | WS_MAXIMIZEBOX | WS_MINIMIZEBOX,
			CW_USEDEFAULT,
			CW_USEDEFAULT,
			640,
			480,
			NULL,
			NULL,
			hInstance
		);
		ShowWindow( *this, SW_SHOWMAXIMIZED );
		unguard;
	}
	void OnSetFocus()
	{
		guard(WMdiFrame::OnSetFocus);
		SetFocus( MdiClient );
		unguard;
	}
};

/*-----------------------------------------------------------------------------
	WBackgroundHolder.
-----------------------------------------------------------------------------*/

// Test.
class WBackgroundHolder : public WWindow
{
	DECLARE_WINDOWCLASS(WBackgroundHolder,WWindow,Window)

	// Structors.
	WBackgroundHolder( FName InPersistentName, WWindow* InOwnerWindow )
	:	WWindow( InPersistentName, InOwnerWindow )
	{}

	// WWindow interface.
	void OpenWindow()
	{
		guard(WBackgroundHolder::OpenWindow);
		MdiChild = 0;
		PerformCreateWindowEx
		(
			WS_EX_TOOLWINDOW | WS_EX_WINDOWEDGE,
			NULL,
			WS_CHILD | WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
			0,
			0,
			512,
			256,
			OwnerWindow ? OwnerWindow->hWnd : NULL,
			NULL,
			hInstance
		);
		unguard;
	}
};

/*-----------------------------------------------------------------------------
	WLevelFrame.
-----------------------------------------------------------------------------*/

enum eBIMODE {
	eBIMODE_CENTER	= 0,
	eBIMODE_TILE	= 1,
	eBIMODE_STRETCH	= 2
};

class WLevelFrame : public WWindow
{
	DECLARE_WINDOWCLASS(WLevelFrame,WWindow,Window)

	// Variables.
	ULevel* Level;
	HBITMAP hImage;
	FString BIFilename;
	int BIMode;	// eBIMODE_

	// Structors.
	WLevelFrame( ULevel* InLevel, FName InPersistentName, WWindow* InOwnerWindow )
	:	WWindow( InPersistentName, InOwnerWindow )
	,	Level( InLevel )
	{
		SetMapFilename( TEXT("") );
		hImage = NULL;
		BIMode = eBIMODE_CENTER;
		BIFilename = TEXT("");

		for( int x = 0 ; x < GViewports.Num() ; x++)
			GViewports(x).m_pViewportFrame = NULL;
		GViewports.Empty();
	}
	void SetMapFilename( TCHAR* _MapFilename )
	{
		appStrcpy( MapFilename, _MapFilename );
		if( ::IsWindow( hWnd ) )
			SetText( MapFilename );
	}
	TCHAR* GetMapFilename()
	{
		return MapFilename;
	}

	void OnDestroy()
	{
		guard(WLevelFrame::OnDestroy);

		ChangeViewportStyle();

		for( int group = 0 ; group < GButtonBar->Groups.Num() ; group++ )
			GConfig->SetInt( TEXT("Groups"), *GButtonBar->Groups(group).GroupName, GButtonBar->Groups(group).iState, TEXT("UnrealEd.ini") );

		// Save data out to config file, and clean up...
		GConfig->SetInt( TEXT("Viewports"), TEXT("Style"), GViewportStyle, TEXT("UnrealEd.ini") );
		GConfig->SetInt( TEXT("Viewports"), TEXT("Config"), GViewportConfig, TEXT("UnrealEd.ini") );

		for( int x = 0 ; x < GViewports.Num() ; x++)
		{
			TCHAR l_chName[20];
			appSprintf( l_chName, TEXT("U2Viewport%d"), x);

			if( GViewports(x).m_pViewportFrame 
					&& ::IsWindow( GViewports(x).m_pViewportFrame->hWnd ) 
					&& !::IsIconic( GViewports(x).m_pViewportFrame->hWnd )
					&& !::IsZoomed( GViewports(x).m_pViewportFrame->hWnd ))
			{
				FRect R = GViewports(x).m_pViewportFrame->GetWindowRect();
			
				GConfig->SetInt( l_chName, TEXT("Active"), 1, TEXT("UnrealEd.ini") );
				GConfig->SetInt( l_chName, TEXT("RendMap"), GViewports(x).m_pViewportFrame->m_pViewport->Actor->RendMap, TEXT("UnrealEd.ini") );

				GConfig->SetFloat( l_chName, TEXT("PctLeft"), GViewports(x).PctLeft, TEXT("UnrealEd.ini") );
				GConfig->SetFloat( l_chName, TEXT("PctTop"), GViewports(x).PctTop, TEXT("UnrealEd.ini") );
				GConfig->SetFloat( l_chName, TEXT("PctRight"), GViewports(x).PctRight, TEXT("UnrealEd.ini") );
				GConfig->SetFloat( l_chName, TEXT("PctBottom"), GViewports(x).PctBottom, TEXT("UnrealEd.ini") );

				GConfig->SetFloat( l_chName, TEXT("Left"), GViewports(x).Left, TEXT("UnrealEd.ini") );
				GConfig->SetFloat( l_chName, TEXT("Top"), GViewports(x).Top, TEXT("UnrealEd.ini") );
				GConfig->SetFloat( l_chName, TEXT("Right"), GViewports(x).Right, TEXT("UnrealEd.ini") );
				GConfig->SetFloat( l_chName, TEXT("Bottom"), GViewports(x).Bottom, TEXT("UnrealEd.ini") );

				FString Device = GViewports(x).m_pViewportFrame->m_pViewport->RenDev->GetClass()->GetFullName();
				Device = Device.Right( Device.Len() - Device.InStr( TEXT(" "), 0 ) - 1 );
				GConfig->SetString( l_chName, TEXT("Device"), *Device, TEXT("UnrealEd.ini") );
			}
			else {

				GConfig->SetInt( l_chName, TEXT("Active"), 0, TEXT("UnrealEd.ini") );
			}

			delete GViewports(x).m_pViewportFrame;
		}

		// "Last Directory"
		GConfig->SetString( TEXT("Directories"), TEXT("PCX"), *GLastDir[eLASTDIR_PCX], TEXT("UnrealEd.ini") );
		GConfig->SetString( TEXT("Directories"), TEXT("WAV"), *GLastDir[eLASTDIR_WAV], TEXT("UnrealEd.ini") );
		GConfig->SetString( TEXT("Directories"), TEXT("BRUSH"), *GLastDir[eLASTDIR_BRUSH], TEXT("UnrealEd.ini") );
		GConfig->SetString( TEXT("Directories"), TEXT("2DS"), *GLastDir[eLASTDIR_2DS], TEXT("UnrealEd.ini") );

		// Background image
		GConfig->SetInt( TEXT("Background Image"), TEXT("Active"), (hImage != NULL), TEXT("UnrealEd.ini") );
		GConfig->SetInt( TEXT("Background Image"), TEXT("Mode"), BIMode, TEXT("UnrealEd.ini") );
		GConfig->SetString( TEXT("Background Image"), TEXT("Filename"), *BIFilename, TEXT("UnrealEd.ini") );

		::DeleteObject( hImage );

		unguard;
	}
	// Looks for an empty viewport slot, allocates a viewport and returns a pointer to it.
	WViewportFrame* NewViewportFrame( FName* pName, UBOOL bNoSize )
	{
		guard(WLevelFrame::NewViewportFrame);

		// Clean up dead windows first.
		for( int x = 0 ; x < GViewports.Num() ; x++)
			if( GViewports(x).m_pViewportFrame && !::IsWindow( GViewports(x).m_pViewportFrame->hWnd ) )
				GViewports.Remove(x);

		if( GViewports.Num() > dED_MAX_VIEWPORTS )
		{
			appMsgf( TEXT("You are at the limit for open viewports.") );
			return NULL;
		}

		// Make up a unique name for this viewport.
		TCHAR l_chName[20];
		for( x = 0 ; x < dED_MAX_VIEWPORTS ; x++)
		{
			appSprintf( l_chName, TEXT("U2Viewport%d"), x);

			// See if this name is already taken
			BOOL bIsUnused = 1;
			for( int y = 0 ; y < GViewports.Num() ; y++)
				if( !appStricmp(GViewports(y).m_pViewportFrame->m_pViewport->GetName(),l_chName) )
				{
					bIsUnused = 0;
					break;
				}

			if( bIsUnused )
				break;
		}

		*pName = l_chName;

		// Create the viewport.
		new(GViewports)VIEWPORTCONFIG();
		INT Index = GViewports.Num() - 1;
		GViewports(Index).PctLeft = 0;
		GViewports(Index).PctTop = 0;
		GViewports(Index).PctRight = bNoSize ? 0 : 50;
		GViewports(Index).PctBottom = bNoSize ? 0 : 50;
		GViewports(Index).Left = 0;
		GViewports(Index).Top = 0;
		GViewports(Index).Right = bNoSize ? 0 : 320;
		GViewports(Index).Bottom = bNoSize ? 0 : 200;
		GViewports(Index).m_pViewportFrame = new WViewportFrame( *pName, this );
		GViewports(Index).m_pViewportFrame->m_iIdx = Index;

		GCurrentViewport = (DWORD)(GViewports(Index).m_pViewportFrame);

		return GViewports(Index).m_pViewportFrame;

		unguard;
	}
	// Causes all viewports to redraw themselves.  This is necessary so we can reliably switch
	// which window has the white focus outline.
	void RedrawAllViewports()
	{
		guard(WLevelFrame::RedrawAllViewports);
		for( int x = 0 ; x < GViewports.Num() ; x++)
			InvalidateRect( GViewports(x).m_pViewportFrame->hWnd, NULL, 1 );
		unguard;
	}
	// Changes the visual style of all open viewports to whatever the current style is.  This is also good
	// for forcing all viewports to recompute their positional data.
	void ChangeViewportStyle()
	{
		guard(WLevelFrame::ChangeViewportStyle);

		for( int x = 0 ; x < GViewports.Num() ; x++)
		{
			if( GViewports(x).m_pViewportFrame && ::IsWindow( GViewports(x).m_pViewportFrame->hWnd ) )
			{
				switch( GViewportStyle )
				{
					case VSTYLE_Floating:
						SetWindowLongA( GViewports(x).m_pViewportFrame->hWnd, GWL_STYLE, WS_OVERLAPPEDWINDOW | WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS );
						break;
					case VSTYLE_Fixed:
						SetWindowLongA( GViewports(x).m_pViewportFrame->hWnd, GWL_STYLE, WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS );
						break;
				}

				GViewports(x).m_pViewportFrame->ComputePositionData();
				SetWindowPos( GViewports(x).m_pViewportFrame->hWnd, HWND_TOP, 0, 0, 0, 0, SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE );

				GViewports(x).m_pViewportFrame->AdjustToolbarButtons();
			}
		}

		unguard;
	}
	// Resizes all existing viewports to fit properly on the screen.
	void FitViewportsToWindow()
	{
		guard(WLevelFrame::FitViewportsToWindow);

		RECT R;
		::GetClientRect( GLevelFrame->hWnd, &R );

		for( int x = 0 ; x < GViewports.Num() ; x++)
		{
			VIEWPORTCONFIG* pVC = &(GViewports(GViewports(x).m_pViewportFrame->m_iIdx));
			if( GViewportStyle == VSTYLE_Floating )
				::MoveWindow(GViewports(x).m_pViewportFrame->hWnd,
					pVC->Left, pVC->Top, pVC->Right, pVC->Bottom, 1);
			else
				::MoveWindow(GViewports(x).m_pViewportFrame->hWnd,
					pVC->PctLeft * R.right, pVC->PctTop * R.bottom,
					pVC->PctRight * R.right, pVC->PctBottom * R.bottom, 1);
		}
		unguard;
	}
	void CreateNewViewports( int _Style, int _Config )
	{
		guard(WLevelFrame::CreateNewViewports);

		GViewportStyle = _Style;
		GViewportConfig = _Config;

		// Get rid of any existing viewports.
		for( int x = 0 ; x < GViewports.Num() ; x++)
		{
			delete GViewports(x).m_pViewportFrame;
			GViewports(x).m_pViewportFrame = NULL;
		}
		GViewports.Empty();

		// Create new viewports
		switch( GViewportConfig )
		{
			case 0:		// classic
			{
				GLevelFrame->OpenFrameViewport( REN_OrthXY,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_OrthXZ,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_DynLight,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_OrthYZ,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
			}
			break;

			case 1:		// big one on buttom, small ones along top
			{
				GLevelFrame->OpenFrameViewport( REN_OrthXY,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_OrthXZ,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_OrthYZ,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_DynLight,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
			}
			break;

			case 2:		// big one on left side, small along right
			{
				GLevelFrame->OpenFrameViewport( REN_DynLight,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_OrthXY,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_OrthXZ,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_OrthYZ,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
			}
			break;

			case 3:		// 2 large windows, split horizontally
			{
				GLevelFrame->OpenFrameViewport( REN_OrthXY,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				GLevelFrame->OpenFrameViewport( REN_DynLight,0,0,10,10,SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
			}
			break;
		}

		// Load initial data from templates
		for( x = 0 ; x < GViewports.Num() ; x++ )
			if( GTemplateViewportConfigs[0][x].PctLeft != -1 )
			{
				GViewports(x).PctLeft = GTemplateViewportConfigs[GViewportConfig][x].PctLeft;
				GViewports(x).PctTop = GTemplateViewportConfigs[GViewportConfig][x].PctTop;
				GViewports(x).PctRight = GTemplateViewportConfigs[GViewportConfig][x].PctRight;
				GViewports(x).PctBottom = GTemplateViewportConfigs[GViewportConfig][x].PctBottom;
			}

		// Set the viewports to their proper sizes.
		int SaveViewportStyle = VSTYLE_Fixed;
		Exchange( GViewportStyle, SaveViewportStyle );
		FitViewportsToWindow();
		Exchange( SaveViewportStyle, GViewportStyle );
		ChangeViewportStyle();

		unguard;
	}
	// WWindow interface.
	void OnKillFocus( HWND hWndNew )
	{
		guard(WLevelFrame::OnKillFocus);
		GEditor->Client->MakeCurrent( NULL );
		unguard;
	}
	void Serialize( FArchive& Ar )
	{
		guard(WLevelFrame::Serialize);
		WWindow::Serialize( Ar );
		Ar << Level;
		unguard;
	}
	void OpenWindow( UBOOL bMdi, UBOOL bMax )
	{
		guard(WLevelFrame::OpenWindow);
		MdiChild = bMdi;
		PerformCreateWindowEx
		(
			MdiChild
			?	(WS_EX_MDICHILD)
			:	(0),
			TEXT("Level"),
			(bMax ? WS_MAXIMIZE : 0 ) |
			(MdiChild
			?	(WS_CHILD | WS_CLIPSIBLINGS | WS_CLIPCHILDREN | WS_SYSMENU | WS_CAPTION | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX)
			:	(WS_CHILD | WS_CLIPCHILDREN | WS_CLIPSIBLINGS)),
			CW_USEDEFAULT,
			CW_USEDEFAULT,
			512,
			384,
			MdiChild ? OwnerWindow->OwnerWindow->hWnd : OwnerWindow->hWnd,
			NULL,
			hInstance
		);
		if( !MdiChild )
		{
			SetWindowLongX( hWnd, GWL_STYLE, WS_POPUP | WS_CLIPCHILDREN | WS_CLIPSIBLINGS );
			OwnerWindow->Show(1);
		}

		// Open the proper configuration of viewports.
		if(!GConfig->GetInt( TEXT("Viewports"), TEXT("Style"), GViewportStyle, TEXT("UnrealEd.ini") ))		GViewportStyle = VSTYLE_Fixed;
		if(!GConfig->GetInt( TEXT("Viewports"), TEXT("Config"), GViewportConfig, TEXT("UnrealEd.ini") ))	GViewportConfig = 0;

		for( int x = 0 ; x < dED_MAX_VIEWPORTS ; x++)
		{
			TCHAR l_chName[20];
			appSprintf( l_chName, TEXT("U2Viewport%d"), x);
			int Active, RendMap;

			if(!GConfig->GetInt( l_chName, TEXT("Active"), Active, TEXT("UnrealEd.ini") ))		Active = 0;

			if( Active )
			{
				if(!GConfig->GetInt( l_chName, TEXT("RendMap"), RendMap, TEXT("UnrealEd.ini") ))	RendMap = REN_OrthXY;

				OpenFrameViewport( RendMap, 0, 0, 10, 10, SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
				VIEWPORTCONFIG* pVC = &(GViewports.Last());

				if(!GConfig->GetFloat( l_chName, TEXT("PctLeft"), pVC->PctLeft, TEXT("UnrealEd.ini") ))	pVC->PctLeft = 0;
				if(!GConfig->GetFloat( l_chName, TEXT("PctTop"), pVC->PctTop, TEXT("UnrealEd.ini") ))	pVC->PctTop = 0;
				if(!GConfig->GetFloat( l_chName, TEXT("PctRight"), pVC->PctRight, TEXT("UnrealEd.ini") ))	pVC->PctRight = .5f;
				if(!GConfig->GetFloat( l_chName, TEXT("PctBottom"), pVC->PctBottom, TEXT("UnrealEd.ini") ))	pVC->PctBottom = .5f;

				if(!GConfig->GetFloat( l_chName, TEXT("Left"), pVC->Left, TEXT("UnrealEd.ini") ))	pVC->Left = 0;
				if(!GConfig->GetFloat( l_chName, TEXT("Top"), pVC->Top, TEXT("UnrealEd.ini") ))	pVC->Top = 0;
				if(!GConfig->GetFloat( l_chName, TEXT("Right"), pVC->Right, TEXT("UnrealEd.ini") ))	pVC->Right = 320;
				if(!GConfig->GetFloat( l_chName, TEXT("Bottom"), pVC->Bottom, TEXT("UnrealEd.ini") ))	pVC->Bottom = 200;

				FString Device;
				int SizeX, SizeY;
				SizeX = pVC->m_pViewportFrame->m_pViewport->SizeX;
				SizeY = pVC->m_pViewportFrame->m_pViewport->SizeY;

				GConfig->GetString( l_chName, TEXT("Device"), Device, TEXT("UnrealEd.ini") );
				if( !Device.Len() )		Device = TEXT("SoftDrv.SoftwareRenderDevice");

				pVC->m_pViewportFrame->m_pViewport->TryRenderDevice( *Device, SizeX, SizeY, INDEX_NONE, 0 );
				if( !pVC->m_pViewportFrame->m_pViewport->RenDev )
					pVC->m_pViewportFrame->m_pViewport->TryRenderDevice( TEXT("SoftDrv.SoftwareRenderDevice"), SizeX, SizeY, INDEX_NONE, 0 );
			}
		}

		FitViewportsToWindow();

		// Background image
		UBOOL bActive;
		if(!GConfig->GetInt( TEXT("Background Image"), TEXT("Active"), bActive, TEXT("UnrealEd.ini") ))	bActive = 0;

		if( bActive )
		{
			if(!GConfig->GetInt( TEXT("Background Image"), TEXT("Mode"), BIMode, TEXT("UnrealEd.ini") ))	BIMode = eBIMODE_CENTER;
			if(!GConfig->GetString( TEXT("Background Image"), TEXT("Filename"), BIFilename, TEXT("UnrealEd.ini") ))	BIFilename.Empty();
			LoadBackgroundImage(BIFilename);
		}

		unguard;
	}
	void LoadBackgroundImage( FString Filename )
	{
		guard(WLevelFrame::LoadBackgroundImage);

		if( hImage ) 
			DeleteObject( hImage );

		hImage = (HBITMAP)LoadImageA( hInstance, appToAnsi( *Filename ), IMAGE_BITMAP, 0, 0, LR_LOADFROMFILE );

		if( hImage )
			BIFilename = Filename;
		else
			appMsgf ( TEXT("Error loading bitmap for background image.") );

		unguard;
	}
	void OnSize( DWORD Flags, INT NewX, INT NewY )
	{
		guard(WLevelFrame::OnSize);
		WWindow::OnSize( Flags, NewX, NewY );

		FitViewportsToWindow();

		unguard;
	}
	INT OnSetCursor()
	{
		guard(WLevelFrame::OnSetCursor);
		WWindow::OnSetCursor();
		SetCursor(LoadCursorIdX(NULL,IDC_ARROW));
		return 0;
		unguard;
	}
	void OnPaint()
	{
		guard(WLevelFrame::OnPaint);
		PAINTSTRUCT PS;
		HDC hDC = BeginPaint( *this, &PS );
		FillRect( hDC, GetClientRect(), (HBRUSH)(COLOR_WINDOW+1) );
		DrawImage( hDC );
		EndPaint( *this, &PS );

		// Put the name of the map into the titlebar.
		SetText( GetMapFilename() );

		unguard;
	}
	void DrawImage( HDC _hdc )
	{
		guard(WLevelFrame::DrawImage);
		if( !hImage ) return;

		HDC hdcMem;
		HBITMAP hbmOld;
		BITMAP bitmap;

		// Prepare the bitmap.
		//
		GetObjectA( hImage, sizeof(BITMAP), (LPSTR)&bitmap );
		hdcMem = CreateCompatibleDC(_hdc);
		hbmOld = (HBITMAP)SelectObject(hdcMem, hImage);

		// Display it.
		//
		RECT l_rc;
		::GetClientRect( hWnd, &l_rc );
		switch( BIMode )
		{
			case eBIMODE_CENTER:
			{
				BitBlt(_hdc,
				   (l_rc.right - bitmap.bmWidth) / 2, (l_rc.bottom - bitmap.bmHeight) / 2,
				   bitmap.bmWidth, bitmap.bmHeight,
				   hdcMem,
				   0, 0,
				   SRCCOPY);
			}
			break;

			case eBIMODE_TILE:
			{
				int XSteps = (int)(l_rc.right / bitmap.bmWidth) + 1;
				int YSteps = (int)(l_rc.bottom / bitmap.bmHeight) + 1;

				for( int x = 0 ; x < XSteps ; x++ )
					for( int y = 0 ; y < YSteps ; y++ )
						BitBlt(_hdc,
						   (x * bitmap.bmWidth), (y * bitmap.bmHeight),
						   bitmap.bmWidth, bitmap.bmHeight,
						   hdcMem,
						   0, 0,
						   SRCCOPY);
			}
			break;

			case eBIMODE_STRETCH:
			{
				StretchBlt(
					_hdc,
				   0, 0,
				   l_rc.right, l_rc.bottom,
				   hdcMem,
				   0, 0,
				   bitmap.bmWidth, bitmap.bmHeight,
				   SRCCOPY);
			}
			break;
		}

		// Clean up.
		//
		SelectObject(hdcMem, hbmOld);
		DeleteDC(hdcMem);
		unguard;
	}

	// Opens a new viewport window.  It creates a viewportframe of the specified size, then creates
	// a viewport that fits inside of it.
	virtual void OpenFrameViewport( INT RendMap, int X, int Y, int W, int H, DWORD ShowFlags )
	{
		guard(WLevelFrame::OpenFrameViewport);

		FName Name = TEXT("");

		// Open a viewport frame.
		WViewportFrame* pViewportFrame = NewViewportFrame( &Name, 1 );

		if( pViewportFrame ) 
		{
			pViewportFrame->OpenWindow();

			// Create the viewport inside of the frame.
			UViewport* Viewport = GEditor->Client->NewViewport( Name );
			Level->SpawnViewActor( Viewport );
			Viewport->Actor->ShowFlags = ShowFlags;
			Viewport->Actor->RendMap   = RendMap;
			Viewport->Input->Init( Viewport );
			pViewportFrame->SetViewport( Viewport );
			::MoveWindow( (HWND)pViewportFrame->hWnd, X, Y, W, H, 1 );
			::BringWindowToTop( pViewportFrame->hWnd );
			pViewportFrame->ComputePositionData();
		}

		unguard;
	}
private:

	TCHAR MapFilename[256];
};



/*-----------------------------------------------------------------------------
	WNewObject.
-----------------------------------------------------------------------------*/

// New object window.
class WNewObject : public WDialog
{
	DECLARE_WINDOWCLASS(WNewObject,WDialog,UnrealEd)

	// Variables.
	WButton OkButton;
	WButton CancelButton;
	WListBox TypeList;
	WObjectProperties Props;
	UObject* Context;
	UObject* Result;
 
	// Constructor.
	WNewObject( UObject* InContext, WWindow* InOwnerWindow )
	:	WDialog		( TEXT("NewObject"), IDDIALOG_NewObject, InOwnerWindow )
	,	OkButton    ( this, IDOK,     FDelegate(this,(TDelegate)OnOk) )
	,	CancelButton( this, IDCANCEL, FDelegate(this,(TDelegate)EndDialogFalse) )
	,	TypeList	( this, IDC_TypeList )
	,	Props		( NAME_None, CPF_Edit, TEXT(""), this, 0 )
	,	Context     ( InContext )
	,	Result		( NULL )
	{
		Props.ShowTreeLines = 0;
		TypeList.DoubleClickDelegate=FDelegate(this,(TDelegate)OnOk);
	}

	// WDialog interface.
	void OnInitDialog()
	{
		guard(WNewObject::OnInitDialog);
		WDialog::OnInitDialog();
		for( TObjectIterator<UClass> It; It; ++It )
		{
			if( It->IsChildOf(UFactory::StaticClass()) )
			{
				UFactory* Default = (UFactory*)It->GetDefaultObject();
				if( Default->bCreateNew )
					TypeList.SetItemData( TypeList.AddString( *Default->Description ), *It );
			}
		}
		Props.OpenChildWindow( IDC_PropHolder );
		TypeList.SetCurrent( 0, 1 );
		TypeList.SelectionChangeDelegate = FDelegate(this,(TDelegate)OnSelChange);
		OnSelChange();
		unguard;
	}
	void OnDestroy()
	{
		guard(WNewObject::OnDestroy);
		WDialog::OnDestroy();
		unguard;
	}
	virtual UObject* DoModal()
	{
		guard(WNewObject::DoModal);
		WDialog::DoModal( hInstance );
		return Result;
		unguard;
	}

	// Notifications.
	void OnSelChange()
	{
		guard(WNewObject::OnSelChange);
		INT Index = TypeList.GetCurrent();
		if( Index>=0 )
		{
			UClass*   Class   = (UClass*)TypeList.GetItemData(Index);
			UObject*  Factory = ConstructObject<UFactory>( Class );
			Props.Root.SetObjects( &Factory, 1 );
			EnableWindow( OkButton, 1 );
		}
		else
		{
			Props.Root.SetObjects( NULL, 0 );
			EnableWindow( OkButton, 0 );
		}
		unguard;
	}
	void OnOk()
	{
		guard(WNewObject::OnOk);
		if( Props.Root._Objects.Num() )
		{
			UFactory* Factory = CastChecked<UFactory>(Props.Root._Objects(0));
			Result = Factory->FactoryCreateNew( Factory->SupportedClass, NULL, NAME_None, 0, Context, GWarn );
			if( Result )
				EndDialogTrue();
		}
		unguard;
	}

	// WWindow interface.
	void Serialize( FArchive& Ar )
	{
		guard(WNewObject::Serialize);
		WDialog::Serialize( Ar );
		Ar << Context;
		for( INT i=0; i<TypeList.GetCount(); i++ )
		{
			UObject* Obj = (UClass*)TypeList.GetItemData(i);
			Ar << Obj;
		}
		unguard;
	}
};

void FileSaveAs( HWND hWnd )
{
	// Make sure we have a level loaded...
	if( !GLevelFrame ) { return; }

	OPENFILENAMEA ofn;
	char File[8192], *pFilename;
	TCHAR l_chCmd[255];

	pFilename = TCHAR_TO_ANSI( GLevelFrame->GetMapFilename() );
	strcpy( File, pFilename );

	ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
	ofn.lStructSize = sizeof(OPENFILENAMEA);
	ofn.hwndOwner = hWnd;
	ofn.lpstrFile = File;
	ofn.nMaxFile = sizeof(char) * 8192;
	char Filter[255];
	::sprintf( Filter,
		"Map Files (*.%s)%c*.%s%cAll Files%c*.*%c%c",
		appToAnsi( *GMapExt ),
		'\0',
		appToAnsi( *GMapExt ),
		'\0',
		'\0',
		'\0',
		'\0' );
	ofn.lpstrFilter = Filter;
	ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_UNR]) );
	ofn.lpstrDefExt = appToAnsi( *GMapExt );
	ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT;

	// Display the Open dialog box. 
	if( GetSaveFileNameA(&ofn) )
	{
		// Convert the ANSI filename to UNICODE, and tell the editor to open it.
		GEditor->Exec( TEXT("BRUSHCLIP DELETE") );
		appSprintf( l_chCmd, TEXT("MAP SAVE FILE=\"%s\""), ANSI_TO_TCHAR(File));
		GEditor->Exec( l_chCmd );

		// Save the filename.
		GLevelFrame->SetMapFilename( ANSI_TO_TCHAR(File) );
		GMRUList->AddItem( GLevelFrame->GetMapFilename() );
		GMRUList->AddToMenu( hWnd, GMainMenu, 1 );

		FString S = ANSI_TO_TCHAR(File);
		GLastDir[eLASTDIR_UNR] = S.Left( S.InStr( TEXT("\\"), 1 ) );
	}

	GFileManager->SetDefaultDirectory(appBaseDir());
}

void FileSave( HWND hWnd )
{
	if( GLevelFrame ) {

		if( ::appStrlen( GLevelFrame->GetMapFilename() ) )
		{
			GEditor->Exec( TEXT("BRUSHCLIP DELETE") );
			GEditor->Exec( *(FString::Printf(TEXT("MAP SAVE FILE=\"%s\""), GLevelFrame->GetMapFilename())) );

			GMRUList->AddItem( GLevelFrame->GetMapFilename() );
			GMRUList->AddToMenu( hWnd, GMainMenu, 1 );
		}
		else
			FileSaveAs( hWnd );
	}
}

void FileSaveChanges( HWND hWnd )
{
	// If a level has been loaded and there is something in the undo buffer, ask the user
	// if they want to save.
	if( GLevelFrame 
			&& GEditor->Trans->CanUndo() )
	{
		TCHAR l_chMsg[256];

		appSprintf( l_chMsg, TEXT("Save changes to %s?"), GLevelFrame->GetMapFilename() );

		if( ::MessageBox( hWnd, l_chMsg, TEXT("UnrealEd"), MB_YESNO) == IDYES )
			FileSave( hWnd );
	}
}

enum eGI {
	eGI_NUM_SELECTED		= 1,
	eGI_CLASSNAME_SELECTED	= 2,
	eGI_NUM_SURF_SELECTED	= 4,
	eGI_CLASS_SELECTED		= 8
};

typedef struct tag_GetInfoRet {
	int iValue;
	FString String;
	UClass*	pClass;
} t_GetInfoRet;

t_GetInfoRet GetInfo( ULevel* Level, int Item )
{
	guard(GetInfo);

	t_GetInfoRet Ret;

	Ret.iValue = 0;
	Ret.String = TEXT("");

	// ACTORS
	if( Item & eGI_NUM_SELECTED
			|| Item & eGI_CLASSNAME_SELECTED 
			|| Item & eGI_CLASS_SELECTED )
	{
		int NumActors = 0;
		BOOL bAnyClass = FALSE;
		UClass*	AllClass = NULL;

		for( int i=0; i<Level->Actors.Num(); i++ )
		{
			if( Level->Actors(i) && Level->Actors(i)->bSelected )
			{
				if( bAnyClass && Level->Actors(i)->GetClass() != AllClass ) 
					AllClass = NULL;
				else 
					AllClass = Level->Actors(i)->GetClass();

				bAnyClass = TRUE;
				NumActors++;
			}
		}

		if( Item & eGI_NUM_SELECTED )
		{
			Ret.iValue = NumActors;
		}
		if( Item & eGI_CLASSNAME_SELECTED )
		{
			if( bAnyClass && AllClass )
				Ret.String = AllClass->GetName();
			else 
				Ret.String = TEXT("Actor");
		}
		if( Item & eGI_CLASS_SELECTED )
		{
			if( bAnyClass && AllClass )
				Ret.pClass = AllClass;
			else 
				Ret.pClass = NULL;
		}
	}

	// SURFACES
	if( Item & eGI_NUM_SURF_SELECTED)
	{
		int NumSurfs = 0;

		for( INT i=0; i<Level->Model->Surfs.Num(); i++ )
		{
			FBspSurf *Poly = &Level->Model->Surfs(i);

			if( Poly->PolyFlags & PF_Selected )
			{
				NumSurfs++;
			}
		}

		if( Item & eGI_NUM_SURF_SELECTED )
		{
			Ret.iValue = NumSurfs;
		}
	}

	return Ret;

	unguard;
}

void ShowCodeFrame( WWindow* Parent )
{
	if( GCodeFrame
			&& ::IsWindow( GCodeFrame->hWnd ) )
	{
		GCodeFrame->Show(1);
		::BringWindowToTop( GCodeFrame->hWnd );
	}
}


/*-----------------------------------------------------------------------------
	WEditorFrame.
-----------------------------------------------------------------------------*/

// Editor frame window.
class WEditorFrame : public WMdiFrame, public FNotifyHook, public FDocumentManager
{
	DECLARE_WINDOWCLASS(WEditorFrame,WMdiFrame,UnrealEd)

	// Variables.
	WBackgroundHolder BackgroundHolder;
	WConfigProperties* Preferences;

	// Constructors.
	WEditorFrame()
	: WMdiFrame( TEXT("EditorFrame") )
	, BackgroundHolder( NAME_None, &MdiClient )
	, Preferences( NULL )
	{
	}

	// WWindow interface.
	void OnCreate()
	{
		guard(WEditorFrame::OnCreate);
		WMdiFrame::OnCreate();
		SetText( *FString::Printf( LocalizeGeneral(TEXT("FrameWindow"),TEXT("UnrealEd")), LocalizeGeneral(TEXT("Product"),TEXT("Core"))) );

		// Create MDI client.
		CLIENTCREATESTRUCT ccs;
        ccs.hWindowMenu = NULL; 
        ccs.idFirstChild = 60000;
		MdiClient.OpenWindow( &ccs );

		// Background.
		BackgroundHolder.OpenWindow();

		NE_EdInit( hWnd, hWnd );

		// Set up progress dialog.
		GDlgProgress = new WDlgProgress( NULL, this );
		GDlgProgress->DoModeless();

		Warn.hWndProgressBar = (DWORD)::GetDlgItem( GDlgProgress->hWnd, IDPG_PROGRESS);
		Warn.hWndProgressText = (DWORD)::GetDlgItem( GDlgProgress->hWnd, IDSC_MSG);
		Warn.hWndProgressDlg = (DWORD)GDlgProgress->hWnd;

		GDlgSearchActors = new WDlgSearchActors( NULL, this );
		GDlgSearchActors->DoModeless();
		GDlgSearchActors->Show(0);

		GDlgScaleLights = new WDlgScaleLights( NULL, this );
		GDlgScaleLights->DoModeless();
		GDlgScaleLights->Show(0);

		GDlgTexReplace = new WDlgTexReplace( NULL, this );
		GDlgTexReplace->DoModeless();
		GDlgTexReplace->Show(0);

		GEditorFrame = this;

		unguard;
	}
	virtual void OnTimer()
	{
		guard(WEditorFrame::OnTimer);
		GEditor->Exec( TEXT("MAYBEAUTOSAVE") );
		unguard;
	}
	void RepositionClient()
	{
		guard(WEditorFrame::RepositionClient);
		WMdiFrame::RepositionClient();
		BackgroundHolder.MoveWindow( MdiClient.GetClientRect(), 1 );
		unguard;
	}
	void OnClose()
	{
		guard(WEditorFrame::OnClose);

		::DestroyWindow( GLevelFrame->hWnd );
		delete GLevelFrame;

		KillTimer( hWnd, 900 );

		GMRUList->WriteINI();

		delete GSurfPropSheet;
		delete GMatineeSheet;
		delete GBuildSheet;
		delete G2DShapeEditor;
		delete GBrowserSound;
		delete GBrowserMusic;
		delete GBrowserGroup;
		delete GBrowserMaster;
		delete GBrowserActor;
		delete GBrowserTexture;
		delete GBrowserMesh;
		delete GDlgAddSpecial;
		delete GDlgScaleLights;
		delete GDlgProgress;
		delete GDlgSearchActors;
		delete GDlgTexReplace;

		appRequestExit( 0 );
		WMdiFrame::OnClose();
		unguard;
	}
	void OnCommand( INT Command )
	{
		guard(WEditorFrame::OnCommand);
		TCHAR l_chCmd[255];

		switch( Command )
		{
			case WM_REDRAWALLVIEWPORTS:
				{
					GEditor->RedrawLevel( GEditor->Level );
					GButtonBar->UpdateButtons();
					GBottomBar->UpdateButtons();
					GTopBar->UpdateButtons();
				}
				break;

			case WM_SETCURRENTVIEWPORT:
				{
					if( GCurrentViewport != (DWORD)LastlParam && LastlParam )
					{
						GCurrentViewport = (DWORD)LastlParam;
						for( int x = 0 ; x < GViewports.Num() ; x++ )
						{
							if( GCurrentViewport == (DWORD)GViewports(x).m_pViewportFrame->m_pViewport )
							{
								GCurrentViewportFrame = GViewports(x).m_pViewportFrame->hWnd;
								break;
							}
						}
					}
					GLevelFrame->RedrawAllViewports();
				}
				break;

			case ID_FileNew:
			{
				FileSaveChanges( hWnd );
				//WNewObject Dialog( NULL, this );
				//UObject* Result = Dialog.DoModal();
				//if( Cast<ULevel>(Result) )
				//{
					GEditor->Exec(TEXT("MAP NEW"));
					GLevelFrame->SetMapFilename( TEXT("") );
					OpenLevelView();
					GButtonBar->RefreshBuilders();
					if( GBrowserGroup )
						GBrowserGroup->RefreshGroupList();
				//}
			}
			break;

			case ID_FILE_IMPORT:
			{
				OPENFILENAMEA ofn;
				ANSICHAR File[8192] = "\0";

				ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
				ofn.lStructSize = sizeof(OPENFILENAMEA);
				ofn.hwndOwner = hWnd;
				ofn.lpstrFile = File;
				ofn.nMaxFile = sizeof(char) * 8192;
				ofn.lpstrFilter = "Unreal Text (*.t3d)\0*.t3d\0All Files\0*.*\0\0";
				ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_UNR]) );
				ofn.lpstrDefExt = "t3d";
				ofn.lpstrTitle = "Import Map";
				ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR;

				// Display the Open dialog box. 
				if( GetOpenFileNameA(&ofn) )
				{
					WDlgMapImport l_dlg( this );
					if( l_dlg.DoModal( appFromAnsi( File ) ) )
					{
						GWarn->BeginSlowTask( TEXT("Importing Map"), 1, 0 );
						TCHAR l_chCmd[256];
						if( l_dlg.bNewMapCheck )
							appSprintf( l_chCmd, TEXT("MAP IMPORTADD FILE=\"%s\""), appFromAnsi( File ) );
						else
						{
							GLevelFrame->SetMapFilename( TEXT("") );
							OpenLevelView();
							appSprintf( l_chCmd, TEXT("MAP IMPORT FILE=\"%s\""), appFromAnsi( File ) );
						}
						GEditor->Exec( l_chCmd );
						GWarn->EndSlowTask();
						GEditor->RedrawLevel( GEditor->Level );

						FString S = appFromAnsi( File );
						GLastDir[eLASTDIR_UNR] = S.Left( S.InStr( TEXT("\\"), 1 ) );

						RefreshEditor();
						if( l_dlg.bNewMapCheck )
							GButtonBar->RefreshBuilders();
					}
				}

				GFileManager->SetDefaultDirectory(appBaseDir());
			}
			break;

			case ID_FILE_EXPORT:
			{
				OPENFILENAMEA ofn;
				char File[8192] = "\0";

				ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
				ofn.lStructSize = sizeof(OPENFILENAMEA);
				ofn.hwndOwner = hWnd;
				ofn.lpstrFile = File;
				ofn.nMaxFile = sizeof(char) * 8192;
				ofn.lpstrFilter = "Unreal Text (*.t3d)\0*.t3d\0All Files\0*.*\0\0";
				ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_UNR]) );
				ofn.lpstrDefExt = "t3d";
				ofn.lpstrTitle = "Export Map";
				ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT;

				if( GetSaveFileNameA(&ofn) )
				{
					GEditor->Exec( TEXT("BRUSHCLIP DELETE") );
					GEditor->Exec( *(FString::Printf(TEXT("MAP EXPORT FILE=\"%s\""), appFromAnsi( File ))));

					FString S = appFromAnsi( File );
					GLastDir[eLASTDIR_UNR] = S.Left( S.InStr( TEXT("\\"), 1 ) );
				}

				GFileManager->SetDefaultDirectory(appBaseDir());
			}
			break;

			case IDMN_MRU1:
			case IDMN_MRU2:
			case IDMN_MRU3:
			case IDMN_MRU4:
			case IDMN_MRU5:
			case IDMN_MRU6:
			case IDMN_MRU7:
			case IDMN_MRU8:
			{
				GLevelFrame->SetMapFilename( (TCHAR*)(*(GMRUList->Items[Command - IDMN_MRU1] ) ) );
				GEditor->Exec( *(FString::Printf(TEXT("MAP LOAD FILE=\"%s\""), *GMRUList->Items[Command - IDMN_MRU1] )) );
				RefreshEditor();
				GButtonBar->RefreshBuilders();
			}
			break;

			case IDMN_LOAD_BACK_IMAGE:
			{
				OPENFILENAMEA ofn;
				char File[8192] = "\0";

				ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
				ofn.lStructSize = sizeof(OPENFILENAMEA);
				ofn.hwndOwner = hWnd;
				ofn.lpstrFile = File;
				ofn.nMaxFile = sizeof(char) * 8192;
				ofn.lpstrFilter = "Bitmaps (*.bmp)\0*.bmp\0All Files\0*.*\0\0";
				ofn.lpstrInitialDir = "..\\maps";
				ofn.lpstrTitle = "Open Image";
				ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_UTX]) );
				ofn.lpstrDefExt = "bmp";
				ofn.Flags = OFN_NOCHANGEDIR;

				// Display the Open dialog box. 
				//
				if( GetOpenFileNameA(&ofn) )
				{
					GLevelFrame->LoadBackgroundImage(appFromAnsi( File ));

					FString S = appFromAnsi( File );
					GLastDir[eLASTDIR_UTX] = S.Left( S.InStr( TEXT("\\"), 1 ) );
				}

				InvalidateRect( GLevelFrame->hWnd, NULL, FALSE );
			}
			break;

			case IDMN_CLEAR_BACK_IMAGE:
			{
				::DeleteObject( GLevelFrame->hImage );
				GLevelFrame->hImage = NULL;
				GLevelFrame->BIFilename = TEXT("");
				InvalidateRect( GLevelFrame->hWnd, NULL, FALSE );
			}
			break;

			case IDMN_BI_CENTER:
			{
				GLevelFrame->BIMode = eBIMODE_CENTER;
				InvalidateRect( GLevelFrame->hWnd, NULL, FALSE );
			}
			break;

			case IDMN_BI_TILE:
			{
				GLevelFrame->BIMode = eBIMODE_TILE;
				InvalidateRect( GLevelFrame->hWnd, NULL, FALSE );
			}
			break;

			case IDMN_BI_STRETCH:
			{
				GLevelFrame->BIMode = eBIMODE_STRETCH;
				InvalidateRect( GLevelFrame->hWnd, NULL, FALSE );
			}
			break;

			case ID_FileOpen:
			{
				FileOpen( hWnd );
			}
			break;

			case ID_FileClose:
			{
				FileSaveChanges( hWnd );

				if( GLevelFrame )
				{
					GLevelFrame->_CloseWindow();
					delete GLevelFrame;
					GLevelFrame = NULL;
				}
			}
			break;

			case ID_FileSave:
			{
				FileSave( hWnd );
			}
			break;

			case ID_FileSaveAs:
			{
				FileSaveAs( hWnd );
			}
			break;

			case ID_BrowserMaster:
			{
				GBrowserMaster->Show(1);
			}
			break;

			case ID_BrowserTexture:
			{
				GBrowserMaster->ShowBrowser(eBROWSER_TEXTURE);
			}
			break;

			case ID_BrowserMesh:
			{
				GBrowserMaster->ShowBrowser(eBROWSER_MESH);
			}
			break;

			case ID_BrowserActor:
			{
				GBrowserMaster->ShowBrowser(eBROWSER_ACTOR);
			}
			break;

			case ID_BrowserSound:
			{
				GBrowserMaster->ShowBrowser(eBROWSER_SOUND);
			}
			break;

			case ID_BrowserMusic:
			{
				GBrowserMaster->ShowBrowser(eBROWSER_MUSIC);
			}
			break;

			case ID_BrowserGroup:
			{
				GBrowserMaster->ShowBrowser(eBROWSER_GROUP);
			}
			break;

			case ID_BrowserPrefabs:
			{
				appMsgf(TEXT("Not implemented yet."));
			}
			break;

			case IDMN_CODE_FRAME:
			{
				GBrowserMaster->ShowBrowser(eBROWSER_ACTOR);
				ShowCodeFrame( this );
			}
			break;

			case ID_FileExit:
			{
				OnClose();
			}
			break;

			case ID_EditUndo:
			{
				GEditor->Exec( TEXT("TRANSACTION UNDO") );
			}
			break;

			case ID_EditRedo:
			{
				GEditor->Exec( TEXT("TRANSACTION REDO") );
			}
			break;

			case ID_EditDuplicate:
			{
				GEditor->Exec( TEXT("DUPLICATE") );
			}
			break;

			case IDMN_EDIT_SEARCH:
			{
				GDlgSearchActors->Show(1);
			}
			break;

			case IDMN_EDIT_SCALE_LIGHTS:
			{
				GDlgScaleLights->Show(1);
			}
			break;

			case IDMN_EDIT_TEX_REPLACE:
			{
				GDlgTexReplace->Show(1);
			}
			break;

			case ID_EditDelete:
			{
				GEditor->Exec( TEXT("DELETE") );
			}
			break;

			case ID_EditCut:
			{
				GEditor->Exec( TEXT("EDIT CUT") );
			}
			break;

			case ID_EditCopy:
			{
				GEditor->Exec( TEXT("EDIT COPY") );
			}
			break;

			case ID_EditPaste:
			{
				GEditor->Exec( TEXT("EDIT PASTE") );
			}
			break;

			case ID_EditSelectNone:
			{
				GEditor->Exec( TEXT("SELECT NONE") );
			}
			break;

			case ID_EditSelectAllActors:
			{
				GEditor->Exec( TEXT("ACTOR SELECT ALL") );
			}
			break;

			case ID_EditSelectAllSurfs:
			{
				GEditor->Exec( TEXT("POLY SELECT ALL") );
			}
			break;

			case ID_ViewActorProp:
			{
				if( !GEditor->ActorProperties )
				{
					GEditor->ActorProperties = new WObjectProperties( TEXT("ActorProperties"), CPF_Edit, TEXT(""), NULL, 1 );
					GEditor->ActorProperties->OpenWindow( hWnd );
					GEditor->ActorProperties->SetNotifyHook( GEditor );
				}
				GEditor->UpdatePropertiesWindows();
				GEditor->ActorProperties->Show(1);
			}
			break;

			case ID_ViewSurfaceProp:
			{
				GSurfPropSheet->Show( TRUE );
			}
			break;

			case ID_ViewLevelProp:
			{
				if( !GEditor->LevelProperties )
				{
					GEditor->LevelProperties = new WObjectProperties( TEXT("LevelProperties"), CPF_Edit, TEXT("Level Properties"), NULL, 1 );
					GEditor->LevelProperties->OpenWindow( hWnd );
					GEditor->LevelProperties->SetNotifyHook( GEditor );
				}
				GEditor->LevelProperties->Root.SetObjects( (UObject**)&GEditor->Level->Actors(0), 1 );
				GEditor->LevelProperties->Show(1);
			}
			break;

			case ID_BrushClip:
			{
				GEditor->Exec( TEXT("BRUSHCLIP") );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_BrushClipSplit:
			{
				GEditor->Exec( TEXT("BRUSHCLIP SPLIT") );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_BrushClipFlip:
			{
				GEditor->Exec( TEXT("BRUSHCLIP FLIP") );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_BrushClipDelete:
			{
				GEditor->Exec( TEXT("BRUSHCLIP DELETE") );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_BrushAdd:
			{
				GEditor->Exec( TEXT("BRUSH ADD") );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_BrushSubtract:
			{
				GEditor->Exec( TEXT("BRUSH SUBTRACT") );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_BrushIntersect:
			{
				GEditor->Exec( TEXT("BRUSH FROM INTERSECTION") );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_BrushDeintersect:
			{
				GEditor->Exec( TEXT("BRUSH FROM DEINTERSECTION") );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_BrushAddMover:
			{
				GEditor->Exec( TEXT("BRUSH ADDMOVER") );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_BrushAddSpecial:
			{
				if( !GDlgAddSpecial )
				{
					GDlgAddSpecial = new WDlgAddSpecial( NULL, GEditorFrame );
					GDlgAddSpecial->DoModeless();
				}
				else
					GDlgAddSpecial->Show(1);
			}
			break;

			case ID_BrushOpen:
			{
				OPENFILENAMEA ofn;
				char File[8192] = "\0";

				ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
				ofn.lStructSize = sizeof(OPENFILENAMEA);
				ofn.hwndOwner = hWnd;
				ofn.lpstrFile = File;
				ofn.nMaxFile = sizeof(char) * 8192;
				ofn.lpstrFilter = "Brushes (*.u3d)\0*.u3d\0All Files\0*.*\0\0";
				ofn.lpstrInitialDir = "..\\maps";
				ofn.lpstrDefExt = "u3d";
				ofn.lpstrTitle = "Open Brush";
				ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR;

				// Display the Open dialog box. 
				if( GetOpenFileNameA(&ofn) )
				{
					GEditor->Exec( *(FString::Printf(TEXT("BRUSH LOAD FILE=\"%s\""), appFromAnsi( File ))));
					GEditor->RedrawLevel( GEditor->Level );
				}

				GFileManager->SetDefaultDirectory(appBaseDir());
				GButtonBar->RefreshBuilders();
			}
			break;

			case ID_BrushSaveAs:
			{
				OPENFILENAMEA ofn;
				char File[8192] = "\0";

				ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
				ofn.lStructSize = sizeof(OPENFILENAMEA);
				ofn.hwndOwner = hWnd;
				ofn.lpstrFile = File;
				ofn.nMaxFile = sizeof(char) * 8192;
				ofn.lpstrFilter = "Brushes (*.u3d)\0*.u3d\0All Files\0*.*\0\0";
				ofn.lpstrInitialDir = "..\\maps";
				ofn.lpstrDefExt = "u3d";
				ofn.lpstrTitle = "Save Brush";
				ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT;

				if( GetSaveFileNameA(&ofn) )
					GEditor->Exec( *(FString::Printf(TEXT("BRUSH SAVE FILE=\"%s\""), appFromAnsi( File ))));

				GFileManager->SetDefaultDirectory(appBaseDir());
			}
			break;

			case ID_BRUSH_IMPORT:
			{
				OPENFILENAMEA ofn;
				char File[8192] = "\0";

				ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
				ofn.lStructSize = sizeof(OPENFILENAMEA);
				ofn.hwndOwner = hWnd;
				ofn.lpstrFile = File;
				ofn.nMaxFile = sizeof(char) * 8192;
				ofn.lpstrFilter = "Import Types (*.t3d, *.dxf, *.asc)\0*.t3d;*.dxf;*.asc;\0All Files\0*.*\0\0";
				ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_BRUSH]) );
				ofn.lpstrDefExt = "t3d";
				ofn.lpstrTitle = "Import Brush";
				ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR;

				// Display the Open dialog box. 
				if( GetOpenFileNameA(&ofn) )
				{
					WDlgBrushImport l_dlg( NULL, this );
					l_dlg.DoModal( appFromAnsi( File ) );
					GEditor->RedrawLevel( GEditor->Level );

					FString S = appFromAnsi( File );
					GLastDir[eLASTDIR_BRUSH] = S.Left( S.InStr( TEXT("\\"), 1 ) );
				}

				GFileManager->SetDefaultDirectory(appBaseDir());
				GButtonBar->RefreshBuilders();
			}
			break;

			case ID_BRUSH_EXPORT:
			{
				OPENFILENAMEA ofn;
				char File[8192] = "\0";

				ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
				ofn.lStructSize = sizeof(OPENFILENAMEA);
				ofn.hwndOwner = hWnd;
				ofn.lpstrFile = File;
				ofn.nMaxFile = sizeof(char) * 8192;
				ofn.lpstrFilter = "Unreal Text (*.t3d)\0*.t3d\0All Files\0*.*\0\0";
				ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_BRUSH]) );
				ofn.lpstrDefExt = "t3d";
				ofn.lpstrTitle = "Export Brush";
				ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR | OFN_OVERWRITEPROMPT;

				if( GetSaveFileNameA(&ofn) )
				{
					GEditor->Exec( *(FString::Printf(TEXT("BRUSH EXPORT FILE=\"%s\""), appFromAnsi( File ))));

					FString S = appFromAnsi( File );
					GLastDir[eLASTDIR_BRUSH] = S.Left( S.InStr( TEXT("\\"), 1 ) );
				}

				GFileManager->SetDefaultDirectory(appBaseDir());
				GButtonBar->RefreshBuilders();
			}
			break;

			case ID_BuildPlay:
			{
				GEditor->Exec( TEXT("HOOK PLAYMAP") );
			}
			break;

			case ID_BuildGeometry:
			{
				UBOOL bVisibleOnly = SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_ONLY_VISIBLE), BM_GETCHECK, 0, 0 ) == BST_CHECKED;
				GEditor->Exec( *(FString::Printf(TEXT("MAP REBUILD VISIBLEONLY=%d"), bVisibleOnly) ) );
				GBuildSheet->RefreshStats();
			}
			break;

			case ID_BuildLighting:
			{
				UBOOL bVisibleOnly = SendMessageA( ::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_ONLY_VISIBLE), BM_GETCHECK, 0, 0 ) == BST_CHECKED;
				UBOOL bSelected = SendMessageA(::GetDlgItem( GhwndBSPages[eBS_OPTIONS], IDCK_SEL_LIGHTS_ONLY), BM_GETCHECK, 0, 0 ) == BST_CHECKED;
				GEditor->Exec( *(FString::Printf(TEXT("LIGHT APPLY SELECTED=%d VISIBLEONLY=%d"), bSelected, bVisibleOnly) ) );
				GBuildSheet->RefreshStats();
			}
			break;

			case ID_BuildPaths:
			{
				GEditor->Exec( TEXT("PATHS DEFINE") );
				GBuildSheet->RefreshStats();
			}
			break;

			case ID_BuildAll:
			{
				GBuildSheet->Build();
				GBuildSheet->RefreshStats();
			}
			break;

			case ID_BuildOptions:
			{
				GBuildSheet->Show( TRUE );
			}
			break;

			case ID_PurgeUnusedTex:
			{
				GEditor->Exec( TEXT("POLY PURGEUNUSEDTEXTURES") );
			}
			break;

			case ID_ToolsLog:
			{
				if( GLogWindow )
				{
					GLogWindow->Show(1);
					SetFocus( *GLogWindow );
					GLogWindow->Display.ScrollCaret();
				}
			}
			break;

			case ID_Tools2DEditor:
			{
				delete G2DShapeEditor;

				G2DShapeEditor = new W2DShapeEditor( TEXT("2D Shape Editor"), this );
				G2DShapeEditor->OpenWindow();
			}
			break;

			case ID_ViewNewFree:
			{
				if( GViewportStyle == VSTYLE_Floating )
					GLevelFrame->OpenFrameViewport( REN_OrthXY, 0, 0, 320, 200, SHOW_Menu | SHOW_Frame | SHOW_Actors | SHOW_Brush | SHOW_StandardView | SHOW_ChildWindow | SHOW_MovingBrushes );
			}
			break;

			case IDMN_VIEWPORT_CLOSEALL:
			{
				for( int x = 0 ; x < GViewports.Num() ; x++)
				{
					delete GViewports(x).m_pViewportFrame;
					GViewports(x).m_pViewportFrame = NULL;
				}
				GViewports.Empty();
			}
			break;

			case WM_EDC_ACTORPROPERTIESCHANGE:
			{
				GMatineeSheet->PropSheet->RefreshPages();
			}
			break;



			case IDMN_VIEWPORT_FLOATING:
			{
				GViewportStyle = VSTYLE_Floating;
				UpdateMenu();
				GLevelFrame->ChangeViewportStyle();
			}
			break;

			case IDMN_VIEWPORT_FIXED:
			{
				GViewportStyle = VSTYLE_Fixed;
				UpdateMenu();
				GLevelFrame->ChangeViewportStyle();
			}
			break;

			case IDMN_VIEWPORT_CONFIG:
			{
				WDlgViewportConfig l_dlg( NULL, this );
				if( l_dlg.DoModal( GViewportConfig ) )
					GLevelFrame->CreateNewViewports( GViewportStyle, l_dlg.ViewportConfig );
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case ID_ToolsPrefs:
			{
				if( !Preferences )
				{
					Preferences = new WConfigProperties( TEXT("Preferences"), LocalizeGeneral(TEXT("AdvancedOptionsTitle"),TEXT("Window")) );
					Preferences->OpenWindow( *this );
					Preferences->SetNotifyHook( this );
					Preferences->ForceRefresh();
				}
				Preferences->Show(1);
			}
			break;
//milestone 4 start
			case ID_VIEW_LABELPAWNS:
				{


					GPawnsLabel=GPawnsLabel^1;
					if(GPawnsLabel)
						CheckMenuItem(GMainMenu,ID_VIEW_LABELPAWNS,MF_CHECKED);
					else
						CheckMenuItem(GMainMenu,ID_VIEW_LABELPAWNS,MF_UNCHECKED);

					GEditor->RedrawLevel( GEditor->Level );

				}
				break;

			case ID_VIEW_LABELTRIGGERS:
				{


					GTriggersLabel=GTriggersLabel^1;
					if(GTriggersLabel)
						CheckMenuItem(GMainMenu,ID_VIEW_LABELTRIGGERS,MF_CHECKED);
					else
						CheckMenuItem(GMainMenu,ID_VIEW_LABELTRIGGERS,MF_UNCHECKED);

					GEditor->RedrawLevel( GEditor->Level );

				}
				break;
			case ID_VIEW_LABELSPECIAL:
				{


					GSlabelLabel=GSlabelLabel^1;
					if(GSlabelLabel)
						CheckMenuItem(GMainMenu,ID_VIEW_LABELSPECIAL,MF_CHECKED);
					else
						CheckMenuItem(GMainMenu,ID_VIEW_LABELSPECIAL,MF_UNCHECKED);

					GEditor->RedrawLevel( GEditor->Level );

				}
				break;
			case ID_VIEW_LABELLIGHTS:
				{


					GLightsLabel=GLightsLabel^1;
					if(GLightsLabel)
						CheckMenuItem(GMainMenu,ID_VIEW_LABELLIGHTS,MF_CHECKED);
					else
						CheckMenuItem(GMainMenu,ID_VIEW_LABELLIGHTS,MF_UNCHECKED);

					GEditor->RedrawLevel( GEditor->Level );

				}
				break;



			case ID_VIEW_LABELMOVERS:
				{


					GMoversLabel=GMoversLabel^1;
					if(GMoversLabel)
						CheckMenuItem(GMainMenu,ID_VIEW_LABELMOVERS,MF_CHECKED);
					else
						CheckMenuItem(GMainMenu,ID_VIEW_LABELMOVERS,MF_UNCHECKED);

					GEditor->RedrawLevel( GEditor->Level );

				}
				break;


//milestone 4 end
			case WM_EDC_SAVEMAP:
			{
				FileSave( hWnd );
			}
			break;

			case WM_EDC_SAVEMAPAS:
			{
				FileSaveAs( hWnd );
			}
			break;

			case WM_BROWSER_DOCK:
			{
				guard(WM_BROWSER_DOCK);
				int Browsr = LastlParam;
				switch( Browsr )
				{
					case eBROWSER_ACTOR:
						delete GBrowserActor;
						GBrowserActor = new WBrowserActor( TEXT("Actor Browser"), GBrowserMaster, GEditorFrame->hWnd );
						check(GBrowserActor);
						GBrowserActor->OpenWindow( 1 );
						GBrowserMaster->ShowBrowser(eBROWSER_ACTOR);
						break;

					case eBROWSER_GROUP:
						delete GBrowserGroup;
						GBrowserGroup = new WBrowserGroup( TEXT("Group Browser"), GBrowserMaster, GEditorFrame->hWnd );
						check(GBrowserGroup);
						GBrowserGroup->OpenWindow( 1 );
						GBrowserMaster->ShowBrowser(eBROWSER_GROUP);
						break;

					case eBROWSER_MUSIC:
						delete GBrowserMusic;
						GBrowserMusic = new WBrowserMusic( TEXT("Music Browser"), GBrowserMaster, GEditorFrame->hWnd );
						check(GBrowserMusic);
						GBrowserMusic->OpenWindow( 1 );
						GBrowserMaster->ShowBrowser(eBROWSER_MUSIC);
						break;

					case eBROWSER_SOUND:
						delete GBrowserSound;
						GBrowserSound = new WBrowserSound( TEXT("Sound Browser"), GBrowserMaster, GEditorFrame->hWnd );
						check(GBrowserSound);
						GBrowserSound->OpenWindow( 1 );
						GBrowserMaster->ShowBrowser(eBROWSER_SOUND);
						break;

					case eBROWSER_TEXTURE:
						delete GBrowserTexture;
						GBrowserTexture = new WBrowserTexture( TEXT("Texture Browser"), GBrowserMaster, GEditorFrame->hWnd );
						check(GBrowserTexture);
						GBrowserTexture->OpenWindow( 1 );
						GBrowserMaster->ShowBrowser(eBROWSER_TEXTURE);
						break;

					case eBROWSER_MESH:
						delete GBrowserMesh;
						GBrowserMesh = new WBrowserMesh( TEXT("Mesh Browser"), GBrowserMaster, GEditorFrame->hWnd );
						check(GBrowserMesh);
						GBrowserMesh->OpenWindow( 1 );
						GBrowserMaster->ShowBrowser(eBROWSER_MESH);
						break;
				}
				unguard;
			}
			break;

			case WM_BROWSER_UNDOCK:
			{
				guard(WM_BROWSER_UNDOCK);
				int Browsr = LastlParam;
				switch( Browsr )
				{
					case eBROWSER_ACTOR:
						delete GBrowserActor;
						GBrowserActor = new WBrowserActor( TEXT("Actor Browser"), GEditorFrame, GEditorFrame->hWnd );
						check(GBrowserActor);
						GBrowserActor->OpenWindow( 0 );
						GBrowserMaster->ShowBrowser(eBROWSER_ACTOR);
						break;

					case eBROWSER_GROUP:
						delete GBrowserGroup;
						GBrowserGroup = new WBrowserGroup( TEXT("Group Browser"), GEditorFrame, GEditorFrame->hWnd );
						check(GBrowserGroup);
						GBrowserGroup->OpenWindow( 0 );
						GBrowserMaster->ShowBrowser(eBROWSER_GROUP);
						break;

					case eBROWSER_MUSIC:
						delete GBrowserMusic;
						GBrowserMusic = new WBrowserMusic( TEXT("Music Browser"), GEditorFrame, GEditorFrame->hWnd );
						check(GBrowserMusic);
						GBrowserMusic->OpenWindow( 0 );
						GBrowserMaster->ShowBrowser(eBROWSER_MUSIC);
						break;

					case eBROWSER_SOUND:
						delete GBrowserSound;
						GBrowserSound = new WBrowserSound( TEXT("Sound Browser"), GEditorFrame, GEditorFrame->hWnd );
						check(GBrowserSound);
						GBrowserSound->OpenWindow( 0 );
						GBrowserMaster->ShowBrowser(eBROWSER_SOUND);
						break;

					case eBROWSER_TEXTURE:
						delete GBrowserTexture;
						GBrowserTexture = new WBrowserTexture( TEXT("Texture Browser"), GEditorFrame, GEditorFrame->hWnd );
						check(GBrowserTexture);
						GBrowserTexture->OpenWindow( 0 );
						GBrowserMaster->ShowBrowser(eBROWSER_TEXTURE);
						break;

					case eBROWSER_MESH:
						delete GBrowserMesh;
						GBrowserMesh = new WBrowserMesh( TEXT("Mesh Browser"), GEditorFrame, GEditorFrame->hWnd );
						check(GBrowserMesh);
						GBrowserMesh->OpenWindow( 0 );
						GBrowserMaster->ShowBrowser(eBROWSER_MESH);
						break;
				}

				GBrowserMaster->RefreshBrowserTabs( -1 );
				unguard;
			}
			break;

			case WM_EDC_CAMMODECHANGE:
			{
				if( GButtonBar )
				{
					GEditor->Exec( TEXT("BRUSHCLIP DELETE") );
					GButtonBar->UpdateButtons();
					GBottomBar->UpdateButtons();
					GTopBar->UpdateButtons();
				}

				if( GMatineeSheet )
				{
					if( GEditor->Mode == EM_Matinee )
					{
						GMatineeSheet->PropSheet->RefreshPages();
						GEditor->Exec( TEXT("HOOK ACTORPROPERTIES") );
					}
					GMatineeSheet->Show( GEditor->Mode == EM_Matinee );
				}


			}
			break;

			case WM_EDC_LOADMAP:
			{
				FileOpen( hWnd );
			}
			break;

			case WM_EDC_PLAYMAP:
			{
				GEditor->Exec( TEXT("HOOK PLAYMAP") );
			}
			break;

			case WM_EDC_BROWSE:
			{
				*GetPropResult = FStringOutputDevice();
				GEditor->Get( TEXT("OBJ"), TEXT("BROWSECLASS"), *GetPropResult );

				if( !appStrcmp( **GetPropResult, TEXT("Texture") ) )
					GBrowserMaster->ShowBrowser(eBROWSER_TEXTURE);

				if( !appStrcmp( **GetPropResult, TEXT("Palette") ) )
					GBrowserMaster->ShowBrowser(eBROWSER_TEXTURE);

				if( !appStrcmp( **GetPropResult, TEXT("Sound") ) )
					GBrowserMaster->ShowBrowser(eBROWSER_SOUND);

				if( !appStrcmp( **GetPropResult, TEXT("Music") ) )
					GBrowserMaster->ShowBrowser(eBROWSER_MUSIC);

				if( !appStrcmp( **GetPropResult, TEXT("Class") ) )
					GBrowserMaster->ShowBrowser(eBROWSER_ACTOR);

				if( !appStrcmp( **GetPropResult, TEXT("Mesh") ) )
					GBrowserMaster->ShowBrowser(eBROWSER_MESH);
					
			}
			break;

			case WM_EDC_USECURRENT:
			{
				*GetPropResult = FStringOutputDevice();
				GEditor->Get( TEXT("OBJ"), TEXT("BROWSECLASS"), *GetPropResult );

				FString Cur;

				if( !appStrcmp( **GetPropResult, TEXT("Palette") ) )
					if( GEditor->CurrentTexture )
						Cur = GEditor->CurrentTexture->Palette->GetPathName();

				if( !appStrcmp( **GetPropResult, TEXT("Texture") ) )
					if( GEditor->CurrentTexture )
						Cur = GEditor->CurrentTexture->GetPathName();

				if( !appStrcmp( **GetPropResult, TEXT("Sound") ) )
					if( GBrowserSound )
						Cur = *GBrowserSound->GetCurrentPathName();

				if( !appStrcmp( **GetPropResult, TEXT("Music") ) )
					if( GBrowserMusic )
						Cur = *GBrowserMusic->GetCurrentPathName();

				if( !appStrcmp( **GetPropResult, TEXT("Class") ) )
					if( GEditor->CurrentClass )
						Cur = GEditor->CurrentClass->GetPathName();

				if( !appStrcmp( **GetPropResult, TEXT("Mesh") ) )
					if( GBrowserMesh )
						Cur = GBrowserMesh->GetCurrentMeshName();

				if( Cur.Len() )
					GEditor->Set( TEXT("OBJ"), TEXT("NOTECURRENT"), *(FString::Printf(TEXT("CLASS=%s OBJECT=%s"), **GetPropResult, *Cur)));
			}
			break;

			case WM_EDC_CURTEXCHANGE:
			{
				if( GBrowserMaster->CurrentBrowser == eBROWSER_TEXTURE )
				{
					GBrowserTexture->SetCaption();
					GBrowserTexture->pViewport->Repaint(1);
				}
			}
			break;

			case WM_EDC_SELPOLYCHANGE:
			{
				GSurfPropSheet->GetDataFromSurfs1();
				GSurfPropSheet->RefreshStats();
			}
			break;

			case WM_EDC_SELCHANGE:
			{
				GSurfPropSheet->GetDataFromSurfs1();
				GSurfPropSheet->RefreshStats();
				GMatineeSheet->PropSheet->RefreshPages();
			}
			break;

			case WM_EDC_RTCLICKTEXTURE:
			{
				POINT pt;
				HMENU menu = GetSubMenu( LoadMenuIdX(hInstance, IDMENU_BrowserTexture_Context), 0 );
				::GetCursorPos( &pt );
				TrackPopupMenu( menu,
					TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
					pt.x, pt.y, 0,
					GBrowserTexture->hWnd, NULL);
			}
			break;

			case WM_EDC_RTCLICKPOLY:
			{
				POINT l_point;

				::GetCursorPos( &l_point );
				HMENU l_menu = GetSubMenu( LoadMenuIdX(hInstance, IDMENU_SurfPopup), 0 );

				// Customize the menu options we need to.
				MENUITEMINFOA mif;
				char l_ch[255];

				mif.cbSize = sizeof(MENUITEMINFO);
				mif.fMask = MIIM_TYPE;
				mif.fType = MFT_STRING;

				t_GetInfoRet gir = GetInfo( GEditor->Level, eGI_NUM_SURF_SELECTED );

				sprintf( l_ch, "Surface &Properties (%i Selected)\tF5", gir.iValue );
				mif.dwTypeData = l_ch;
				SetMenuItemInfoA( l_menu, ID_SurfProperties, FALSE, &mif );

				if( GEditor->CurrentClass )
				{
					sprintf( l_ch, "&Add %s Here", TCHAR_TO_ANSI( GEditor->CurrentClass->GetName() ) );
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_SurfPopupAddClass, FALSE, &mif );
				}
				else {

					DeleteMenu( l_menu, ID_SurfPopupAddClass, MF_BYCOMMAND );
				}

				if( GEditor->CurrentTexture )
				{
					sprintf( l_ch, "&Apply Texture : %s", TCHAR_TO_ANSI( GEditor->CurrentTexture->GetName() ) );
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_SurfPopupApplyTexture, FALSE, &mif );
				}



				//milestone 4 gk
				if(FavClass[0]==NULL)
				{	
					DeleteMenu(l_menu,ID_SURFPOPUP_Fav1, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[0]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_SURFPOPUP_Fav1, FALSE, &mif );
				}

				if(FavClass[1]==NULL)
				{	
					DeleteMenu(l_menu,ID_SURFPOPUP_Fav2, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[1]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_SURFPOPUP_Fav2, FALSE, &mif );
				}
				if(FavClass[2]==NULL)
				{	
					DeleteMenu(l_menu,ID_SURFPOPUP_Fav3, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[2]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_SURFPOPUP_Fav3, FALSE, &mif );
				}
				if(FavClass[3]==NULL)
				{	
					DeleteMenu(l_menu,ID_SURFPOPUP_Fav4, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[3]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_SURFPOPUP_Fav4, FALSE, &mif );
				}
					if(FavClass[4]==NULL)
				{	
					DeleteMenu(l_menu,ID_SURFPOPUP_Fav5, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[4]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_SURFPOPUP_Fav5, FALSE, &mif );
				}
				//end milestone 4 changes gk



				TrackPopupMenu( l_menu,
					TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
					l_point.x, l_point.y, 0,
					hWnd, NULL);
			}
			break;

			case WM_EDC_RTCLICKACTOR:
			{
				POINT l_point;

				::GetCursorPos( &l_point );
				HMENU l_menu = GetSubMenu( LoadMenuIdX(hInstance, IDMENU_ActorPopup), 0 );

				// Customize the menu options we need to.
				MENUITEMINFOA mif;
				char l_ch[255];

				mif.cbSize = sizeof(MENUITEMINFO);
				mif.fMask = MIIM_TYPE;
				mif.fType = MFT_STRING;

				t_GetInfoRet gir = GetInfo( GEditor->Level, eGI_NUM_SELECTED | eGI_CLASSNAME_SELECTED | eGI_CLASS_SELECTED );

				sprintf( l_ch, "%s &Properties (%i Selected)", TCHAR_TO_ANSI( *gir.String ), gir.iValue );
				mif.dwTypeData = l_ch;
				SetMenuItemInfoA( l_menu, IDMENU_ActorPopupProperties, FALSE, &mif );

				EnableMenuItem( l_menu, IDMENU_ActorPopupSetAsDefault, (gir.iValue != 1) );
				sprintf( l_ch, "Set as %s Defaults", TCHAR_TO_ANSI( *gir.String ), gir.iValue );
				mif.dwTypeData = l_ch;
				SetMenuItemInfoA( l_menu, IDMENU_ActorPopupSetAsDefault, FALSE, &mif );

				sprintf( l_ch, "&Select All %s", TCHAR_TO_ANSI( *gir.String ) );
				mif.dwTypeData = l_ch;
				SetMenuItemInfoA( l_menu, IDMENU_ActorPopupSelectAllClass, FALSE, &mif );

				EnableMenuItem( l_menu, IDMENU_ActorPopupEditScript, (gir.pClass == NULL) );
				EnableMenuItem( l_menu, IDMENU_ActorPopupMakeCurrent, (gir.pClass == NULL) );

				TrackPopupMenu( l_menu,
					TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
					l_point.x, l_point.y, 0,
					hWnd, NULL);
			}
			break;

			case WM_EDC_RTCLICKWINDOW:
			case WM_EDC_RTCLICKWINDOWCANADD:
			{
				POINT l_point;

				::GetCursorPos( &l_point );
				HMENU l_menu = GetSubMenu( LoadMenuIdX(hInstance, IDMENU_BackdropPopup), 0 );

				// Customize the menu options we need to.
				MENUITEMINFOA mif;
				char l_ch[255];

				mif.cbSize = sizeof(MENUITEMINFO);
				mif.fMask = MIIM_TYPE;
				mif.fType = MFT_STRING;

				if( GEditor->CurrentClass )
				{
					sprintf( l_ch, "&Add %s here", TCHAR_TO_ANSI( GEditor->CurrentClass->GetName() ) );
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_BackdropPopupAddClassHere, FALSE, &mif );
				}
				else {

					DeleteMenu( l_menu, ID_BackdropPopupAddClassHere, MF_BYCOMMAND );
				}

				//milestone 4 gk
				if(FavClass[0]==NULL)
				{	
					DeleteMenu(l_menu,ID_BACKDROPPOPUP_Fav1, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[0]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_BACKDROPPOPUP_Fav1, FALSE, &mif );
				}

				if(FavClass[1]==NULL)
				{	
					DeleteMenu(l_menu,ID_BACKDROPPOPUP_Fav2, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[1]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_BACKDROPPOPUP_Fav2, FALSE, &mif );
				}
				if(FavClass[2]==NULL)
				{	
					DeleteMenu(l_menu,ID_BACKDROPPOPUP_Fav3, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[2]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_BACKDROPPOPUP_Fav3, FALSE, &mif );
				}
				if(FavClass[3]==NULL)
				{	
					DeleteMenu(l_menu,ID_BACKDROPPOPUP_Fav4, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[3]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_BACKDROPPOPUP_Fav4, FALSE, &mif );
				}
					if(FavClass[4]==NULL)
				{	
					DeleteMenu(l_menu,ID_BACKDROPPOPUP_Fav5, MF_BYCOMMAND);
				}
				else
				{
					sprintf(l_ch,"Add %s here",TCHAR_TO_ANSI(FavClass[4]->GetName()));
					mif.dwTypeData = l_ch;
					SetMenuItemInfoA( l_menu, ID_BACKDROPPOPUP_Fav5, FALSE, &mif );
				}
				//end milestone 4 changes gk
				TrackPopupMenu( l_menu,
					TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RIGHTBUTTON,
					l_point.x, l_point.y, 0,
					hWnd, NULL);
			}
			break;

			case WM_EDC_MAPCHANGE:
			{
			}
			break;

			case WM_EDC_VIEWPORTUPDATEWINDOWFRAME:
			{
				for( int x = 0 ; x < GViewports.Num() ; x++)
					if( GViewports(x).m_pViewportFrame && ::IsWindow( GViewports(x).m_pViewportFrame->hWnd ) )
						UpdateWindow(GViewports(x).m_pViewportFrame->hWnd);
			}
			break;

			case WM_EDC_SURFPROPS:
			{
				GSurfPropSheet->Show( TRUE );
			}
			break;

			//
			// BACKDROP POPUP
			//

			// Root
			case ID_BackdropPopupAddClassHere:
			{
				int count;
				UClass* Fav;
				//milestone 4 gk
				count=0;
				while(count<5)
				{
					if(FavClass[count]==GEditor->CurrentClass)
					{
						FavCount[count]=FavCount[count]+1;	//up its access count
						break;	//already captured this one
					}
					count++;
				}
				if(count==5) //no previous match
				{
					count=0;
					while(count<5)
					{
						Fav=FavClass[count];
						if(FavClass[count]==NULL)
						{
							
							FavClass[count]=GEditor->CurrentClass;
							break;
						}
						count++;
					}
					if(count==5)	//no empties
					{
						int lowUse=FavCount[0];
						int lowLoser=0;
						count=0;
						while(count<5)
						{
							if(lowUse>FavCount[count])
							{
								lowUse=FavCount[count];
								lowLoser=count;
							}
							count++;
						}
						FavClass[lowLoser]=GEditor->CurrentClass;
						FavCount[lowLoser]=0;
					}
				}

				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), GEditor->CurrentClass->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
			}
			break;

			case ID_BackdropPopupAddLightHere:
			{
				GEditor->Exec( TEXT("ACTOR ADD CLASS=LIGHT") );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
			}
			break;

			case ID_BackdropPopupLevelProperties:
			{
				if( !GEditor->LevelProperties )
				{
					GEditor->LevelProperties = new WObjectProperties( TEXT("LevelProperties"), CPF_Edit, TEXT("Level Properties"), NULL, 1 );
					GEditor->LevelProperties->OpenWindow( hWnd );
					GEditor->LevelProperties->SetNotifyHook( GEditor );
				}
				GEditor->LevelProperties->Root.SetObjects( (UObject**)&GEditor->Level->Actors(0), 1 );
				GEditor->LevelProperties->Show(1);
			}
			break;

			// Grid
			case ID_BackdropPopupGrid1:
			{
				GEditor->Exec( TEXT("MAP GRID X=1 Y=1 Z=1") );
			}
			break;

			case ID_BackdropPopupGrid2:
			{
				GEditor->Exec( TEXT("MAP GRID X=2 Y=2 Z=2") );
			}
			break;

			case ID_BackdropPopupGrid4:
			{
				GEditor->Exec( TEXT("MAP GRID X=4 Y=4 Z=4") );
			}
			break;

			case ID_BackdropPopupGrid8:
			{
				GEditor->Exec( TEXT("MAP GRID X=8 Y=8 Z=8") );
			}
			break;

			case ID_BackdropPopupGrid16:
			{
				GEditor->Exec( TEXT("MAP GRID X=16 Y=16 Z=16") );
			}
			break;

			case ID_BackdropPopupGrid32:
			{
				GEditor->Exec( TEXT("MAP GRID X=32 Y=32 Z=32") );
			}
			break;

			case ID_BackdropPopupGrid64:
			{
				GEditor->Exec( TEXT("MAP GRID X=64 Y=64 Z=64") );
			}
			break;

			case ID_BackdropPopupGrid128:
			{
				GEditor->Exec( TEXT("MAP GRID X=128 Y=128 Z=128") );
			}
			break;

			case ID_BackdropPopupGrid256:
			{
				GEditor->Exec( TEXT("MAP GRID X=256 Y=256 Z=256") );
			}
			break;

			//milestone 4

			case ID_BACKDROPPOPUP_Fav1:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[0]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[0]+1;
	
				break;

			case ID_BACKDROPPOPUP_Fav2:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[1]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[1]+1;
	
				break;

			case ID_BACKDROPPOPUP_Fav3:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[2]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[2]+1;
	
				break;

			case ID_BACKDROPPOPUP_Fav4:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[3]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[3]+1;
	
				break;

			case ID_BACKDROPPOPUP_Fav5:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[4]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[4]+1;
	
				break;


			// Pivot
			case ID_BackdropPopupPivotSnapped:
			{
				GEditor->Exec( TEXT("PIVOT SNAPPED") );
			}
			break;

			case ID_BackdropPopupPivot:
			{
				GEditor->Exec( TEXT("PIVOT HERE") );
			}
			break;

			//
			// SURFACE POPUP MENU
			//

			// Root
			case ID_SurfProperties:
			{
				GSurfPropSheet->Show( TRUE );
			}
			break;

			case ID_SurfPopupAddClass:
			{
				int count;
				UClass* Fav;
				//milestone 4 gk
				count=0;
				while(count<5)
				{
					if(FavClass[count]==GEditor->CurrentClass)
					{
						FavCount[count]=FavCount[count]+1;	//up its access count
						break;	//already captured this one
					}
					count++;
				}
				if(count==5) //no previous match
				{
					count=0;
					while(count<5)
					{
						Fav=FavClass[count];
						if(FavClass[count]==NULL)
						{
							
							FavClass[count]=GEditor->CurrentClass;
							break;
						}
						count++;
					}
					if(count==5)	//no empties
					{
						int lowUse=FavCount[0];
						int lowLoser=0;
						count=0;
						while(count<5)
						{
							if(lowUse>FavCount[count])
							{
								lowUse=FavCount[count];
								lowLoser=count;
							}
							count++;
						}
						FavClass[lowLoser]=GEditor->CurrentClass;
						FavCount[lowLoser]=0;
					}
				}

				if( GEditor->CurrentClass )
				{
					GEditor->Exec( *(FString::Printf(TEXT("ACTOR ADD CLASS=%s"), GEditor->CurrentClass->GetName())));
					GEditor->Exec( TEXT("POLY SELECT NONE") );
				}
			}
			break;

			case ID_SurfPopupAddLight:
			{
				GEditor->Exec( TEXT("ACTOR ADD CLASS=LIGHT") );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
			}
			break;

			case ID_SurfPopupApplyTexture:
			{
				GEditor->Exec( TEXT("POLY SETTEXTURE") );
			}
			break;

			// Align Selected
			case ID_SurfPopupAlignFloor:
			{
				GEditor->Exec( TEXT("POLY TEXALIGN FLOOR") );
			}
			break;

			case ID_SurfPopupAlignWallDirection:
			{
				GEditor->Exec( TEXT("POLY TEXALIGN WALLDIR") );
			}
			break;

			case ID_SurfPopupAlignWallPanning:
			{
				GEditor->Exec( TEXT("POLY TEXALIGN WALLPAN") );
			}
			break;

			case ID_SurfPopupUnalign:
			{
				GEditor->Exec( TEXT("POLY TEXALIGN DEFAULT") );
			}
			break;

			// Select Surfaces
			case ID_SurfPopupSelectMatchingGroups:
			{
				GEditor->Exec( TEXT("POLY SELECT MATCHING GROUPS") );
			}
			break;

			//milestone 4

			case ID_SURFPOPUP_Fav1:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[0]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[0]+1;
	
				break;

			case ID_SURFPOPUP_Fav2:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[1]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[1]+1;
	
				break;

			case ID_SURFPOPUP_Fav3:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[2]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[2]+1;
	
				break;

			case ID_SURFPOPUP_Fav4:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[3]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[3]+1;
	
				break;

			case ID_SURFPOPUP_Fav5:
				GEditor->Exec( *(FString::Printf( TEXT("ACTOR ADD CLASS=%s"), FavClass[4]->GetName() ) ) );
				GEditor->Exec( TEXT("POLY SELECT NONE") );
				FavCount[0]=FavCount[4]+1;
	
				break;



			case ID_SurfPopupSelectMatchingItems:
			{
				GEditor->Exec( TEXT("POLY SELECT MATCHING ITEMS") );
			}
			break;

			case ID_SurfPopupSelectMatchingBrush:
			{
				GEditor->Exec( TEXT("POLY SELECT MATCHING BRUSH") );
			}
			break;

			case ID_SurfPopupSelectMatchingTexture:
			{
				GEditor->Exec( TEXT("POLY SELECT MATCHING TEXTURE") );
			}
			break;

			case ID_SurfPopupSelectAllAdjacents:
			{
				GEditor->Exec( TEXT("POLY SELECT ADJACENT ALL") );
			}
			break;

			case ID_SurfPopupSelectAdjacentCoplanars:
			{
				GEditor->Exec( TEXT("POLY SELECT ADJACENT COPLANARS") );
			}
			break;

			case ID_SurfPopupSelectAdjacentWalls:
			{
				GEditor->Exec( TEXT("POLY SELECT ADJACENT WALLS") );
			}
			break;

			case ID_SurfPopupSelectAdjacentFloors:
			{
				GEditor->Exec( TEXT("POLY SELECT ADJACENT FLOORS") );
			}
			break;

			case ID_SurfPopupSelectAdjacentSlants:
			{
				GEditor->Exec( TEXT("POLY SELECT ADJACENT SLANTS") );
			}
			break;

			case ID_SurfPopupSelectReverse:
			{
				GEditor->Exec( TEXT("POLY SELECT REVERSE") );
			}
			break;

			case ID_SurfPopupMemorize:
			{
				GEditor->Exec( TEXT("POLY SELECT MEMORY SET") );
			}
			break;

			case ID_SurfPopupRecall:
			{
				GEditor->Exec( TEXT("POLY SELECT MEMORY RECALL") );
			}
			break;

			case ID_SurfPopupOr:
			{
				GEditor->Exec( TEXT("POLY SELECT MEMORY INTERSECTION") );
			}
			break;

			case ID_SurfPopupAnd:
			{
				GEditor->Exec( TEXT("POLY SELECT MEMORY UNION") );
			}
			break;

			case ID_SurfPopupXor:
			{
				GEditor->Exec( TEXT("POLY SELECT MEMORY XOR") );
			}
			break;


			//
			// ACTOR POPUP MENU
			//

			// Root
			case IDMENU_ActorPopupProperties:
			{
				GEditor->Exec( TEXT("HOOK ACTORPROPERTIES") );
			}
			break;

			case IDMENU_ActorPopupSetAsDefault:
			{
				GEditor->Exec( TEXT("ACTOR SETASDEFAULT") );
			}
			break;

			case IDMENU_ActorPopupSetToDefault:
			{
				GEditor->Exec( TEXT("ACTOR SETTODEFAULT") );
			}
			break;

			case IDMENU_ActorPopupSelectAllClass:
			{
				t_GetInfoRet gir = GetInfo( GEditor->Level, eGI_NUM_SELECTED | eGI_CLASSNAME_SELECTED );

				if( gir.iValue )
				{
					appSprintf( l_chCmd, TEXT("ACTOR SELECT OFCLASS CLASS=%s"), *gir.String );
					GEditor->Exec( l_chCmd );
				}
			}
			break;

			case IDMENU_ActorPopupSelectAll:
			{
				GEditor->Exec( TEXT("ACTOR SELECT ALL") );
			}
			break;

			case IDMENU_ActorPopupSelectNone:
			{
				GEditor->Exec( TEXT("SELECT NONE") );
			}
			break;

			case IDMENU_ActorPopupDuplicate:
			{
				GEditor->Exec( TEXT("ACTOR DUPLICATE") );
			}
			break;

			case IDMENU_ActorPopupDelete:
			{
				GEditor->Exec( TEXT("ACTOR DELETE") );
			}
			break;

			case IDMENU_ActorPopupEditScript:
			{
				GBrowserMaster->ShowBrowser(eBROWSER_ACTOR);
				t_GetInfoRet gir = GetInfo( GEditor->Level, eGI_CLASS_SELECTED );
				GCodeFrame->AddClass( gir.pClass );
			}
			break;

			case IDMENU_ActorPopupMakeCurrent:
			{
				t_GetInfoRet gir = GetInfo( GEditor->Level, eGI_CLASSNAME_SELECTED );
				GEditor->Exec( *(FString::Printf(TEXT("SETCURRENTCLASS CLASS=%s"), *gir.String)) );
			}
			break;

			case IDMENU_ActorPopupMerge:
			{
				GWarn->BeginSlowTask( TEXT("Merging Faces"), 1, 0 );
				for( int i=0; i<GEditor->Level->Actors.Num(); i++ )
				{
					GWarn->StatusUpdatef( i, GEditor->Level->Actors.Num(), TEXT("Merging Faces") );
					AActor* pActor = GEditor->Level->Actors(i);
					if( pActor && pActor->bSelected && pActor->IsBrush() )
						GEditor->bspValidateBrush( pActor->Brush, 1, 1 );
				}
				GEditor->RedrawLevel( GEditor->Level );
				GWarn->EndSlowTask();
			}
			break;

			case IDMENU_ActorPopupSeparate:
			{
				GWarn->BeginSlowTask( TEXT("Separating Faces"), 1, 0 );
				for( int i=0; i<GEditor->Level->Actors.Num(); i++ )
				{
					GWarn->StatusUpdatef( i, GEditor->Level->Actors.Num(), TEXT("Separating Faces") );
					AActor* pActor = GEditor->Level->Actors(i);
					if( pActor && pActor->bSelected && pActor->IsBrush() )
						GEditor->bspUnlinkPolys( pActor->Brush );
				}
				GEditor->RedrawLevel( GEditor->Level );
				GWarn->EndSlowTask();
			}
			break;

			// Select Brushes
			case IDMENU_ActorPopupSelectBrushesAdd:
			{
				GEditor->Exec( TEXT("MAP SELECT ADDS") );
			}
			break;

			case IDMENU_ActorPopupSelectBrushesSubtract:
			{
				GEditor->Exec( TEXT("MAP SELECT SUBTRACTS") );
			}
			break;

			case IDMENU_ActorPopupSubtractBrushesSemisolid:
			{
				GEditor->Exec( TEXT("MAP SELECT SEMISOLIDS") );
			}
			break;

			case IDMENU_ActorPopupSelectBrushesNonsolid:
			{
				GEditor->Exec( TEXT("MAP SELECT NONSOLIDS") );
			}
			break;

			// Movers
			case IDMN_ActorPopupShowPolys:
			{
				for( INT i=0; i<GEditor->Level->Actors.Num(); i++ )
				{
					ABrush* Brush = Cast<ABrush>(GEditor->Level->Actors(i));
					if( Brush && Brush->IsMovingBrush() && Brush->bSelected )
					{
						Brush->Brush->EmptyModel( 1, 0 );
						Brush->Brush->BuildBound();
						GEditor->bspBuild( Brush->Brush, BSP_Good, 15, 1, 0 );
						GEditor->bspRefresh( Brush->Brush, 1 );
						GEditor->bspValidateBrush( Brush->Brush, 1, 1 );
						GEditor->bspBuildBounds( Brush->Brush );

						GEditor->bspBrushCSG( Brush, GEditor->Level->Model, 0, CSG_Add, 1 );
					}
				}
				GEditor->RedrawLevel( GEditor->Level );
			}
			break;

			case IDMENU_ActorPopupKey0:
			{
				GEditor->Exec( TEXT("ACTOR KEYFRAME NUM=0") );
			}
			break;

			case IDMENU_ActorPopupKey1:
			{
				GEditor->Exec( TEXT("ACTOR KEYFRAME NUM=1") );
			}
			break;

			case IDMENU_ActorPopupKey2:
			{
				GEditor->Exec( TEXT("ACTOR KEYFRAME NUM=2") );
			}
			break;

			case IDMENU_ActorPopupKey3:
			{
				GEditor->Exec( TEXT("ACTOR KEYFRAME NUM=3") );
			}
			break;

			case IDMENU_ActorPopupKey4:
			{
				GEditor->Exec( TEXT("ACTOR KEYFRAME NUM=4") );
			}
			break;

			case IDMENU_ActorPopupKey5:
			{
				GEditor->Exec( TEXT("ACTOR KEYFRAME NUM=5") );
			}
			break;

			case IDMENU_ActorPopupKey6:
			{
				GEditor->Exec( TEXT("ACTOR KEYFRAME NUM=6") );
			}
			break;

			case IDMENU_ActorPopupKey7:
			{
				GEditor->Exec( TEXT("ACTOR KEYFRAME NUM=7") );
			}
			break;

			// Reset
			case IDMENU_ActorPopupResetOrigin:
			{
				GEditor->Exec( TEXT("ACTOR RESET LOCATION") );
			}
			break;

			case IDMENU_ActorPopupResetPivot:
			{
				GEditor->Exec( TEXT("ACTOR RESET PIVOT") );
			}
			break;

			case IDMENU_ActorPopupResetRotation:
			{
				GEditor->Exec( TEXT("ACTOR RESET ROTATION") );
			}
			break;

			case IDMENU_ActorPopupResetScaling:
			{
				GEditor->Exec( TEXT("ACTOR RESET SCALE") );
			}
			break;

			case IDMENU_ActorPopupResetAll:
			{
				GEditor->Exec( TEXT("ACTOR RESET ALL") );
			}
			break;

			// Transform
			case IDMENU_ActorPopupMirrorX:
			{
				GEditor->Exec( TEXT("ACTOR MIRROR X=-1") );
			}
			break;

			case IDMENU_ActorPopupMirrorY:
			{
				GEditor->Exec( TEXT("ACTOR MIRROR Y=-1") );
			}
			break;

			case IDMENU_ActorPopupMirrorZ:
			{
				GEditor->Exec( TEXT("ACTOR MIRROR Z=-1") );
			}
			break;

			case IDMENU_ActorPopupPerm:
			{
				GEditor->Exec( TEXT("ACTOR APPLYTRANSFORM") );
			}
			break;

			// Order
			case IDMENU_ActorPopupToFirst:
			{
				GEditor->Exec( TEXT("MAP SENDTO FIRST") );
			}
			break;

			case IDMENU_ActorPopupToLast:
			{
				GEditor->Exec( TEXT("MAP SENDTO LAST") );
			}
			break;

			// Copy Polygons
			case IDMENU_ActorPopupToBrush:
			{
				GEditor->Exec( TEXT("MAP BRUSH GET") );
			}
			break;

			case IDMENU_ActorPopupFromBrush:
			{
				GEditor->Exec( TEXT("MAP BRUSH PUT") );
			}
			break;

			// Solidity
			case IDMENU_ActorPopupMakeSolid:
			{
				appSprintf( l_chCmd, TEXT("MAP SETBRUSH CLEARFLAGS=%d SETFLAGS=%d"), PF_Semisolid + PF_NotSolid, 0);
				GEditor->Exec( l_chCmd );
			}
			break;

			case IDMENU_ActorPopupMakeSemisolid:
			{
				appSprintf( l_chCmd, TEXT("MAP SETBRUSH CLEARFLAGS=%d SETFLAGS=%d"), PF_Semisolid + PF_NotSolid, PF_Semisolid );
				GEditor->Exec( l_chCmd );
			}
			break;

			case IDMENU_ActorPopupMakeNonSolid:
			{
				appSprintf( l_chCmd, TEXT("MAP SETBRUSH CLEARFLAGS=%d SETFLAGS=%d"), PF_Semisolid + PF_NotSolid, PF_NotSolid );
				GEditor->Exec( l_chCmd );
			}
			break;

			// CSG
			case IDMENU_ActorPopupMakeAdd:
			{
				GEditor->Exec( *(FString::Printf(TEXT("MAP SETBRUSH CSGOPER=%d"), CSG_Add) ) );
			}
			break;

			case IDMENU_ActorPopupMakeSubtract:
			{
				GEditor->Exec( *(FString::Printf(TEXT("MAP SETBRUSH CSGOPER=%d"), CSG_Subtract) ) );
			}
			break;
		
			default:
				WMdiFrame::OnCommand(Command);
			}
		unguard;
	}
	void NotifyDestroy( void* Other )
	{
		if( Other==Preferences )
			Preferences=NULL;
	}

	// FDocumentManager interface.
	virtual void OpenLevelView()
	{
		guard(WEditorFrame::OpenLevelView);

		// This is making it so you can only open one level window - it will reuse it for each
		// map you load ... which is not really MDI.  But the editor has problems with 2+ level windows open.  
		// Fix if you can...
		if( !GLevelFrame )
		{
			GLevelFrame = new WLevelFrame( GEditor->Level, TEXT("LevelFrame"), &BackgroundHolder );
			GLevelFrame->OpenWindow( 1, 1 );
		}

		unguard;
	}
};


void UpdateMenu()
{
	guard(UpdateMenu);

	CheckMenuItem( GMainMenu, IDMN_VIEWPORT_FLOATING, MF_BYCOMMAND | (GViewportStyle == VSTYLE_Floating ? MF_CHECKED : MF_UNCHECKED) );
	CheckMenuItem( GMainMenu, IDMN_VIEWPORT_FIXED, MF_BYCOMMAND | (GViewportStyle == VSTYLE_Fixed ? MF_CHECKED : MF_UNCHECKED) );

	EnableMenuItem( GMainMenu, ID_ViewNewFree, MF_BYCOMMAND | (GViewportStyle == VSTYLE_Floating ? MF_ENABLED : MF_GRAYED) );

	unguard;
}

void FileOpen( HWND hWnd )
{
	guard(FileOpen);
	FileSaveChanges( hWnd );

	OPENFILENAMEA ofn;
	char File[255] = "\0";

	ZeroMemory(&ofn, sizeof(OPENFILENAMEA));
	ofn.lStructSize = sizeof(OPENFILENAMEA);
	ofn.hwndOwner = hWnd;
	ofn.lpstrFile = File;
	ofn.nMaxFile = sizeof(File);
	char Filter[255];
	::sprintf( Filter,
		"Map Files (*.%s)%c*.%s%cAll Files%c*.*%c%c",
		appToAnsi( *GMapExt ),
		'\0',
		appToAnsi( *GMapExt ),
		'\0',
		'\0',
		'\0',
		'\0' );
	ofn.lpstrFilter = Filter;
	ofn.lpstrInitialDir = appToAnsi( *(GLastDir[eLASTDIR_UNR]) );
	ofn.lpstrDefExt = appToAnsi( *GMapExt );
	ofn.Flags = OFN_HIDEREADONLY | OFN_NOCHANGEDIR;

	// Display the Open dialog box. 
	if( GetOpenFileNameA(&ofn) )
	{
		// Make sure there's a level frame open.
		GEditorFrame->OpenLevelView();
			
		// Convert the ANSI filename to UNICODE, and tell the editor to open it.
		GLevelFrame->SetMapFilename( (TCHAR*)appFromAnsi(File) );
		GEditor->Exec( *(FString::Printf(TEXT("MAP LOAD FILE=\"%s\""), GLevelFrame->GetMapFilename() ) ) );

		FString S = GLevelFrame->GetMapFilename();
		GMRUList->AddItem( GLevelFrame->GetMapFilename() );
		GMRUList->AddToMenu( hWnd, GMainMenu, 1 );

		GLastDir[eLASTDIR_UNR] = S.Left( S.InStr( TEXT("\\"), 1 ) );

		GMRUList->AddItem( GLevelFrame->GetMapFilename() );
		GMRUList->AddToMenu( hWnd, GMainMenu, 1 );
	}

	// Make sure that the browsers reflect any new data the map brought with it.
	RefreshEditor();
	GButtonBar->RefreshBuilders();

	GFileManager->SetDefaultDirectory(appBaseDir());
	unguard;
}

/*-----------------------------------------------------------------------------
	WinMain.
-----------------------------------------------------------------------------*/

//
// Main window entry point.
//
INT WINAPI WinMain( HINSTANCE hInInstance, HINSTANCE hPrevInstance, char* InCmdLine, INT nCmdShow )
{
	// Remember instance.
	GIsStarted = 1;
	hInstance = hInInstance; 

	// Set package name.
	appStrcpy( GPackage, appPackage() );

	// Begin.
#ifndef _DEBUG
	try
	{
#endif
		// Set mode.
		GIsClient = GIsServer = GIsEditor = GLazyLoad = 1;
		GIsScriptable = 0;

		// Start main loop.
		GIsGuarded=1;

		// Create a fully qualified pathname for the log file.  If we don't do this, pieces of the log file
		// tends to get written into various directories as the editor starts up.
		TCHAR chLogFilename[256] = TEXT("\0");
		appSprintf( chLogFilename, TEXT("%s%s"), appUserDir(), TEXT("Editor.log") );
		appStrcpy( Log.Filename, chLogFilename );

		appInit( TEXT("HP"), GetCommandLine(), &Malloc, &Log, &Error, &Warn, &FileManager, FConfigCacheIni::Factory, 1 );
		GetPropResult = new FStringOutputDevice;

		// Init windowing.
		InitWindowing();
		IMPLEMENT_WINDOWCLASS(WMdiFrame,CS_DBLCLKS);
		IMPLEMENT_WINDOWCLASS(WEditorFrame,CS_DBLCLKS);
		IMPLEMENT_WINDOWCLASS(WBackgroundHolder,CS_DBLCLKS);
		IMPLEMENT_WINDOWCLASS(WLevelFrame,CS_DBLCLKS);
		IMPLEMENT_WINDOWCLASS(WDockingFrame,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WCodeFrame,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(W2DShapeEditor,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WViewportFrame,CS_DBLCLKS);
		IMPLEMENT_WINDOWCLASS(WBrowser,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WBrowserSound,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WBrowserMusic,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WBrowserGroup,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WBrowserMaster,CS_DBLCLKS  | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WBrowserTexture,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WBrowserMesh,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WBrowserActor,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWSUBCLASS(WMdiClient,TEXT("MDICLIENT"));
		IMPLEMENT_WINDOWCLASS(WButtonBar,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WButtonGroup,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WBottomBar,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WVFToolBar,CS_DBLCLKS);
		IMPLEMENT_WINDOWCLASS(WTopBar,CS_DBLCLKS | CS_VREDRAW | CS_HREDRAW);
		IMPLEMENT_WINDOWCLASS(WPageMatGeneral,CS_DBLCLKS);
		IMPLEMENT_WINDOWCLASS(WMatineeSheet,CS_DBLCLKS);

		// Windows.
		WEditorFrame Frame;
		GDocumentManager = &Frame;
		Frame.OpenWindow();
		InvalidateRect( Frame, NULL, 1 );
		UpdateWindow( Frame );
		UBOOL ShowLog = ParseParam(appCmdLine(),TEXT("log"));
		if( !ShowLog && !ParseParam(appCmdLine(),TEXT("server")) )
		InitSplash( TEXT("EdSplash.bmp") );

		// Init.
		GLogWindow = new WLog( Log.Filename, Log.LogAr, TEXT("EditorLog"), &Frame );
		GLogWindow->OpenWindow( ShowLog, 0 );
		GLogWindow->MoveWindow( 100, 100, 450, 450, 0 );

		// Init engine.
		GEditor = CastChecked<UEditorEngine>(InitEngine());
		GhwndEditorFrame = GEditorFrame->hWnd;

		// Set up autosave timer.  We ping the engine once a minute and it determines when and 
		// how to do the autosave.
		SetTimer( GEditorFrame->hWnd, 900, 60000, NULL);

		// Initialize "last dir" array
		GLastDir[eLASTDIR_UNR] = TEXT("..\\maps");
		GLastDir[eLASTDIR_UTX] = TEXT("..\\textures");
		GLastDir[eLASTDIR_UAX] = TEXT("..\\sounds");

		if( !GConfig->GetString( TEXT("Directories"), TEXT("PCX"), GLastDir[eLASTDIR_PCX], TEXT("UnrealEd.ini") ) )		GLastDir[eLASTDIR_PCX] = TEXT("..\\textures");
		if( !GConfig->GetString( TEXT("Directories"), TEXT("WAV"), GLastDir[eLASTDIR_WAV], TEXT("UnrealEd.ini") ) )		GLastDir[eLASTDIR_WAV] = TEXT("..\\sounds");
		if( !GConfig->GetString( TEXT("Directories"), TEXT("BRUSH"), GLastDir[eLASTDIR_BRUSH], TEXT("UnrealEd.ini") ) )		GLastDir[eLASTDIR_BRUSH] = TEXT("..\\maps");
		if( !GConfig->GetString( TEXT("Directories"), TEXT("2DS"), GLastDir[eLASTDIR_2DS], TEXT("UnrealEd.ini") ) )		GLastDir[eLASTDIR_2DS] = TEXT("..\\maps");

		if( !GConfig->GetString( TEXT("URL"), TEXT("MapExt"), GMapExt, TEXT("UnrealTournament.ini") ) )		GMapExt = TEXT("unr");
		GEditor->Exec( *(FString::Printf(TEXT("MODE MAPEXT=%s"), *GMapExt ) ) );

		// Init input.
		UInput::StaticInitInput();

		// Toolbar.
		GButtonBar = new WButtonBar( TEXT("EditorToolbar"), &Frame.LeftFrame );
		GButtonBar->OpenWindow();
		Frame.LeftFrame.Dock( GButtonBar );
		Frame.LeftFrame.OnSize( SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE, 0, 0 );

		GBottomBar = new WBottomBar( TEXT("BottomBar"), &Frame.BottomFrame );
		GBottomBar->OpenWindow();
		Frame.BottomFrame.Dock( GBottomBar );
		Frame.BottomFrame.OnSize( SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE, 0, 0 );

		GTopBar = new WTopBar( TEXT("TopBar"), &Frame.TopFrame );
		GTopBar->OpenWindow();
		Frame.TopFrame.Dock( GTopBar );
		Frame.TopFrame.OnSize( SWP_FRAMECHANGED | SWP_NOMOVE | SWP_NOSIZE, 0, 0 );

		GBrowserMaster = new WBrowserMaster( TEXT("Master Browser"), GEditorFrame );
		GBrowserMaster->OpenWindow( 0 );
		GBrowserMaster->Browsers[eBROWSER_MESH] = (WBrowser**)(&GBrowserMesh);
		GBrowserMaster->Browsers[eBROWSER_MUSIC] = (WBrowser**)(&GBrowserMusic);
		GBrowserMaster->Browsers[eBROWSER_SOUND] = (WBrowser**)(&GBrowserSound);
		GBrowserMaster->Browsers[eBROWSER_ACTOR] = (WBrowser**)(&GBrowserActor);
		GBrowserMaster->Browsers[eBROWSER_GROUP] = (WBrowser**)(&GBrowserGroup);
		GBrowserMaster->Browsers[eBROWSER_TEXTURE] = (WBrowser**)(&GBrowserTexture);
		::InvalidateRect( GBrowserMaster->hWnd, NULL, 1 );
		
		GMatineeSheet = new WMatineeSheet( TEXT("Matinee Controls"), GEditorFrame );
		GMatineeSheet->OpenWindow();
		GMatineeSheet->Show( FALSE );


		
		// Open a blank level on startup.
		Frame.OpenLevelView();

		// Reopen whichever windows we need to.
		UBOOL bDocked, bActive;

		if(!GConfig->GetInt( TEXT("Mesh Browser"), TEXT("Docked"), bDocked, TEXT("UnrealEd.ini") ))	bDocked = FALSE;
		SendMessageX( GEditorFrame->hWnd, WM_COMMAND, bDocked ? WM_BROWSER_DOCK : WM_BROWSER_UNDOCK, eBROWSER_MESH );
		if( !bDocked ) 
		{
			if(!GConfig->GetInt( *GBrowserMesh->PersistentName, TEXT("Active"), bActive, TEXT("UnrealEd.ini") ))	bActive = FALSE;
			GBrowserMesh->Show( bActive );
		}
		
		if(!GConfig->GetInt( TEXT("Music Browser"), TEXT("Docked"), bDocked, TEXT("UnrealEd.ini") ))	bDocked = FALSE;
		SendMessageX( GEditorFrame->hWnd, WM_COMMAND, bDocked ? WM_BROWSER_DOCK : WM_BROWSER_UNDOCK, eBROWSER_MUSIC );
		if( !bDocked ) 
		{
			if(!GConfig->GetInt( *GBrowserMusic->PersistentName, TEXT("Active"), bActive, TEXT("UnrealEd.ini") ))	bActive = FALSE;
			GBrowserMusic->Show( bActive );
		}

		if(!GConfig->GetInt( TEXT("Sound Browser"), TEXT("Docked"), bDocked, TEXT("UnrealEd.ini") ))	bDocked = FALSE;
		SendMessageX( GEditorFrame->hWnd, WM_COMMAND, bDocked ? WM_BROWSER_DOCK : WM_BROWSER_UNDOCK, eBROWSER_SOUND );
		if( !bDocked ) 
		{
			if(!GConfig->GetInt( *GBrowserSound->PersistentName, TEXT("Active"), bActive, TEXT("UnrealEd.ini") ))	bActive = FALSE;
			GBrowserSound->Show( bActive );
		}

		if(!GConfig->GetInt( TEXT("Actor Browser"), TEXT("Docked"), bDocked, TEXT("UnrealEd.ini") ))	bDocked = FALSE;
		SendMessageX( GEditorFrame->hWnd, WM_COMMAND, bDocked ? WM_BROWSER_DOCK : WM_BROWSER_UNDOCK, eBROWSER_ACTOR );
		if( !bDocked ) 
		{
			if(!GConfig->GetInt( *GBrowserActor->PersistentName, TEXT("Active"), bActive, TEXT("UnrealEd.ini") ))	bActive = FALSE;
			GBrowserActor->Show( bActive );
		}

		if(!GConfig->GetInt( TEXT("Group Browser"), TEXT("Docked"), bDocked, TEXT("UnrealEd.ini") ))	bDocked = FALSE;
		SendMessageX( GEditorFrame->hWnd, WM_COMMAND, bDocked ? WM_BROWSER_DOCK : WM_BROWSER_UNDOCK, eBROWSER_GROUP );
		if( !bDocked ) 
		{
			if(!GConfig->GetInt( *GBrowserGroup->PersistentName, TEXT("Active"), bActive, TEXT("UnrealEd.ini") ))	bActive = FALSE;
			GBrowserGroup->Show( bActive );
		}

		if(!GConfig->GetInt( TEXT("Texture Browser"), TEXT("Docked"), bDocked, TEXT("UnrealEd.ini") ))	bDocked = FALSE;
		SendMessageX( GEditorFrame->hWnd, WM_COMMAND, bDocked ? WM_BROWSER_DOCK : WM_BROWSER_UNDOCK, eBROWSER_TEXTURE );
		if( !bDocked ) 
		{
			if(!GConfig->GetInt( *GBrowserTexture->PersistentName, TEXT("Active"), bActive, TEXT("UnrealEd.ini") ))	bActive = FALSE;
			GBrowserTexture->Show( bActive );
		}
		
		if(!GConfig->GetInt( TEXT("CodeFrame"), TEXT("Active"), bActive, TEXT("UnrealEd.ini") ))	bActive = FALSE;
		if( bActive )	ShowCodeFrame( GEditorFrame );

		GCodeFrame = new WCodeFrame( TEXT("CodeFrame"), GEditorFrame );
		GCodeFrame->OpenWindow( 0, 0 );

		GMainMenu = LoadMenuIdX( hInstance, IDMENU_MainMenu );
		SetMenu( GEditorFrame->hWnd, GMainMenu );

		GMRUList = new MRUList( TEXT("MRU") );
		GMRUList->ReadINI();
		GMRUList->AddToMenu( GEditorFrame->hWnd, GMainMenu, 1 );

		ExitSplash();

		if( !GIsRequestingExit )
			MainLoop( GEditor );

		GDocumentManager=NULL;
		GFileManager->Delete(TEXT("Running.ini"),0,0);
		if( GLogWindow )
			delete GLogWindow;
		appPreExit();
		GIsGuarded = 0;
		delete GMRUList;
		::DestroyWindow( GCodeFrame->hWnd );
		delete GCodeFrame;
#ifndef _DEBUG
	}
	catch( ... )
	{
		// Crashed.
		Error.HandleError();
	}
#endif

	// Shut down.
	appExit();
	GIsStarted = 0;
	return 0;
}


//MILESTONE 3
// updated milestone 4
bool CALLBACK TweenDialogProc( HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam )

{
	static float* tweenNum;
	static HWND eHdc;
	static HWND textHdc;
	TCHAR textBuffer[30];
	float divdown;
#define MAXTWEENTIME 10
	switch (uMsg)
	{

		case WM_INITDIALOG:
			
			eHdc=GetDlgItem(hWnd,IDC_SLIDER1);
			textHdc=GetDlgItem(hWnd,IDC_POS);
			tweenNum=(float*)lParam;
			SendMessage(eHdc,TBM_SETRANGE,(WPARAM)true,(LPARAM)MAKELONG(0,MAXTWEENTIME));
			SendMessage(eHdc,TBM_SETPOS,(WPARAM)true,(LPARAM)MAXTWEENTIME-*tweenNum);
			if(*tweenNum>0)
				divdown=*tweenNum/3.0;
			else
				divdown=0.0;
			swprintf(textBuffer,TEXT("%f"),divdown);
			SetWindowText(textHdc,textBuffer);
			return true;

		case WM_NOTIFY:
			*tweenNum=MAXTWEENTIME-(float)SendMessage(eHdc,TBM_GETPOS,0,0);
			
			if(*tweenNum>0)
				divdown=*tweenNum/3.0;
			else
				divdown=0.0;
			swprintf(textBuffer,TEXT("%f"),divdown);
			SetWindowText(textHdc,textBuffer);
			return true;

		case WM_COMMAND:
			switch LOWORD(wParam)
			{
			case IDOK:
				*tweenNum=MAXTWEENTIME-(float)SendMessage(eHdc,TBM_GETPOS,0,0);
				 EndDialog(hWnd,NULL);
				return TRUE;

			}
			break;

		case WM_CLOSE:
		case WM_DESTROY:

				EndDialog(hWnd,NULL);
				return TRUE;

	}

	return FALSE;

}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
