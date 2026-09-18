/*=============================================================================
	UnReplayWire.cpp: FReplay input-event wire serialization.

	Kept in a dependency-light translation unit so the production engine and
	the linked-independent replay oracle execute the same operator.
=============================================================================*/

#include "EnginePrivate.h"

FArchive& operator<<( FArchive& Ar, FReplay::FInputEvent& IE )
{
	// Serialize two bytes.
	BYTE iKey = IE.iKey, State = IE.State;
	if( IE.Delta != 0.0f )
		State |= 0x80;
	Ar << iKey << State;
	IE.iKey = EInputKey(iKey);  IE.State = EInputAction(State & 0x7F);

	// Serialize the analog value only when needed.
	if( State & 0x80 )
		Ar << IE.Delta;
	else if( Ar.IsLoading() )
		IE.Delta = 0.f;
	return Ar;
}
