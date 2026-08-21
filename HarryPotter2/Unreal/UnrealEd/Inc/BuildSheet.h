#include <windows.h>
#define _WIN32_IE 0x0200
#include <commctrl.h>

enum eBS {
	eBS_OPTIONS		= 0,
	eBS_STATS		= 1,
	eBS_MAX			= 2
};

class TBuildSheet
{
public:

	TBuildSheet();
	~TBuildSheet();

	void OpenWindow( HINSTANCE hInst, HWND hWndOwner );
	void Show( BOOL bShow );
	void GetDataFromSurfs(void);
	void Build();
	void RefreshStats();

	PROPSHEETPAGE m_pages[eBS_MAX];
    PROPSHEETHEADER m_psh;
	HWND m_hwndSheet;
	BOOL m_bShow;
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
