//=============================================================================
// InterpolationPoint.
// used to mark bezier spline path for scripted camera sequence (can also be used for other actors)
//=============================================================================
class InterpolationPoint extends Keypoint
	native;

#exec Texture Import File=Textures\IntrpPnt.pcx Name=S_Interp Mips=Off Flags=2

// Alternative route editable info (also defined in UnInterpolationPoint.h)
struct SwitchInfo
{
	var() float		Chance;			// Chance (out of 1.0) that this alternative path would be taken (0=never)
	var() name		PathName;		// Tag Name of path to switch to
	var() int		PathPosition;	// Which position on that path to switch to
};

// Alternative route private info (also defined in UnInterpolationPoint.h)
struct SwitchInfo_Priv
{
	var InterpolationPoint	Next;	// Actual point on the path to go to
};

// Number in sequence sharing this tag.
var() int    Position;				// position of this point in the path (specifies order)
var() bool   bEndOfPath;			// if true, stop at this path
var() bool   bInstantNextPath;		// when this path is the next path, move to it instantly, and apply all 0 offset effect instantly
var() bool	 bFaceMoveDirection;	// camera faces movement direction
//var() bool   bDontAlterRotation;    // turn off bFaceMoveDirection and turn this on, and it will not do any rotations
var   bool   bRollIntoTurn;			// while facing move direction, roll camera based on turn
var   bool	 bSmoothPath;			// true by default - when one control point is adjusted, other is moved to keep tangents the same
var() bool	 bConstantSpeed;		// if true, try to match DesiredVelocity as closesly as possible at all times
var	  bool	 bTurnChange;			// set true if yaw or pitch direction from prev to me is different than from me to next
var /*()*/ float	 Pause[8];		// pause this length of time at this path
var /*()*/ name   ViewTargetTag[8];	// tag of actor to focus on	
var() bool   bNewRotationSmoothing;
var   transient actor	 ViewTarget[8];

// smoothly change values either over period of move or during pause
var /*()*/ float  GameSpeed[8];		// rate of passage of time, smoothly changed
var /*()*/ float  FovModifier[8];	// modifies player's FOV. Default value is 1
var() float  ScreenFlashScale[8];	// modifies player's screen flash smoothly. Default value is 1
var() vector ScreenFlashFog[8];		// modifies player's screen fog smoothly.  Default value is (0,0,0)
var() name Events[8];				// the event to trigger when the move (if 0) or the pause is completed
var() byte NonLinearEffect[8];		// whether effect should be applied in a non-linear fashion

var InterpolationPoint Prev;		// previous point in this interpolation path.
var InterpolationPoint Next;		// following point in this interpolation path.

var() vector  StartControlPoint;	// control point offset for bezier section am start of
var() vector  EndControlPoint;		// control point offset for bezier section am end of

var() color DistanceFogColor;
var() float DistanceFogStart;
var() float DistanceFogEnd;

var() float Smoothing;			// how much to smooth rotation rate changes occuring at this path
var() float DesiredSpeed;		// if not 0, try to match this velocity.  Try to maintain as average for each spline, or if bConstantSpeed, try to maintain for each tick
var	  float PathDist;			// length of spline path ending at this point (calculated in editor)

// Spline Actions
var(SplineActions) name      PatrolAnim;
var(SplineActions) float	 tweenTime;				// The time taken between animations (to move from first to second)
var(SplineActions) sound     PatrolSound;
var(SplineActions) float     PauseTime;
var(SplineActions) name      PauseAnim;
var(SplineActions) bool      bUseLookDir; //If on, and pausetime is positive, char will turn to this point's orientation during the pause.
var(SplineActions) name      EventToSend;
var(SplineActions) bool      bDestroyPawn;

// Switch properties
var() SwitchInfo	Switches_Forward[8];		// Switches available going forward
var() SwitchInfo	Switches_Reverse[8];		// Switches available going backward

var SwitchInfo_Priv	Switches_Forward_Priv[8];	// Private info about forward switches
var SwitchInfo_Priv	Switches_Reverse_Priv[8];	// Private info about reverse switches

//
// At start of gameplay, link all matching interpolation points together.
//
simulated function BeginPlay()
{
	local int	i;
	local InterpolationPoint Current;
	local name	SwitchPathName;

	Super.BeginPlay();

	// Try to find next interpolation point.
	foreach AllActors( class 'InterpolationPoint', Current, Tag )
	{
		if ( (Current.Position > Position)
			&& ((Next == None) || (Next.Position > Current.Position)) )
		{
			Next = Current;
			if( Next.Position == Position+1 )
				break;
		}
	}

	// If not found, find firstmost
	if( Next == None )
	{
		foreach AllActors( class 'InterpolationPoint', Current, Tag )
		{
			if ( (Next == None) || (Next.Position > Current.Position) )
			{
				Next = Current;
				if( Next.Position == 0 )
					break;
			}
		}
	}

	// Link back
	if( Next != None )
		Next.Prev = Self;

	// Try to find next interpolation points for all forward alternative paths;
	// find point whose position is nearest but not below the desired point number
	for ( i = 0; i < 8; ++i )
	{
		if ( Switches_Forward[i].Chance > 0.0 )
		{
			SwitchPathName = Switches_Forward[i].PathName;
			if ( SwitchPathName == '' && Switches_Forward[i].PathPosition > 0 )
				SwitchPathName = Tag;
			foreach AllActors( class'InterpolationPoint', Current, SwitchPathName )
			{
				if (    Current.Position >= Switches_Forward[i].PathPosition
					 && (Switches_Forward_Priv[i].Next == None || Current.Position < Switches_Forward_Priv[i].Next.Position) )
				{
					Switches_Forward_Priv[i].Next = Current;
					if ( Current.Position == Switches_Forward[i].PathPosition )
						break;
				}
			}
		}
	}

	// Try to find next interpolation points for all reverse alternative paths;
	// find point whose position is nearest but not above the desired point number
	for ( i = 0; i < 8; ++i )
	{
		if ( Switches_Reverse[i].Chance > 0.0 )
		{
			SwitchPathName = Switches_Forward[i].PathName;
			if ( SwitchPathName == '' && Switches_Reverse[i].PathPosition > 0 )
				SwitchPathName = Tag;
			foreach AllActors( class'InterpolationPoint', Current, SwitchPathName )
			{
				if (    Current.Position <= Switches_Reverse[i].PathPosition
					 && (Switches_Reverse_Priv[i].Next == None || Current.Position > Switches_Reverse_Priv[i].Next.Position) )
				{
					Switches_Reverse_Priv[i].Next = Current;
					if ( Current.Position == Switches_Reverse[i].PathPosition )
						break;
				}
			}
		}
	}


	if ( (Events[0] == 'None') || (Events[0] == '') )
		Events[0] = Event;

	if ( bFaceMoveDirection || (ViewTargetTag[0] != 'None')
		|| (Prev == None) || (Next == None) )
		return;
	if ( Next.bFaceMoveDirection || (Next.ViewTargetTag[0] != 'None')
		|| Prev.bFaceMoveDirection || (Prev.ViewTargetTag[0] != 'None') )
		return;

	for ( i=0; i<8; i++ )
	{
		GameSpeed[i] = FMax(GameSpeed[i], 0.2);
	}
	bTurnChange = ( (PlusDir(Next.Rotation.Yaw,Rotation.Yaw) != PlusDir(Rotation.Yaw,Prev.Rotation.Yaw))
					|| (PlusDir(Next.Rotation.Pitch,Rotation.Pitch) != PlusDir(Rotation.Pitch,Prev.Rotation.Pitch)) );
					
}

// returns true if shortest rotation direction is in the positive (clockwise) direction
function bool PlusDir(int A, int B)
{
	A = A & 65535;
	B = B & 65535;

	if ( Abs(A - B) > 32768 )
		return ( A - B < 0 );
	return ( A - B > 0 );
}

//
// Determines whether to switch to an alternative path or stay on course
//
function InterpolationPoint DetermineNextDestination( bool bForward )
{
	// Pick the next switch alternative from the set corresponding to the
	// current traveling direction
	local float			fRandChoice;
	local float			fTotalChance;
	local int			iChoice;
	local int			i;

	fRandChoice = frand();
	if ( bForward )
	{
		// Compute total chance used in this point
		if ( bEndOfPath )
		{
			fTotalChance = 0.0;
			for ( i=0; i<8; ++i )	// If main line is shut off, spread chance over all alternatives
				fTotalChance += Switches_Forward[i].Chance;
		}
		else
			fTotalChance = 100.0;

		// Pick choice
		fRandChoice *= fTotalChance;
		for ( i = 0; i < 8; ++i )
		{
			if ( fRandChoice < Switches_Forward[i].Chance )
				return Switches_Forward_Priv[i].Next;

			fRandChoice -= Switches_Forward[i].Chance;
			if ( fRandChoice < 0.0 )
				fRandChoice = 0.0;
		}

		// Stay on current path
		if ( bEndOfPath )
			return None;
		else
			return Next;
	}
	else	// Reverse
	{
		// Compute total chance used in this point
		if ( bEndOfPath )
		{
			fTotalChance = 0.0;
			for ( i=0; i<8; ++i )	// If main line is shut off, spread chance over all alternatives
				fTotalChance += Switches_Reverse[i].Chance;
		}
		else
			fTotalChance = 100.0;

		// Pick choice
		fRandChoice *= fTotalChance;
		for ( i = 0; i < 8; ++i )
		{
			if ( fRandChoice < Switches_Reverse[i].Chance )
				return Switches_Reverse_Priv[i].Next;

			fRandChoice -= Switches_Reverse[i].Chance;
			if ( fRandChoice < 0.0 )
				fRandChoice = 0.0;
		}

		// Stay on current path
		if ( bEndOfPath )
			return None;
		else
			return Prev;
	}
}

//
// When reach an interpolation point.
//
simulated event InterpolateEnd( InterpolationManager Other, bool bForward )
{
	local InterpolationPoint Dest;

	if( Pawn(Other.Owner) != none )
		if( !Pawn(Other.Owner).PawnAtInterpolationPoint( self, Other ) )
			return;

	TriggerEvent(Events[Other.PauseNum], self, Other.Owner.Instigator);
	if ( Pause[Other.PauseNum] > 0 )
	{
		Other.SetPause(Pause[Other.PauseNum]);
		Other.PauseNum++;
		Other.SetStartParameters();
		return;
	}

	Dest = DetermineNextDestination( bForward );

	Other.PauseNum = 0;
	Other.Dest = Dest;

	if ( Dest == None )
		Other.FinishedInterpolation(self);
	else
	{
		Other.SetStartParameters();
		if ( Dest.bInstantNextPath )
		{
			// move to dest instantly (location and rotation and any other effects)
			Other.InstantMove();
			Dest.InterpolateEnd(Other, bForward);	
		}
	}
}

defaultproperties
{
	 GameSpeed(0)=1
	 GameSpeed(1)=1
	 GameSpeed(2)=1
	 GameSpeed(3)=1
	 GameSpeed(4)=1
	 GameSpeed(5)=1
	 GameSpeed(6)=1
	 GameSpeed(7)=1
	 FovModifier(0)=1
	 FovModifier(1)=1
	 FovModifier(2)=1
	 FovModifier(3)=1
	 FovModifier(4)=1
	 FovModifier(5)=1
	 FovModifier(6)=1
	 FovModifier(7)=1
	 ScreenFlashScale(0)=1
	 ScreenFlashScale(1)=1
	 ScreenFlashScale(2)=1
	 ScreenFlashScale(3)=1
	 ScreenFlashScale(4)=1
	 ScreenFlashScale(5)=1
	 ScreenFlashScale(6)=1
	 ScreenFlashScale(7)=1
	 ScreenFlashFog(0)=(X=0,Y=0,Z=0)
	 ScreenFlashFog(1)=(X=0,Y=0,Z=0)
	 ScreenFlashFog(2)=(X=0,Y=0,Z=0)
	 ScreenFlashFog(3)=(X=0,Y=0,Z=0)
	 ScreenFlashFog(4)=(X=0,Y=0,Z=0)
	 ScreenFlashFog(5)=(X=0,Y=0,Z=0)
	 ScreenFlashFog(6)=(X=0,Y=0,Z=0)
	 ScreenFlashFog(7)=(X=0,Y=0,Z=0)
     bDirectional=True
	 Texture=S_Interp
	 bEndOfPath=False
	 bInstantNextPath=false
	 bConstantSpeed=true
	 bSmoothPath=true
	 DesiredSpeed=+150.0
	 Smoothing=+1.0
	 StartControlPoint=(X=200,Y=200,Z=0)
	 EndControlPoint=(X=-200,Y=-200,Z=0)
	 bFaceMoveDirection=true
	 bNewRotationSmoothing=true
	 tweenTime=0.2
}
