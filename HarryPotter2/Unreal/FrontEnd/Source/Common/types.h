// ---------------------------------------------------------------------------------
//  _                            _     
// | |                          | |    
// | |_ _   _ _ __   ___ ___    | |__  
// | __| | | | '_ \ / _ \ __|   | '_ \ 
// | |_| |_| | |_) |  __/__ \ _ | | | |
//  \__|\__, | .__/ \___|___/(_)|_| |_|
//       __/ | |                       
//      |___/|_|                       
//
// ---------------------------------------------------------------------------------
// Created on  : 07/11/2001
// Authored by : Elijah Emerson
//
// Dependencies: NONE
// 
// Description : The types.h file is the most commonly included file in my source
//				 code. It defines all basic types as well as common macros and templates.
//
// ---------------------------------------------------------------------------------

#pragma once
#ifndef	_TYPES_H_
#define _TYPES_H_


//**********************
//*** STD DATA TYPES ***
//**********************

typedef unsigned char		u8;
typedef unsigned short		u16;
typedef unsigned int		u32;
typedef unsigned long		u64;

typedef char				s8;
typedef short				s16;
typedef int					s32;
typedef long				s64;

typedef float				f32;
typedef double				f64;

typedef unsigned char		byte;
typedef unsigned short int	word;
typedef unsigned long  int	dword;
typedef unsigned __int64	qword;

typedef signed __int64		longlong;

typedef unsigned char		uchar;
typedef unsigned int		uint;
typedef unsigned short int	ushort;
typedef unsigned long  int	ulong;
typedef unsigned __int64	ulonglong;


#define STR_BUFFER_SIZE		512



//**************************
//*** STANDARD TEMPLATES ***
//**************************
template <class T> inline T_ABS	 (T& x)					{ return (x >= 0.0f ) ? x : -x; }
template <class T> inline T_MAX  (T& x, T& y)			{ return x > y ? x : y; }
template <class T> inline T_CLAMP(T& x,T& min,T& max)	{ return (x > max) ? max : (x < min) ? min: x ; }


//***********************
//*** STANDARD MACROS ***
//***********************
#ifndef SAFE_DELETE
#define SAFE_DELETE(_x) { if(_x){ delete _x; _x=NULL; } }
#endif
#ifndef SAFE_DELETE_ARRAY
#define SAFE_DELETE_ARRAY(_x) { if(_x){ delete[] _x; _x=NULL; } }
#endif
#ifndef SAFE_RELEASE
#define SAFE_RELEASE(_x){ if(_x){ _x->Release(); _x=NULL; } }
#endif


#define absf(x)	((x) >= 0.0f ? (x) : -(x))
#define CLAMP(_x,_min,_max)	( ((_x)>(_max)) ? (_max) : ((_x)<(_min)) ? (_min) : (_x) )

#define COLOR_ARGB(a,r,g,b) \
			((DWORD)((((a)&0xff)<<24)|(((r)&0xff)<<16)|(((g)&0xff)<<8)|((b)&0xff)))

#define COLOR_RGBA(r,g,b,a) COLOR_ARGB(a,r,g,b)

#define COLOR_COLORVALUE(r,g,b,a) \
			COLOR_RGBA((DWORD)((r)*255.f),(DWORD)((g)*255.f),(DWORD)((b)*255.f),(DWORD)((a)*255.f))

//**************************
//*** STANDARD Functions ***
//**************************
//DWORD inline COLOR_FromInt	(u32 a,u32 r,u32 g,u32 b){ return ((DWORD)((((a)&0xff)<<24)|(((r)&0xff)<<16)|(((g)&0xff)<<8)|((b)&0xff))); }
//DWORD inline COLOR_FromFloat(f32 a,f32 r,f32 g,f32 b){ return COLOR_FromInt((DWORD)((r)*255.f),(DWORD)((g)*255.f),(DWORD)((b)*255.f),(DWORD)((a)*255.f)); }


//*******************
//*** ERROR TYPES ***
//*******************
#ifndef _HRESULT_DEFINED
#define _HRESULT_DEFINED
typedef long HRESULT;

#define		S_OK					0x00000000	// Operation successful				
#define		E_UNEXPECTED			0x8000FFFF	// Unexpected failure					
#define		E_NOTIMPL				0x80004001	// Not implemented						
#define		E_OUTOFMEMORY			0x8007000E	// Failed to allocate necessary memory 
#define		E_INVALIDARG			0x80070057	// One or more arguments are invalid	
#define		E_NOINTERFACE			0x80004002	// No such interface supported			
#define		E_POINTER				0x80004003	// Invalid pointer						
#define		E_HANDLE				0x80070006	// Invalid handle						
#define		E_ABORT					0x80004004	// Operation aborted					
#define		E_FAIL					0x80004005	// Unspecified failure					
#define		E_ACCESSDENIED			0x80070005	// General access denied error			

#define		ERROR_FILE_NOT_FOUND	2L			// Could not find specified file
#define		ERROR_PATH_NOT_FOUND	3L			// Could not find specified path
#define		ERROR_HANDLE_EOF		38L			// Unexpected End of file found

#ifndef SUCCEEDED
#define SUCCEEDED(_Status) ((HRESULT)(_Status) >= 0)
#endif

#ifndef FAILED
#define FAILED(_Status) ((HRESULT)(_Status)<0)
#endif

#endif // !_HRESULT_DEFINED



//******************
//*** WINNT DEFS ***
//******************
//-----------------------------------
#ifndef _WINNT_

typedef void *PVOID;

#ifdef STRICT
typedef void *HANDLE;
#define DECLARE_HANDLE(name) struct name##__ { int unused; }; typedef struct name##__ *name
#else
typedef PVOID HANDLE;
#define DECLARE_HANDLE(name) typedef HANDLE name
#endif
typedef HANDLE *PHANDLE;

#define DLL_PROCESS_ATTACH 1    
#define DLL_THREAD_ATTACH  2    
#define DLL_THREAD_DETACH  3    
#define DLL_PROCESS_DETACH 0  

#endif //end _WINNT_
//-----------------------------------

//********************
//*** WINDOWS DEFS ***
//********************
//-----------------------------------
#ifndef _WINDEF_

#define NULL		0

//common vkeys
/*
#define VK_RETURN	0x0D
#define VK_SPACE	0x20
#define VK_LEFT		0x25
#define VK_UP		0x26
#define VK_RIGHT	0x27
#define VK_DOWN		0x28
*/
typedef unsigned char		BYTE;
typedef unsigned short int	WORD;
typedef unsigned long  int	DWORD;
typedef unsigned int		UINT;

typedef UINT WPARAM;
typedef s64 LPARAM;
typedef s64 LRESULT;


#ifndef WIN_INTERNAL
DECLARE_HANDLE( HWND );
#endif

#ifndef GDI_INTERNAL
DECLARE_HANDLE( HDC );
#endif

//MAKE_HANDLE( HWND );
//MAKE_HANDLE( HINSTANCE );
//MAKE_HANDLE( HICON );

typedef struct tagRECT
{
    long    left;
    long    top;
    long    right;
    long    bottom;
} RECT, *LPRECT;

#endif	//end _WINDEF_
//-----------------------------------


/*
//FRect's use floats instead of longs
struct FRECT
{
	f32	 left,top,right,bottom;
		 FRECT		( void )						{ }
		 ~FRECT		( void )						{ }
		 FRECT		( f32 l, f32 t, f32 r, f32 b )	{ left = l; top = t; right = r; bottom = b; }
		 FRECT		( const RECT& rhs )				{ FromRect(rhs); }
	void Clear		( void )						{ left = top = right = bottom = 0.0f; }
	void FromRect	( const RECT& rhs )				{ left =(f32)rhs.left; top =(f32)rhs.top; right = (f32)rhs.right; bottom =(f32)rhs.bottom; }
};

struct IRECT : public RECT
{
		 IRECT		( void )						{ }
		 ~IRECT		( void )						{ }
		 IRECT		( u32 l, u32 t, u32 r, u32 b )	{ left = l; top = t; right = r; bottom = b; }
		 IRECT		( const FRECT& rhs )			{ FromFRect(rhs); }
	void Clear		( void )						{ left = top = right = bottom = 0; }
	void FromFRect	( const FRECT& rhs )			{ left = (u32)rhs.left; top =(u32)rhs.top; right =(u32)rhs.right; bottom =(u32)rhs.bottom; }
};

//little helper functions
inline void ClearRect( RECT&  rc ) { rc.left = rc.right = rc.bottom = rc.top = 0; } 
*/
//Convert WinHandle to and from long and ulong
//__inline u64	WinHandleToULong(const void *h ){ return( (u64) h ); }
//__inline s64	WinHandleToLong	(const void *h ){ return( (s64) h ); }
//__inline void*	ULongToWinHandle(const u64 h )	{ return((void *)(u64*) h ); }
//__inline void*	LongToWinHandle	(const s64 h )	{ return((void *)(s64*) h ); }
//__inline void*	ULongToWinHandle(const u64 h )	{ return((void *)(UINT_PTR) h ); }
//__inline void*	LongToWinHandle	(const s64 h )	{ return((void *)(INT_PTR)  h ); }




#endif //end TYPES_H