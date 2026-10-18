/*=============================================================================
	HP2EngineServices.h: Engine-global services called by the Rust engine port
	(crates/hp2-engine). The table is defined in HP2EngineServices.cpp.
=============================================================================*/

#ifndef HP2ENGINESERVICES_H
#define HP2ENGINESERVICES_H

#include "hp2_rs.h"

extern const Hp2EngineServices GHp2EngineServices;

// Native shim for a script-callable function. Defines Class::exec<Name>: the script
// parameters are decoded in C++ by Decode (e.g. P_GET_VECTOR(Loc)), then the call is
// forwarded to hp2_rs_native_<Class>_<Name>(services, this, __VA_ARGS__). The trailing
// arguments are the export's own inputs and output (e.g. &Loc, (FVector*)Result).
// Usage: HP2_RUST_NATIVE( AWind, GetWind, P_GET_VECTOR (Loc), &Loc, (FVector*)Result )
#define HP2_RUST_NATIVE(Class, Name, Decode, ...) \
	void Class::exec##Name( FFrame& Stack, RESULT_DECL ) \
	{ \
		Decode; \
		P_FINISH; \
		hp2_rs_native_##Class##_##Name( &GHp2EngineServices, this, __VA_ARGS__ ); \
	}

#endif
