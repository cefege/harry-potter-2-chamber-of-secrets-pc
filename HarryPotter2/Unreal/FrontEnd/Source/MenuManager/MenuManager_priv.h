// -----------------------------------------------------------------------------
//  __  __                  __  __                                                     _           _     
// |  \/  |                |  \/  |                                                   (_)         | |    
// | \  / | ___ _ __  _   _| \  / | __ _ _ __   __ _  __ _  ___ _ __        _ __  _ __ ___   __   | |__  
// | |\/| |/ _ \ '_ \| | | | |\/| |/ _` | '_ \ / _` |/ _` |/ _ \ '__|      | '_ \| '__| \ \ / /   | '_ \ 
// | |  | |  __/ | | | |_| | |  | | (_| | | | | (_| | (_| |  __/ |         | |_) | |  | |\ V /  _ | | | |
// |_|  |_|\___|_| |_|\__,_|_|  |_|\__,_|_| |_|\__,_|\__, |\___|_|         | .__/|_|  |_| \_/  (_)|_| |_|
//                                                    __/ |          ______| |                           
//                                                   |___/          |______|_|                           
//
// -----------------------------------------------------------------------------
// Originally created on 07/25/2002 by Elijah Emerson
//
// Copyright 2002, Amaze Entertainment, all rights reserved.
// -----------------------------------------------------------------------------

#ifndef	_H_MENU_MANAGER_PRIV_
#define _H_MENU_MANAGER_PRIV_


// ***************
// *** DEFINES ***
// ***************
#define MAX_NAME_LEN	256

#define VECTOR(_c)		std::vector<(_c)>
#define STACK(_c)		std::stack<(_c)>


// ---------------------------------------------------------------------------------
//	CLASS NAME:		CMenu
//	AUTHOR(S):		Elijah Emerson
//	CREATION DATE:	07/21/2002
//
//	DESCRIPTION:	CMenu is a single menu used by the menu manager
// ---------------------------------------------------------------------------------
class CMenu : public IMenu
{
private:
	char				m_strName[MAX_NAME_LEN];	// Menu name
	HWND				m_hWnd;						// handle to main window (used by main menu)
	
	bool				m_bUsePopTimer;				// Time menu has been alive
	float				m_fPopTimer;				// Time

	// *** Private Functions ***
	// --- Base Functions
						CMenu				( void ) { Clear();		}
						~CMenu				( void ) { Destroy();	}
	void				Clear				( void );
	HRESULT				Init				( const char* strName, const HWND hWndMenu );
	void				Destroy				( void );
	bool				Update				( float fTimeDelta );
	
public:
	// *** Interface Functions ***
	// --- Base Functions
	virtual	HRESULT		Release				( void );
	
	// --- Set Functions
	virtual void		SetPopTimer			( f32 fWaitTime );
	virtual void		SetRelativePosition	( f32 fXPercent, f32 fYPercent );

	// --- Get Functions
	virtual HWND		GetHandle			( void ) const { return m_hWnd; }

	// *** Friends
	friend class CMenuManager;
};

//**********************************************************************************
//	CLASS NAME:		CMenuManager	
//	AUTHOR(S):		Elijah Emerson
//	CREATION DATE:	7/12/2001
//
//	DESCRIPTION:	The Menu manager is used to manage all menus
//**********************************************************************************
class CMenuManager : public IMenuManager
{
private:
	// *** Private Data ***
	// --- Base Data
	HINSTANCE			m_hInstance;		// handle to module instance
	HWND				m_hWnd;				// handle to main window (used by main menu)
	
	std::stack<CMenu*>	m_vMenuStack;		// vector of Menus
	CMenu*				m_pLastMenu;		// interface to current menu
	
	// --- State Data
	bool				m_bActive;			// is our menu manger active? (in focus)
	bool				m_bMinimized;		// is our menu manager minimized
	bool				m_bUpdateShown;		// does our top menu need shown?

	float				m_fPopWaitTime;		// if > 0 then wait till you pop the current menu
	
	// *** Private Functions ***
	// --- Base Functions
						CMenuManager		( void ) { Clear();   }
	virtual				~CMenuManager		( void ) { Destroy(); }
	void				Clear				( void );
	HRESULT				Init				( const HINSTANCE hInstance );
	void				Destroy				( void );
	
public:
	// *** Interface Implementation ***
	// --- Base Functions
	virtual HRESULT		Release				( void );
	virtual void		Update				( void );
	
	// --- Menu Management
	virtual IMenu*		PushMenu			( const char* strName, const LPCTSTR lpTemplate, const DLGPROC dlgProc );
	virtual IMenu*		PopMenu				( void );
	
	// --- Query Functions
	virtual bool		IsActive			( void ) const { return m_bActive;		}
	virtual bool		IsMinimized			( void ) const { return m_bMinimized;	}
	
	virtual u32			GetNumMenus			( void ) { return m_vMenuStack.size(); }
	virtual HWND		GetLastMenuHandle	( void ) { return ( m_pLastMenu )		  ? m_pLastMenu->GetHandle() : NULL; }
	virtual HWND		GetCurrentMenuHandle( void ) { return ( m_vMenuStack.size() ) ? m_vMenuStack.top()->GetHandle() : NULL; }
	
	// *** Friends ***
	// --- Functions
	friend IMenuManager* CreateMenuManager	( HINSTANCE hInstance );

	// --- Classes
	friend class CMenu;
};





#endif // _H_MENU_MANAGER_PRIV_
// --------------------------------------------------------------------------------
// FRONTEND.h - End of file
// --------------------------------------------------------------------------------