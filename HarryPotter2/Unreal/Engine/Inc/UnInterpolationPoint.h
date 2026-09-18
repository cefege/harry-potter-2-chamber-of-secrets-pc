/*=============================================================================
	UnInterpolationPoint.h: Unreal Interpolation Point definitions.

	These definitions are compiled prior to the inclusion of the
	EngineClasses.h file.  Definitions contained herein can be
	refered to in the definition of the InterpolationPoint class
	(InterpolationPoint.uc).
=============================================================================*/

// Alternative route editable info (also defined in InterpolationPoint.uc)
struct FSwitchInfo
{
	FLOAT			Chance;				// Chance (out of 1.0) that this alternative path would be taken (0=never)
	FName			PathName;			// Tag Name of path to switch to
	INT				PathPosition;		// Position to connect to on that path
};

// Alternative route private info (also defined in InterpolationPoint.uc)
struct FSwitchInfo_Priv
{
    class AInterpolationPoint*	Next;	// Actual point on the path to go to
};

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
