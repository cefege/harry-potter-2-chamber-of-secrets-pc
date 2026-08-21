// -----------------------------------------------------------------------------
//           _                    _   _ _ _ _   _                               
//          (_)                  | | (_) (_) | (_)                              
// __      ___ _ __         _   _| |_ _| |_| |_ _  ___ ___      ___ _ __  _ __  
// \ \ /\ / / | '_ \       | | | | __| | | | __| |/ _ \ __|    / __| '_ \| '_ \ 
//  \ V  V /| | | | |      | |_| | |_| | | | |_| |  __/__ \ _ | (__| |_) | |_) |
//   \_/\_/ |_|_| |_|       \__,_|\__|_|_|_|\__|_|\___|___/(_) \___| .__/| .__/ 
//                   ______                                        | |   | |    
//                  |______|                                       |_|   |_|    
//
// -----------------------------------------------------------------------------
// Created on  : 07/11/2001
// Authored by : Elijah Emerson
//
// Description : For all your windows specific needs, for the most common windows
//				 functions include win_utilities.h, instead of that slug windows.h 
//
// -----------------------------------------------------------------------------


//****************
//*** INCLUDES ***
//****************
#define WIN32_LEAN_AND_MEAN
#include <windows.h>			//for window's function wrappers and the WIN32_FIND_DATA definition
#include <mmsystem.h>			//for file search functions
#include <stdio.h>				//for sprintf
#include <stdlib.h>

#include <shlobj.h>				//for getting the user folder

#include <string>
using namespace std;

//#include ".\mmgr.h"
#include ".\types.h"			//standard type definitions
#include ".\win_utilities.h"	//for CFileList definition


#define STR_BUFFER_SIZE		512

//----------------------------------------------------------------------------------
//	FUNCTION NAME:	CErrorManager::Init
//	AUTHOR(S):      Elijah Emerson
//	CREATION DATE:	07/11/2001
//	
//	RETURN TYPE:	void		- 
//	DESCRIPTION:	Init our Error Manager's data members
//----------------------------------------------------------------------------------
void CErrorManager::Init( void )
{
	m_iLine		= 0;
	m_bErrorHit = false;

	//Clear the error log
	LogClear( ERROR_LOG_FILENAME );
}

//----------------------------------------------------------------------------------
//	FUNCTION NAME:	CErrorManager::HandleMessage
//	AUTHOR(S):      Elijah Emerson
//	CREATION DATE:	07/11/2001
//	
//	ARGUMENT 1:     const char* str		- string handled like printf()
//	ARGUMENT 2:     ...					- argument array handled like printf()
//	RETURN TYPE:	void				- 
//	DESCRIPTION:	log our error message into our "error.log" file.
//----------------------------------------------------------------------------------
void CErrorManager::HandleMessage(const char* str, ...)
{
	if( (str == 0) || (str[0] == 0) )		//Check to make sure we have a string
		return;
	va_list	pArgList;						//Pointer to our list of arguments
	va_start(pArgList, str);				//Start our list of agruments
	vsprintf(m_strErrorMsg, str, pArgList);	//Put our formated string into the buffer and get the size
	va_end(pArgList);						//End our argument list
	
	if(!m_bErrorHit)
	{
		char strBuffer[64];
		//This is our first error so lets log time and date.
		Util_GetTimeAndDate(strBuffer, 64);
		LogEntry(ERROR_LOG_FILENAME, "\n-= eengine error log =-  %s\n", strBuffer);
	}

	LogEntry(ERROR_LOG_FILENAME, "%s\n", m_strErrorMsg);
}


//----------------------------------------------------------------------------------
//	FUNCTION NAME:	CErrorManager::HandleError
//	AUTHOR(S):      Elijah Emerson
//	CREATION DATE:	07/11/2001
//	
//	ARGUMENT 1:     const char* strFileName		- filename of source code that had the error
//	ARGUMENT 2:     u32 iLine					- line in the file that had the error
//	ARGUMENT 3:     const char* strFuncName		- function name that had the error (usually null)
//	RETURN TYPE:	void						- 
//	DESCRIPTION:	output our error as a message box displaying the file that 
//					had the error as well as the line of code and *hopefully* the function name
//----------------------------------------------------------------------------------
void CErrorManager::HandleError( const char* strFileName, u32 iLine, const char* strFuncName)
{
	char	strBuffer[STR_BUFFER_SIZE];

	//Is this our first time hitting an error?
	if( m_bErrorHit )
	{
		//copy our main information into the string buffer
		sprintf(strBuffer, "File:\t%s\nLine:\t%d\nFunc:\t%s\nMsg:\t%s\n",
			(strrchr(strFileName,'\\'))+1,	// file
			iLine,							// line
			strFuncName,					// function name
			m_strErrorMsg);					// error message

		//log error
		LogEntry(ERROR_LOG_FILENAME, strBuffer);
	}
	else
	{
		//Set the ErrorHit boolean
		m_bErrorHit = true;
		
		// Set our error information

		//(strrchr(m_strFilename,'\\'))+1
		if(strFileName && strFileName[0])
			strcpy(m_strFileName, strFileName);
		else
			strcpy(m_strFileName, "NONE");

		if(strFuncName && strFuncName[0])
			strcpy(m_strFuncName, strFuncName);
		else
			strcpy(m_strFuncName, "NONE");

		m_iLine = iLine;
		
		
		//This is our first time calling the HandleError function.
		// First open an error log file and put the details into that text file.

		// so lets save our error message untill we have a completed history.
		// pChar = strrchr(strFile, '\\'); //find last occurence
		char strMsg[512];
		sprintf(strMsg, 
			"File:\t%s\nLine:\t%d\nFunc:\t%s\nMsg:\t%s\n",
			m_strFileName,		// file
			m_iLine,			// line
			m_strFuncName,		// function name
			m_strErrorMsg);		// error message

		//sprintf(strHistory, "%s", strLocation);

#if _DEBUG
#if BREAK_ON_ERROR
		//strcat(strBuffer, strHistory);
		strcpy(strBuffer, "\n********************************************************\n");
		strcat(strBuffer, strMsg);
		strcat(strBuffer, "\n Do you wish to break into the code?");
		if( IDYES == MessageBox(NULL, strBuffer, "Eli's CRAZY Error log (Debug:1, BOE: 1)", MB_YESNO|MB_ICONEXCLAMATION) )
		{
			// If you got here because you want to debug, you will need to go
			// back on the callstack.
			MANUAL_BREAKPOINT;
		}
#endif //end BREAK_ON_ERROR
#endif //end DEBUG

		//log error
		LogEntry(ERROR_LOG_FILENAME, strBuffer);
	}
}


//----------------------------------------------------------------------------------
//	FUNCTION NAME:	DebugOutput
//	AUTHOR(S):      Elijah Emerson
//	CREATION DATE:	05/01/2001
//	
//	ARGUMENT 1:     char *str		- message displayed inside our DebugWindow in vc.
//	ARGUMENT 2:     ...				- handled like printf()
//	RETURN TYPE:	void			- 
//	DESCRIPTION:	Wrapper for OutputDebugString function
//----------------------------------------------------------------------------------
void DebugOutput(char *str, ...)
{
	va_list	pArgList;						//Pointer to our list of arguments
	char	buffer[STR_BUFFER_SIZE];		//Temporary buffer to hold the text

	if((str == 0)||(str == '\0'))			//Check to make sure we have a string
		return;

	va_start(pArgList, str);				//Start our list of agruments
	vsprintf(buffer, str, pArgList);		//Put our formated string into the buffer and get the size
	va_end(pArgList);						//End our argument list

	//Now that we formated our string pass it into our debug window.
	OutputDebugString(buffer);
}

//----------------------------------------------------------------------------------
//	FUNCTION NAME:	ErrorMessage
//	AUTHOR(S):      Elijah Emerson
//	CREATION DATE:	05/01/2001
//	
//	ARGUMENT 1:     char *str		- message displayed inside our DebugWindow in vc.
//	ARGUMENT 2:     ...				- handled like printf()
//	RETURN TYPE:	void			- 
//	DESCRIPTION:	Wrapper for MessageBox function with error logging added
//----------------------------------------------------------------------------------
void ErrorMessage(char *str, ...)
{
	va_list	pArgList;						//Pointer to our list of arguments
	char	buffer[STR_BUFFER_SIZE];		//Temporary buffer to hold the text
	
	if((str == 0)||(str == '\0'))			//Check to make sure we have a string
		return;
	
	va_start(pArgList, str);				//Start our list of agruments
	vsprintf(buffer, str, pArgList);		//Put our formated string into the buffer and get the size
	va_end(pArgList);						//End our argument list
	
	// log our error
	CErrorManager::GetInstance().HandleMessage( buffer );

	//Now that we formated our string pass it into our debug window.
	MessageBox(NULL, buffer, "Error Message", MB_OK);
}



//----------------------------------------------------------------------------------
//	FUNCTION NAME:	DisplayLastWinError
//	AUTHOR(S):      Elijah Emerson
//	CREATION DATE:	05/01/2001
//	
//	ARGUMENT 1:     char *str		- message displayed inside our DebugWindow in vc.
//	ARGUMENT 2:     ...				- handled like printf()
//	RETURN TYPE:	void			- 
//	DESCRIPTION:	Wrapper for displaying a standard windows error message
//----------------------------------------------------------------------------------
void DisplayLastWinError( void )
{
	LPVOID lpMsgBuf;
	
	FormatMessage( 
		FORMAT_MESSAGE_ALLOCATE_BUFFER | 
		FORMAT_MESSAGE_FROM_SYSTEM | 
		FORMAT_MESSAGE_IGNORE_INSERTS,
		NULL,
		GetLastError(),
		MAKELANGID(LANG_NEUTRAL, SUBLANG_DEFAULT), // Default language
		(LPTSTR) &lpMsgBuf,
		0,
		NULL 
		);
	
	// Process any inserts in lpMsgBuf.
	// ...
	// Display the string.
	MessageBox( NULL, (LPCTSTR)lpMsgBuf, "Win32 Error", MB_OK | MB_ICONINFORMATION );
	
	// Free the buffer.
	LocalFree( lpMsgBuf );
}

BOOL Util_CreateDirectoy( char *strDir )
{
	return CreateDirectory( strDir, NULL );
}

bool Util_IsOSVer2kOrXP	( void )
{
	DWORD dwPlatformId, dwMajorVersion, dwMinorVersion, dwBuildNumber;
#if UNICODE
	if( GUnicode && !GUnicodeOS )
	{
		OSVERSIONINFOA Version;
		Version.dwOSVersionInfoSize = sizeof(OSVERSIONINFOA);
		GetVersionExA(&Version);
		dwPlatformId   = Version.dwPlatformId;
		dwMajorVersion = Version.dwMajorVersion;
		dwMinorVersion = Version.dwMinorVersion;
		dwBuildNumber  = Version.dwBuildNumber;
	}
	else
#endif
	{
		OSVERSIONINFO Version;
		Version.dwOSVersionInfoSize = sizeof(OSVERSIONINFO);
		GetVersionEx(&Version);
		dwPlatformId   = Version.dwPlatformId;
		dwMajorVersion = Version.dwMajorVersion;
		dwMinorVersion = Version.dwMinorVersion;
		dwBuildNumber  = Version.dwBuildNumber;
	}

	if( dwPlatformId == VER_PLATFORM_WIN32_NT )
	{
		if ( dwMajorVersion >= 5 )
			return true;
	}

	return false;
}

// Get startup directory.
// Number of elements in an array.
#define ARRAY_COUNT( _a ) ( sizeof(_a) / sizeof((_a)[0]) )
const char* Util_GetBaseDir( HINSTANCE hInstance )
{
	static char Result[256]=TEXT("");
	if( !Result[0] )
	{
		// Get directory this executable was launched from.
		GetModuleFileName( hInstance, Result, ARRAY_COUNT(Result) );

		for( INT i = strlen(Result)-1; i>0; i-- )
			if( Result[i-1] == "\\"[0] || Result[i-1]=='/' )
				break;
		Result[i]=0;
	}
	return Result;
}

// --- Helper function that will get the "Personal Folder"
#define STDCALL		__stdcall				// Standard calling convention
FARPROC (STDCALL *pfnSHGetFolderPath)(HWND hwnd, int csidl, HANDLE hToken, DWORD dwFlags, char pszPath[]);
const char* Util_GetPersonalDir( void )
{
	static char Result[1024]=TEXT("");
	if( !Result[0] )
	{
		HMODULE hModSHFolder = LoadLibrary(TEXT("shfolder.dll"));
		if( hModSHFolder != NULL )
		{
			(*(FARPROC*)&pfnSHGetFolderPath = GetProcAddress(hModSHFolder, "SHGetFolderPathA"));
			if (pfnSHGetFolderPath != NULL )
			{
				// Get the personal folder
				pfnSHGetFolderPath(NULL, CSIDL_PERSONAL, NULL, 0, Result);
			}
		}
	}
	return Result;
}

const char* Util_GetControlPanelDir( void )
{
	static char Result[1024]=TEXT("");
	if( !Result[0] )
	{
		HMODULE hModSHFolder = LoadLibrary(TEXT("shfolder.dll"));
		if( hModSHFolder != NULL )
		{
			(*(FARPROC*)&pfnSHGetFolderPath = GetProcAddress(hModSHFolder, "SHGetFolderPathA"));
			if (pfnSHGetFolderPath != NULL )
			{
				// Get the personal folder
				pfnSHGetFolderPath(NULL, CSIDL_CONTROLS, NULL, 0, Result);
			}
		}
	}
	return Result;
}



//----------------------------------------------------------------------------------
//	FUNCTION NAME:	Util_GetWindowRect
//	AUTHOR(S):      Elijah Emerson
//	CREATION DATE:	04/25/2001
//	
//	ARGUMENT 1:     u32   hWnd		- handle to our window
//	ARGUMENT 2:     RECT* pRect		- pointer to a RECT structure
//	RETURN TYPE:	int				- returns true or false, 0 or 1 where true == success
//	DESCRIPTION:	Wrapper for GetWindowRect()
//----------------------------------------------------------------------------------
int Util_GetWindowRect(HWND hWnd, RECT* pRect)
{
	return GetWindowRect((HWND__ *)hWnd, (tagRECT *)pRect);
}

int Util_GetClientRect(HWND hWnd, RECT* pRect)
{
	return GetClientRect(hWnd, pRect );
}

int Util_OrientWindow( HWND hWnd, float fScreenPercentX, float fScreenPercentY )
{
	int		X,Y,W,H;
	RECT	rc;
	
	if( fScreenPercentX > 1.0f ) fScreenPercentX = 1.0f;
	else if( fScreenPercentX < 0.0f ) fScreenPercentX = 0.0f;
	
	if( fScreenPercentY > 1.0f ) fScreenPercentY = 1.0f;
	else if( fScreenPercentY < 0.0f ) fScreenPercentY = 0.0f;
	
	// Get the width and height
	GetWindowRect( hWnd, &rc );
	W = rc.right-rc.left;
	H = rc.bottom-rc.top;
	
	// Get our new X and Y position
	SystemParametersInfo( SPI_GETWORKAREA, 0, (LPVOID) &rc, 0);
	X = static_cast<int>( (rc.right  - rc.left - W ) * fScreenPercentX );
	Y = static_cast<int>( (rc.bottom - rc.top  - H ) * fScreenPercentY );
	
	// Set our window's new position
	return MoveWindow( hWnd, X, Y, W, H, true );
}

/*
void Util_SetClientRect(RECT* pRect, bool bMenu )
{
	AdjustWindowRect(pRect, , true );
}
*/
void Util_SetWindow( HWND hWnd, f32 fXPosRatio, f32 fYPosRatio, u32 iClientWidth, u32 iClientHeight )
{
	u32		iPosX, iPosY, iRealWidth, iRealHeight;
	RECT	rcDesiredClient = { 0, 0, iClientWidth, iClientHeight };
	RECT	rcScreen;

	// Adjust our window rect for the desired client window.
	AdjustWindowRect( &rcDesiredClient, GetWindowLong(hWnd, GWL_STYLE), false );
	
	// AdjustWindowRect will tell us the real width and height of our window. (including boarders)
	iRealWidth	= abs( rcDesiredClient.left ) + rcDesiredClient.right;
	iRealHeight	= abs( rcDesiredClient.top  ) + rcDesiredClient.bottom;
	
	// Get our client, window and workspace rects
	SystemParametersInfo( SPI_GETWORKAREA, 0, (LPVOID) &rcScreen, 0);	
	
	// Calculate our new window position based upon x and y screen ratios
	iPosX = static_cast<u32>( (rcScreen.right  - rcScreen.left - iRealWidth ) * fXPosRatio );
	iPosY = static_cast<u32>( (rcScreen.bottom - rcScreen.top  - iRealHeight) * fYPosRatio );
	
	// We can make sure our client window is correct now that we know the real width and height
	MoveWindow(hWnd, iPosX, iPosY, iRealWidth, iRealHeight, false );

//	GetClientRect( hWnd, &rcDesiredClient );

}


u32 Util_GetActiveWindow( void )
{
	return (u32)GetActiveWindow( );
}

int Util_SetCursorPos(int x, int y)
{
	return SetCursorPos(x, y);
}

void Util_CenterCursorPos( void )
{
	RECT		rcClient;
	u32			iHalfWidth, iHalfHeight;
	
	// Get the boarder size info so we can make sure our client window is the correct size
	GetClientRect( GetActiveWindow(), &rcClient );

	// Get our real width and height of the window
	iHalfWidth	= ( rcClient.right  - rcClient.left ) >> 1;
	iHalfHeight	= ( rcClient.bottom - rcClient.top  ) >> 1;
		
	// Calculate our new window position based upon x and y screen ratios
	SetCursorPos(rcClient.left + iHalfWidth, rcClient.top + iHalfHeight );
}

void Util_RemoveKeyFromString( char *s, const char *key )
{
	char buffer[STR_BUFFER_SIZE];
	char *c;
	
	strcpy(buffer, s);
	
	if( (c = strstr(buffer, key)) == 0)
		return;	
	
	c[0] = '\0';

	c = strstr(s, key);
	strcat(buffer, c + strlen(key) );

	//our new buffer should have the key removed so make s == buffer
	strcpy(s, buffer);
}

void Util_PostQuitMessage( void )
{
	PostQuitMessage( 0 );
}

void Util_GetTimeAndDate( char *output, u32 size )
{
	char		strBuffer[STR_BUFFER_SIZE];
	SYSTEMTIME	SystemTime;
	
	//Fill our SystemTime struct
	GetLocalTime( &SystemTime );
	sprintf(strBuffer, "%d/%d/%d %d:%d.%d",
		SystemTime.wMonth, SystemTime.wDay, SystemTime.wYear, 
		SystemTime.wHour, SystemTime.wMinute, SystemTime.wSecond );
	
	//if the strlen is < size, assume it is safe to copy
	if(strlen(strBuffer) < size )
	{
		strcpy(output, strBuffer);
	}
}

void Util_TextOut( u32 hDC, int x, int y, const char* str, int iStrLen )
{
	TextOut((HDC__ *)hDC, x, y, str, iStrLen);
}


void LogClear( const char* strFilename )
{
	FILE*	fp;

	//argument checks
	if( (strFilename == 0) || (strFilename[0] == 0) )
		return;

	// See if the file exists
	fp = fopen(strFilename, "wt");
	if( fp == NULL )
		return;
	fclose( fp );
	
	// Try to delete the file we found
	unlink( strFilename );
}

void LogEntry( const char* strFilename, const char* str, ... )
{
	char	strEntry[STR_BUFFER_SIZE];
	FILE*	fp;
	
	fp = fopen(strFilename, "at");
	if( fp == NULL )
		return;

	//argument checks
	if( (str == 0) || (str[0] == 0) )		//Check to make sure we have a string
	{
		fprintf(fp, "LogEntry Error! (invalid string)");
		return;
	}
	
	va_list	pArgList;						//Pointer to our list of arguments
	va_start(pArgList, str);				//Start our list of agruments
	vsprintf(strEntry, str, pArgList);	//Put our formated string into the buffer and get the size
	va_end(pArgList);						//End our argument list
	

	fprintf(fp, strEntry);
	fclose(fp);
}

//----------------------------------------------------------------------------------
//	FUNCTION NAME:	Util_ReplaceCharInString
//	AUTHOR(S):		Elijah Emerson
//	CREATION DATE:	8/2/2001
//	
//	ARGUMENT 1:		char* str	- string to look for character "c"
//	ARGUMENT 2:		char c		- character to look for 
//	ARGUMENT 3:		char n		- new character to replace "c" with
//	RETURN TYPE:	void 		- 
//	DESCRIPTION:	Replaces all characters in the string of var "c" into var "n".
//----------------------------------------------------------------------------------
void Util_ReplaceCharInString( char* str, char c, char n )
{
	int	iLen;
	
	if(str == NULL)	return;
	
	iLen = strlen(str);
	while(iLen--)
	{
		if( str[iLen] == c) 
			str[iLen] = n;
	}
}

//----------------------------------------------------------------------------------
//	FUNCTION NAME:	Util_StrTok
//	AUTHOR(S):		Elijah Emerson
//	CREATION DATE:	7/22/2001
//	
//	ARGUMENT 1:		char* strToken		- (in) pointer to string to look at for tokens
//  ARGUMENT 2:		char  cSeperator	- (in) characters that seperate one token from another
//	ARGUMENT 2:		char** pNext		- (in/out) address of pointer to next token found 
//	RETURN TYPE:	char* 				- pointer to first token found
//	DESCRIPTION:	Handy dirivitave of strtok, this function allows quotable tokens
//----------------------------------------------------------------------------------
char* Util_StrTok( char* strToken, char cSeperator, char** pNext )
{
	// Start of next token (if there is one)
	char *pStart;
	
	// If NULL is passed in, continue searching
	if( strToken == NULL )
	{
		if( *pNext != NULL )
			strToken = *pNext;
		else 
			return NULL;// Reached end of original string
	}
	
	// Look for commented out text to skip
	while( ( *strToken == '/' && *(strToken+1) == '/' ) )
	{
		// We found some commented out text. Which means that all text untill the '\n'
		// is not concidered a token. So loop untill end of string delimiter is found.
		while( *strToken != '\n' && *strToken != 0 )
		{
			++strToken;
		}
	}

	// Skip leading whitespace before next token
	while(	(*strToken == ' ') || (*strToken == '\t') || (*strToken == '\n') ) 
	{
		++strToken;
	}

	// Zero length string, so no more tokens to be found
	if ( *strToken == 0 ) 
	{
		*pNext = NULL;
		return NULL;
	}

	// Look for a quoted token
	if ( *strToken == cSeperator ) //'\"' ) 
	{
		//skip the first quote char
		++strToken;
		
		pStart = strToken;
		
		// Find ending quote or end of string
		while ( (*strToken != cSeperator) && (*strToken != 0) ) 
		{
			++strToken;
		}
		
		// Check for end of string
		if ( *strToken == 0 ) 
		{
			*pNext = NULL;
		} 
		else 
		{
			// More to find, note where to continue searching
			*strToken = 0;
			*pNext = strToken + 1;
		}
		
	} 
	else 
	{
		// Token not in quotes
		pStart = strToken;
		
		// Find next whitespace delimiter or end of string
		while ( (*strToken != 0)	&& (*strToken != ' ')  && 
			    (*strToken != '\t') && (*strToken != '\n') ) 
		{
			++strToken;
		}
		
		// Reached end of original string?
		if ( *strToken == 0 ) 
		{
			*pNext = NULL;
		} 
		else 
		{
			*strToken	= 0;
			*pNext		= strToken + 1;
		}
		return pStart;
	}
	
	// Return ptr to start of token found
	return pStart;
}

void Util_StrTabify( char* strBuffer, u32 iNumSpaces )
{
	u32 iLen;

	if(strBuffer == 0 || strBuffer[0] == 0)
		return;

	while( iLen = strlen(strBuffer) < iNumSpaces )
		strcat(strBuffer, " ");
}

//***********************************************************
//***************** STRING UTILS ****************************
//***********************************************************


HRESULT	String_GetTitleFromFilename( const char* strInput, char *strOutput )
{
	char buffer[STR_BUFFER_SIZE];
	char *p;

	// argument checks
	if(strInput == 0 || strInput[0] == 0)
		return E_INVALIDARG;
	
	//copy our strInput into a buffer we can manipulate.
	strcpy( buffer, strInput );

	//Look for directory info, if found delete it
	if( p = strrchr( buffer, '\\' ) )
	{
		//rid the directory information
		strcpy(buffer, p+1);
	}
	
	//Look for an extension, if found delete it
	if( p = strrchr(buffer, '.') )
	{
		//rid the extension information
		p[0] = NULL;
	}
	
	//Set our strOutput param
	strcpy(strOutput, buffer );

	return S_OK;
}



HRESULT String_AddPathToFilename( const char* strPath, const char *strFilename, char *strOutput )
{
	char bufPath[STR_BUFFER_SIZE];
	char bufFile[STR_BUFFER_SIZE];
	char *p;

	// argument checks
	if( strPath == 0 || strPath[0] == 0 || strFilename == 0 || strFilename[0] == 0 )
		return E_INVALIDARG;

	//copy our strPath into a buffer we can manipulate.
	strcpy( bufPath, strPath );
	
	//Look for filename info, if found delete it
	if( p = strrchr( bufPath, '\\' ) )
	{
		//get rid of the filename information
		p[1] = NULL;
	}

	//copy our strPath into a buffer we can manipulate.
	strcpy( bufFile, strFilename );
	
	//Look for Path info, if found delete it
	if( p = strrchr( bufFile, '\\' ) )
	{
		//get rid of the filename information
		strcpy(bufFile, p+1);
	}
	
	strcpy(strOutput, bufPath );
	strcat(strOutput, bufFile );

	return S_OK;
}


//----------------------------------------------------------------------------------
// FUNCTION:	String_GetString
// PARAMATERS:	FILE *fp,			- file pointer that is open with flags "rw"
//				char *str,			- character string to fill in once we find a
//									  string in quotes.
//
// RETURNS:		u32					- 0 == OK, 1 == ERROR
//
// DESCRIPTION:	This function will look for the next quote symbol '"' and once it
//				finds that quote symbol it will read in characters untill either 
//				another	quote symbol or a return symbol is found.
//
//----------------------------------------------------------------------------------
HRESULT String_GetString(const char *strInput, char *strOutput)
{
	if(strInput == NULL) return E_INVALIDARG;

	char *pos;
	
	//Find the beggining of our string by looking for a quote symbol '"'.
	pos = strchr(strInput, '"');

	if(!pos)
	{
		DebugOutput("\n***Error looking for \" in string %s!!!\n", strInput);
		return E_FAIL;
	}
	//Copy our new found string untill we hit a '"' or the end of file marker.
	++pos;
	while( pos[0] != '"')
	{
		*strOutput = pos[0];
		++strOutput;
		
		++pos;

		//If we find a /" then disregaurd the " 
		if(pos[0] == '"' && strOutput[-1] == '/')
		{
			strOutput[-1] = '"';
			++pos;
		}
	}
	*strOutput = '\0'; //put null at the end of our string

	return S_OK;//success
}

//***********************************************************
//************************* FILE UTILS **********************
//***********************************************************

//----------------------------------------------------------------------------------
// FUNCTION:	LoopUntilString
// PARAMATERS:	FILE *fp,			- file pointer that is open with flags "rw"
//				char *str,			- character string to fill in once we find a
//									  string in quotes.
//
// RETURNS:		HRESULT				- 0 == OK, 1 == ERROR
//
// DESCRIPTION:	This function searches a text file for the first occurrence of
//				<string> starting from the current position of the file pointer.
//
//----------------------------------------------------------------------------------
HRESULT LoopUntilString(FILE *fp, const char *str)
{
	if(!fp || !str ) return E_INVALIDARG;

	char buffer[STR_BUFFER_SIZE];
	s32	 length;
	
	length = strlen(str);
	if(length >= STR_BUFFER_SIZE)
		return E_INVALIDARG;

	buffer[length]	= '\0';

	// read <length> characters at a time, then do a strcmp
	for(s32 i = 0; i < length; ++i)
	{
		buffer[i] = getc(fp);
		if(feof(fp) )
		{
			DebugOutput("\n*** Error looking for tag %s in LoopUntilSuck, er..String *** \n", str);
			return ERROR_HANDLE_EOF;
		}
	}
	
	// this loop will only execute if the strings didn't match the first time
	while(strcmp(buffer, str))
	{
		// move all the letters in buffer back by one space,
		// then read a new character
		for(i = 0; i < length - 1; ++i)
			buffer[i]		= buffer[i + 1];

		buffer[length - 1]	= getc(fp);
		if(feof(fp) )
		{
			DebugOutput("\n*** Error looking for tag %s in LoopUntilSuck, er..String *** \n", str);
			return ERROR_HANDLE_EOF; //return failure
		}
	}

	return S_OK;//return success
}

#define TCHAR_CALL_OS(funcW,funcA) (funcA)
#define TCHAR_TO_ANSI(str) str
#define ANSI_TO_TCHAR(str) str

INT Util_FileSize( const char* Filename )
{
	HANDLE Handle = TCHAR_CALL_OS( CreateFileW( Filename, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL ), CreateFileA( TCHAR_TO_ANSI(Filename), GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL ) );
	if( Handle==INVALID_HANDLE_VALUE )
		return -1;
	DWORD Result = GetFileSize( Handle, NULL );
	CloseHandle( Handle );
	return Result;
}

BOOL Util_FileDelete( const char* Filename, bool bRequireExists, bool bEvenReadOnly )
{
	if( bEvenReadOnly )
		TCHAR_CALL_OS(SetFileAttributesW(Filename,FILE_ATTRIBUTE_NORMAL),
					  SetFileAttributesA(TCHAR_TO_ANSI(Filename),FILE_ATTRIBUTE_NORMAL));
	return TCHAR_CALL_OS(DeleteFile(Filename),DeleteFileA(TCHAR_TO_ANSI(Filename)))!=0 || (!bRequireExists && GetLastError()==ERROR_FILE_NOT_FOUND);
}

/*
bool Util_FileMove( const TCHAR* Dest, const TCHAR* Src, bool Replace=1, bool EvenIfReadOnly=0, bool Attributes=0 )
{
	//warning: MoveFileEx is broken on Windows 95 (Microsoft bug).
	Delete( Dest, 0, EvenIfReadOnly );
	INT Result = TCHAR_CALL_OS( MoveFileW(Src,Dest), MoveFileA(TCHAR_TO_ANSI(Src),TCHAR_TO_ANSI(Dest)) );
	if( !Result )
		debugf( NAME_Warning, TEXT("Error moving file '%s' to '%s'"), Src, Dest );
	return Result!=0;
}
*/

//----------------------------------------------------------------------------------
// FUNCTION:	File_GetString
// PARAMATERS:	FILE *fp,			- file pointer that is open with flags "rw"
//				char *str,			- character string to fill in once we find a
//									  string in quotes.
//
// RETURNS:		HRESULT				- 0 == OK, 1 == ERROR
//
// DESCRIPTION:	This function will look for the next quote symbol '"' and once it
//				finds that quote symbol it will read in characters untill either 
//				another	quote symbol or a return symbol is found.
//
//----------------------------------------------------------------------------------
HRESULT File_GetString(FILE *fp, char *result)
{
	if(fp == NULL) return E_INVALIDARG;
	
	char c, *p = result;
	
	//Find the beggining of our string by looking for a quote symbol '"'.
	while( ( c = getc(fp) ) != '"' )
	{
		if(feof(fp)	)	
		{
			DebugOutput("*** Unexpected eof in File_GetString()\n");
			return ERROR_HANDLE_EOF;	//end of file found, return fail
		}
	}
	
	//Copy our new found string untill we hit a '"' or the end of file marker.
	
	c  = getc(fp);
	while( c != '"')
	{
		*p = c;
		++p;
		
		c  = getc(fp);

		//If we find a /" then disregaurd the " 
		if(c == '"' && p[-1] == '/')
		{
			p[-1] = '"';
			c = getc(fp);
		}
		
		if(feof(fp))
		{
			DebugOutput("*** Unexpected eof in File_GetString()\n");
			return ERROR_HANDLE_EOF;	//end of file found, return fail
		}
	}
	*p = '\0'; //put null at the end of our string

	return S_OK;//success
}

HRESULT File_GetString_NoQuotes(FILE *fp, char *result)
{
	if(fp == NULL) return E_INVALIDARG;

	s32 i = 0;
	char c;

	//***Find the BEGINING***
	//go through string untill we find a valid symbol
	while( ((c = getc(fp)) < '!' ) || c > 'z' || c == '"')
	{
		if(feof(fp)	)
		{
			DebugOutput("***Unexpected eof in File_GetString_NoQuotes()\n");
			return ERROR_HANDLE_EOF;	//end of file found, return fail
		}
	}

	//since we already found 1 valid character lets assign that to our buffer
	result[i++] = c;

	//***Find the END***
	while( (c = getc(fp)) >= '!' && c <= 'z' && c != '"')//!= '\n' &&	c != '\t' && c != '\r' && c != ' ' ) 
	{
		if(feof(fp))
		{
			DebugOutput("***Unexpected eof in File_GetString_NoQuotes()\n");
			return ERROR_HANDLE_EOF;	//end of file found, return fail
		}
		result[i++] = c;
	}

	//terminate our string
	result[i++] = NULL;
	
	return S_OK;//success
}

//----------------------------------------------------------------------------------
// FUNCTION:	File_GetInt
// PARAMATERS:	FILE *fp,			- file pointer that is open with flags "rw"
//				s32  *result,		- pointer to result that we are going to fill.
//
// RETURNS:		HRESULT					- 0 == OK, 1 == ERROR
//
// DESCRIPTION:	This function will look for the next number, or '-' sign in the
//				file pointer and then read in that number and fill in the *result pointer.
//
//----------------------------------------------------------------------------------
HRESULT File_GetInt(FILE *fp, s32 *result)
{
	if(fp == NULL) return E_INVALIDARG;

	char	c;
	//***Find the BEGINING of our number***
	while( ( ((c = getc(fp)) < '0') || c > '9') && c != '-' )
	{
		if(feof(fp))
		{
			DebugOutput("***Unexpected eof in File_GetInt()\n");
			return ERROR_HANDLE_EOF;	//end of file found, return fail
		}
	}
	fseek(fp, -1, SEEK_CUR);
	fscanf(fp, "%d", result);
	return S_OK; //return success
}

//----------------------------------------------------------------------------------
// FUNCTION:	File_GetInt
// PARAMATERS:	FILE *fp,			- file pointer that is open with flags "rw"
//				u32  *result,		- pointer to result that we are going to fill.
//
// RETURNS:		HRESULT					- 0 == OK, 1 == ERROR
//
// DESCRIPTION:	This function will look for the next number, or '-' sign in the
//				file pointer and then read in that number and fill in the *result pointer.
//
//----------------------------------------------------------------------------------
HRESULT File_GetInt(FILE *fp, u32 *result)
{
	if(fp == NULL) return E_INVALIDARG;

	char	c;
	//***Find the BEGINING of our number***
	while( ( ((c = getc(fp)) < '0') || c > '9') && c != '-' )
	{
		if(feof(fp)	)	
		{
			DebugOutput("***Unexpected eof in File_GetInt()\n");
			return ERROR_HANDLE_EOF;	//end of file found, return fail
		}
	}
	fseek(fp, -1, SEEK_CUR);
	fscanf(fp, "%d", result);
	return S_OK; //return success
}

//----------------------------------------------------------------------------------
// FUNCTION:	File_GetFloat
// PARAMATERS:	FILE *fp,			- file pointer that is open with flags "rw"
//				f32  *result,		- pointer to result that we are going to fill.
//
// RETURNS:		HRESULT				- 0 == OK, 1 == ERROR
//
// DESCRIPTION:	This function will look for the next number, or '-' sign in the
//				file pointer and then read in that number and fill in the *result pointer.
//
//----------------------------------------------------------------------------------
HRESULT File_GetFloat(FILE *fp, f32 *result)
{
	if(fp == NULL) return E_INVALIDARG;

	char	c;
	//***Find the BEGINING of our number***
	while( ( ((c = getc(fp)) < '0') || c > '9') && c != '-' )
	{
		if(feof(fp))
		{
			DebugOutput("***Unexpected eof in File_GetFloat()\n");
			return ERROR_HANDLE_EOF;	//end of file found, return fail
		}
	}
	fseek(fp, -1, SEEK_CUR);
	fscanf(fp, "%f", result);
	return S_OK; //success
}

HRESULT File_GetDouble(FILE *fp, f64 *result)
{
	if(fp == NULL)	return E_INVALIDARG;

	char	c;
	f32 f;

	//***Find the BEGINING of our number***
	while( ( ((c = getc(fp)) < '0') || c > '9') && c != '-' )
	{
		if(feof(fp))
		{
			DebugOutput("***Unexpected eof in File_GetDouble()\n");
			return ERROR_HANDLE_EOF;	//end of file found, return fail
		}
	}
	fseek(fp, -1, SEEK_CUR);
	fscanf(fp, "%f", &f);

	*result = f;

	return S_OK; //success
}


//**************************************************************
//** These functions are here to make life easy... thats all. **
//**************************************************************
HRESULT File_GetIntAfterString(FILE *fp, const char *string, s32 *result)
{
	if(fp == NULL)	return E_INVALIDARG;
	HRESULT ret = LoopUntilString(fp, string);
	return ( ret != S_OK ) ? ret : File_GetInt(fp, result);
}

HRESULT File_GetIntAfterString(FILE *fp, const char *string, u32 *result)
{
	if(fp == NULL)	return E_INVALIDARG;
	HRESULT ret = LoopUntilString(fp, string);
	return ( ret != S_OK ) ? ret : File_GetInt(fp, result);
}

HRESULT File_GetFloatAfterString(FILE *fp, const char *string, f32 *result)
{
	if(fp == NULL)	return E_INVALIDARG;
	HRESULT ret = LoopUntilString(fp, string);
	return ( ret != S_OK ) ? ret : File_GetFloat(fp, result);
}

HRESULT	File_GetStringAfterString(FILE *fp, const char *string, char *result)
{
	if(fp == NULL)	return E_INVALIDARG;
	HRESULT ret = LoopUntilString(fp, string);
	return ( ret != S_OK ) ? ret : File_GetString(fp, result);
}

//*************************************************************************************
//********************************* CFileList *****************************************
//*************************************************************************************

void CFileList::Clear()
{
	m_iNumFiles			= 0;
	m_pFileList			= NULL;
	
	m_iNumDirectories	= 0;	//total number of directorys in list.
	m_pDirList			= NULL;	//2D array because we need a "single" array of character strings
	
	m_iNumPaths			= 0;
	m_pFileInfoList		= NULL;	//A malloced array of FileInfo structs
	
	for(int i=0; i< MAX_NUM_PATHS; ++i)
		m_strPathList[i][0] = 0;
}

HRESULT CFileList::Destroy()
{
	u32 i;
	
	if( m_pFileInfoList )
	{
		delete[] m_pFileInfoList;
		m_pFileInfoList = NULL;
	}
	
	if( m_pDirList )
	{
		for(i=0; i < m_iNumDirectories; i++)
		{
			free(m_pDirList[i]);
			m_pDirList[i] = NULL;
		}
		free(m_pDirList);
		m_pDirList = NULL;
	}

	Clear();

	return S_OK;
}

void CFileList::FindFiles( const char* Filename, bool Files, bool Directories )
{
	HANDLE Handle = NULL;
#if UNICODE
	if( GUnicodeOS )
	{
		WIN32_FIND_DATAW Data;
		Handle = FindFirstFileW( Filename, &Data );
		if( Handle!=INVALID_HANDLE_VALUE )
		{
			do
			{
				if(	stricmp(Data.cFileName,TEXT(".")) && 
					stricmp(Data.cFileName,TEXT(".."))&& 
					((Data.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY)?Directories:Files) )
				{
					//new(Result)FString(Data.cFileName);
					strcpy(m_strPathList[m_iNumPaths], Data.cFileName );
					++m_iNumPaths;
				}
			}
			while( FindNextFileW(Handle, &Data) && m_iNumPaths < MAX_NUM_PATHS );
		}
	}
	else
#endif
	{
		WIN32_FIND_DATAA Data;
		Handle = FindFirstFileA(TCHAR_TO_ANSI(Filename), &Data );
		if( Handle!=INVALID_HANDLE_VALUE )
		{	
			do
			{
				if(	stricmp(ANSI_TO_TCHAR(Data.cFileName),TEXT(".")) && 
					stricmp(ANSI_TO_TCHAR(Data.cFileName),TEXT(".."))&&	
					((Data.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY)?Directories:Files) )
				{
					//new(Result)FString(ANSI_TO_TCHAR(Data.cFileName));
					strcpy(m_strPathList[m_iNumPaths], Data.cFileName );
					++m_iNumPaths;
				}
			}
			while( FindNextFileA(Handle, &Data) && m_iNumPaths < MAX_NUM_PATHS );
		}
	}
	if( Handle )
		FindClose( Handle );
}


const char* CFileList::GetFileName( u32 i ) const
{
	if(i > m_iNumFiles || m_pFileInfoList == NULL)
		return NULL;

	return m_pFileInfoList[i].m_strFileName;
}

const char* CFileList::GetPath( u32 i ) const
{
	if(i > m_iNumFiles || m_pFileInfoList == NULL)
		return NULL;

	return m_pFileInfoList[i].m_strPath;
}

const char* CFileList::GetPathAndFileName( u32 i ) const
{
	if(i > m_iNumFiles || m_pFileInfoList == NULL)
		return NULL;

	return m_pFileInfoList[i].m_strPathAndName;
}