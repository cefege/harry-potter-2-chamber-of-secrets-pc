// ---------------------------------------------------------------------------------
//           _                    _   _ _ _ _   _              _     
//          (_)                  | | (_) (_) | (_)            | |    
// __      ___ _ __         _   _| |_ _| |_| |_ _  ___ ___    | |__  
// \ \ /\ / / | '_ \       | | | | __| | | | __| |/ _ \ __|   | '_ \ 
//  \ V  V /| | | | |      | |_| | |_| | | | |_| |  __/__ \ _ | | | |
//   \_/\_/ |_|_| |_|       \__,_|\__|_|_|_|\__|_|\___|___/(_)|_| |_|
//                   ______                                          
//                  |______|                                         
//
// ---------------------------------------------------------------------------------
// Created on  : 07/11/2001
// Authored by : Elijah Emerson
//
// Dependencies: "types.h"
// 
// Description : Win utilities abstracts "windows.h" specific functions from the
//				 the user so you don't have to include "windows.h". Common utility
//				 functions are included as well.
//
// ---------------------------------------------------------------------------------

#pragma once
#ifndef	_WIN_UTILITIES_H_
#define _WIN_UTILITIES_H_

// Added utilities
const char* Util_GetBaseDir			( HINSTANCE hInstance );
const char* Util_GetPersonalDir		( void );
const char* Util_GetControlPanelDir ( void );
BOOL		Util_CreateDirectoy		( char *strDir );
INT			Util_FileSize			( const char* Filename );
BOOL		Util_FileDelete			( const char* Filename, bool RequireExists=0, bool EvenReadOnly=0 );

bool		Util_IsOSVer2kOrXP		( void );

//****************************
//*** Forward declarations ***
//****************************

#ifndef _FILE_DEFINED
struct _iobuf {
        char *_ptr;
        int   _cnt;
        char *_base;
        int   _flag;
        int   _file;
        int   _charbuf;
        int   _bufsiz;
        char *_tmpfname;
        };
typedef struct _iobuf FILE;
#define _FILE_DEFINED
#endif

struct tagRECT;


//*************************************************
//************** ERROR UTILITIES ******************
//*************************************************

// *** Error Settings ***
#define BREAK_ON_ERROR		1	//do you want to break on an error?
#define ERROR_LOG_FILENAME	("error.log")


class CErrorManager
{
private:
	char	m_strErrorMsg[512];		// message from the user
	char	m_strFileName[256];		// file name where the first error took place
	char	m_strFuncName[128];		// function name string
	u32		m_iLine;				// line number in code where the error occured
	bool	m_bErrorHit;			// did we hit an error yet?
	
	void	Init			( void );
			CErrorManager	( void ) { Init(); }
public:
			~CErrorManager	( void ) { }
	static	CErrorManager&	
			GetInstance		( void ) { static CErrorManager Obj; return Obj; }
	void	HandleMessage	( const char* str, ...);
	void	HandleError		( const char* strFileName, u32 iLine, const char* strFuncName);
};

//*** Error Handeling utility functions***
void	DisplayLastWinError	( void );
void	DebugOutput			( char *str, ...);
void	ErrorMessage		( char *str, ...);
void	HRErrorMessage		( HRESULT hr, char *str, ...);
void	DXErrorMessage		( HRESULT hr, char *str, ...);

// *** Error Macros 
#define MANUAL_BREAKPOINT	_asm { int 3 }

// ErrorLog
#define ErrorLog( _str )								{	CErrorManager::GetInstance().HandleMessage( (_str) );									CErrorManager::GetInstance().HandleError(__FILE__, __LINE__, NULL); } //__FUNCTION__); }
#define ErrorLog1( _str, a1 )							{	CErrorManager::GetInstance().HandleMessage( (_str), a1 );								CErrorManager::GetInstance().HandleError(__FILE__, __LINE__, NULL); } //__FUNCTION__); }
#define ErrorLog2( _str, a1, a2 )						{	CErrorManager::GetInstance().HandleMessage( (_str), a1, a2 );							CErrorManager::GetInstance().HandleError(__FILE__, __LINE__, NULL); } //__FUNCTION__); }
#define ErrorLog3( _str, a1, a2, a3 )					{	CErrorManager::GetInstance().HandleMessage( (_str), a1, a2, a3 );						CErrorManager::GetInstance().HandleError(__FILE__, __LINE__, NULL); } //__FUNCTION__); }
#define ErrorLog4( _str, a1, a2, a3, a4 )				{	CErrorManager::GetInstance().HandleMessage( (_str), a1, a2, a3, a4 );					CErrorManager::GetInstance().HandleError(__FILE__, __LINE__, NULL); } //__FUNCTION__); }
#define ErrorLog5( _str, a1, a2, a3, a4, a5 )			{	CErrorManager::GetInstance().HandleMessage( (_str), a1, a2, a3, a4, a5 );				CErrorManager::GetInstance().HandleError(__FILE__, __LINE__, NULL); } //__FUNCTION__); }
#define ErrorLog6( _str, a1, a2, a3, a4, a5, a6 )		{	CErrorManager::GetInstance().HandleMessage( (_str), a1, a2, a3, a4, a5, a6 );			CErrorManager::GetInstance().HandleError(__FILE__, __LINE__, NULL); } //__FUNCTION__); }
#define ErrorLog7( _str, a1, a2, a3, a4, a5, a6, a7 )	{	CErrorManager::GetInstance().HandleMessage( (_str), a1, a2, a3, a4, a5, a6, a7 );		CErrorManager::GetInstance().HandleError(__FILE__, __LINE__, NULL); } //__FUNCTION__); }
#define ErrorLog8( _str, a1, a2, a3, a4, a5, a6, a7, a8){	CErrorManager::GetInstance().HandleMessage( (_str), a1, a2, a3, a4, a5, a6, a7, a8 );	CErrorManager::GetInstance().HandleError(__FILE__, __LINE__, NULL); } //__FUNCTION__); }

#define IF_FAIL_BREAK( _hr )						{ if( FAILED(_hr) ) break; }
#define IF_FAIL_MSG( _hr, _msg )					{ if( FAILED(_hr) ) { ErrorLog( (_msg) ); } }
#define IF_FAIL_MSG_AND_BREAK( _hr, _msg )			{ if( FAILED(_hr) ) { ErrorLog( (_msg) ); break; } }
#define IF_NULL_SET_HR_MSG_AND_BREAK(_ptr,_hr,_msg)	{ if( NULL==(_ptr) ) { _hr = E_FAIL; ErrorLog( (_msg) ); break; } }

// *** HRESULT Functions and macros
char*	EEGetErrorString( HRESULT er );
	
//Displays a message box, and passes the error code to EEGetErrorString.
HRESULT EETrace			( char *strFile, DWORD dwLine, HRESULT er, char* strMsg, bool bPopMsgBox );

//Error Handling Macros
#define	EETRACE_MSG(str)			EETrace( __FILE__, (DWORD)__LINE__, 0, str, FALSE)
#define	EETRACE_ERR(str,er)			EETrace( __FILE__, (DWORD)__LINE__,er, str, TRUE )
#define	EETRACE_ERR_NOMSGBOX(str,er)EETrace( __FILE__, (DWORD)__LINE__,er, str, FALSE)




//***************************************************
//************** WINDOWS UTILITIES ******************
//***************************************************
// 

void		Util_PostQuitMessage	( void );
void		Util_TimeGetTime		( void );
void		Util_GetTimeAndDate		( char *output, u32 size );

void		Util_SetWindow			( HWND hWnd, f32 fXPosRatio, f32 fYPosRatio, u32 iClientWidth, u32 iClientHeight );

long		Util_DefWindowProc		( HWND hWnd, u32 iMsg, u32 wParam, long lParam);
long		Util_SetWindowLong		( HWND hWnd, int nIndex, long dwNewLong );
int			Util_SetWindowPos		( HWND hWnd, u32 hWndInsertAfter, 
									  int x, int y, int cx, int cy, u32 uFlags);

int			Util_SetCursorPos		( int x, int y);
void		Util_CenterCursorPos	( void );

int			Util_GetWindowRect		( HWND hWnd, RECT* pRect);
int			Util_GetClientRect		( HWND hWnd, RECT* pRect);
int			Util_OrientWindow		( HWND hWnd, float fScreenPercentX, float fScreenPercentY );

u32			Util_GetActiveWindow	( void );	//returns an HWND as a u32
void		Util_TextOut			( u32 hDC, int x, int y, const char* str, int iStrLen );
void		Util_ReplaceCharInString( char* str, char c, char n );
char*		Util_StrTok				( char* strToken, char** pNext );
void		Util_StrTabify			( char* strBuffer, u32 iNumSpaces );


//**************************************************
//************** STRING UTILITIES ******************
//**************************************************

HRESULT	String_GetString( const char *input, char *result );

//*******************************************************
//************** FILE IN/OUT UTILITIES ******************
//*******************************************************


#define MAX_NUM_PATHS	64
#define MAXCHAR_PATH	512

struct SFileInfo
{
	char	m_strFileName[MAXCHAR_PATH];
	char	m_strPath[MAXCHAR_PATH];
	char	m_strPathAndName[MAXCHAR_PATH];
	void	Clear() { m_strFileName[0] = 0; m_strPath[0] = 0; m_strPathAndName[0]; }
};


class CFileList
{
private:
	// *** Private Data Members ***
	// --- File and Directroy Data
	u32			m_iNumFiles;		//total number of files in list.
	u32			m_iNumDirectories;	//total number of directorys in list.
	char**		m_pFileList;		//2D array because we need a "single" array of character strings
	char**		m_pDirList;			//2D array because we need a "single" array of character strings
	
	u32			m_iNumPaths;
	char		m_strPathList[MAX_NUM_PATHS][MAXCHAR_PATH];
	
	SFileInfo*	m_pFileInfoList;	//A malloced array of FileInfo structs
	
	// *** Private Functions
	// --- Base Functions
	void		Clear				( void );
	HRESULT		CountFiles			( const char* strPath, const char* strDir, bool bRecursive );
	
public:
	// *** Public Functions
	// --- Base Functions
				CFileList			( void )	{ Clear();	}
				~CFileList			( void )	{ Destroy();}
	HRESULT		Destroy				( void );
	
	// --- Main Functions
	void		FindFiles			( const char* Filename, bool Files, bool Directories );

	// --- Get Functions
	const char* GetFileName			( u32 i ) const;
	const char* GetPath				( u32 i ) const;
	const char* GetPathAndFileName	( u32 i ) const; 
	inline u32	GetNumFiles			( void )  const	{ return m_iNumFiles;		}
	inline u32	GetNumDirectories	( void )  const	{ return m_iNumDirectories; }
};

//*** File IN/OUT utility functions ***
void	LogClear					( const char* strFilename );
void	LogEntry					( const char* strFilename, const char* str, ...);

HRESULT String_GetString			( const char* strInput, char *strOutput );
HRESULT	String_GetTitleFromFilename	( const char* strInput, char *strOutput );
HRESULT String_AddPathToFilename	( const char* strPath, const char *strFilename, char *strOutput );

HRESULT	LoopUntilString				(FILE *fp, const char *str);
HRESULT	File_GetString				(FILE *fp, char *result);
HRESULT	File_GetString_NoQuotes		(FILE *fp, char *result);
HRESULT	File_GetInt					(FILE *fp, s32  *result);
HRESULT	File_GetInt					(FILE *fp, u32  *result);
HRESULT	File_GetFloat				(FILE *fp, f32  *result);
HRESULT	File_GetDouble				(FILE *fp, f64  *result);
//HRESULT		File_GetScalar		(FILE *fp, scalar *result);

HRESULT	File_GetIntAfterString		(FILE *fp, const char *string, s32 *result);
HRESULT	File_GetIntAfterString		(FILE *fp, const char *string, u32 *result);
HRESULT	File_GetFloatAfterString	(FILE *fp, const char *string, f32 *result);
HRESULT	File_GetStringAfterString	(FILE *fp, const char *string, char*result);



#endif