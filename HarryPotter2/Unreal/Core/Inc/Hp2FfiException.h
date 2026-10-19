#ifndef _INC_HP2FFIEXCEPTION_H
#define _INC_HP2FFIEXCEPTION_H

// Shared exception capture for C++ services thunks called from Rust.
//
// A C++ throw (appErrorf under a guard, appThrowf, appFailAssert, CastChecked) must never unwind
// through a Rust frame: the exports are plain extern "C", so such an unwind aborts. Each thunk
// therefore runs its body through Hp2FfiCall, which keeps the first exception and returns 1. The
// Rust side returns Err on status 1 and makes NO further service calls on that path, so the
// effects stop at the same point as the original throw. The C++ shell then calls
// Hp2RethrowPending(status) after the export returns. That call runs inside the shell's guard()/
// unguard (UnBuild.h guard macros, which catch the throw and call appUnwindf), so the throw lands
// in the same catch site as the original throw. Include this after EnginePrivate.h.

#include <cstdint>
#include <exception>

inline std::exception_ptr& Hp2PendingException()
{
	static thread_local std::exception_ptr Pending;
	return Pending;
}

// Runs one services thunk body. Returns 0 when the body completes, or 1 after recording the first
// thrown exception (later exceptions on the same path are not recorded).
template< class Fn >
int32_t Hp2FfiCall( Fn Body )
{
	try
	{
		Body();
		return 0;
	}
	catch( ... )
	{
		if( !Hp2PendingException() )
			Hp2PendingException() = std::current_exception();
		return 1;
	}
}

// Call after a Rust export returns. A non-zero status rethrows the recorded exception (clearing it
// first). A non-zero status with nothing recorded means the Rust side failed on its own (a panic
// caught by guard), so it raises a plain engine error at this point.
inline void Hp2RethrowPending( int32_t Status )
{
	if( Status == 0 )
		return;
	std::exception_ptr E = Hp2PendingException();
	Hp2PendingException() = std::exception_ptr();
	if( E )
		std::rethrow_exception( E );
	appErrorf( TEXT("Rust net port call failed.") );
}

#endif
