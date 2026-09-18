//=============================================================================
// InterpolationManager
// Spawned whenever an actor is interpolating
//=============================================================================
class InterpolationManager extends Info
	native;

var InterpolationPoint Dest;				// Next Interpolation point
var InterpolationPoint Last;				// Interpolation point traveling from
var float       PhysAlpha;			// Interpolating position, 0.0-1.0.
var float       PhysRate;			// Interpolation rate per second.
var	float		RemainingPause;	
var float		StartPause;	
var int PauseNum;					// current paused offset in interpolationpoint
var float OldGameSpeed;
var float OldFOVModifier;
var float OldFlashScale;
var vector OldFlashFog;
var color OldFogColor;
var float OldFogStart;
var float OldFogEnd;
	
var vector TurnRateX, TurnRateZ;	// turning rate (smoothed) in FCoords representation
var vector OldDesiredX, OldDesiredZ;// Old desired rotation (used to determine desired rotation rate)
var bool bInstantMove;

function PostBeginPlay()
{
	super.PostBeginPlay();

	//Set our Owner's TickParent to the ourself so we get ticked before the object we're moving
	if( Owner != none )
		Owner.TickParent = self;
}

function Destroyed()
{
	//What ever.  You call destroy, then this gets called, but for some reason, owner doesn't get set to none right away,
	// so the phys_interpolating messes with the actor he's controlling, and makes him point an odd direction.
	SetOwner( none );
}
	
function SetPause(float F)
{
	RemainingPause = F;
	StartPause = F;
}
		 
function Init(InterpolationPoint D, Float Rate, bool bJumpToStart)
{
	Dest = D;
	PhysRate = Rate;
	PhysAlpha = 0.0;
	RemainingPause = D.Pause[0];
	PauseNum = 0;
	if ( bJumpToStart && (Dest.Prev != None) )
	{
		Dest = Dest.Prev;
		InstantMove();
		bInstantMove = false;	// didn't happen during interpolation physics
		Dest = Dest.Next;
	}
	SetStartParameters();
}

function FinishedInterpolation(InterpolationPoint Other)
{
	Owner.FinishedInterpolation(Other);
	Destroy();
}

function InstantMove()
{
	local vector  Y;
	local Rotator R;

	R = Rotator(Dest.StartControlPoint);

	Owner.SetLocation(Dest.Location);
	Owner.SetRotation( R ); //Dest.Rotation );
	if ( Owner.IsA('Pawn') )
		Pawn(Owner).ClientSetLocation(Dest.Location, R);
	GetAxes(Owner.Rotation,OldDesiredX,Y,OldDesiredZ);
	TurnRateX = vect(0,0,0);
	TurnRateZ = vect(0,0,0);
	UpdateCamera(1);
	bInstantMove = true;
}

simulated function SetStartParameters()
{
//	local PlayerPawn P;
	local Actor A;

//	OldGameSpeed = Level.TimeDilation;
//	if ( Pawn(Owner) != None )
//		P = PlayerPawn(Owner);
//	if ( P == None )
//		return;

//	OldFOVModifier = P.FOVangle/P.Default.FOVangle;
//	OldFlashScale = P.FlashScale.X;
//	OldFlashFog = P.FlashFog;
//	OldFogColor = Region.Zone.DistanceFogColor;
//	OldFogStart = Region.Zone.DistanceFogStart;
//	OldFogEnd = Region.Zone.DistanceFogEnd;
}
	
simulated event UpdateCamera(float Pct)
{
//	local PlayerPawn P;

	
//	if ( Pawn(Owner) != None )
//		P = PlayerPawn(Owner);
//	if ( P == None )
//		return;

//	if ( Dest.NonLinearEffect[PauseNum] == 1 )
//		Pct = sqrt(Pct);

//	Level.TimeDilation = Pct * Dest.GameSpeed[PauseNum] + (1-Pct) * OldGameSpeed;
//	P.DesiredFlashScale = Pct * Dest.ScreenFlashScale[PauseNum] + (1-Pct) * OldFlashScale;
//	P.FlashScale = vect(1,1,1) * P.DesiredFlashScale;
//	P.DesiredFlashFog = Pct * Dest.ScreenFlashFog[PauseNum] + (1-Pct) * OldFlashFog;
//	P.FOVangle = P.Default.FOVAngle * ( Pct * Dest.FOVModifier[PauseNum] + (1-Pct) * OldFOVModifier );

//	if ( Dest.DistanceFogStart != 0 )
//		Region.Zone.DistanceFogStart = Pct * Dest.DistanceFogStart + (1-Pct) * OldFogStart;
//	if ( Dest.DistanceFogEnd != 0 )
//		Region.Zone.DistanceFogEnd = Pct * Dest.DistanceFogEnd + (1-Pct) * OldFogEnd;
//	if ( Dest.DistanceFogColor.R + Dest.DistanceFogColor.G + Dest.DistanceFogColor.B > 0 )
//	{
//		Region.Zone.DistanceFogColor.R = Pct * Dest.DistanceFogColor.R + (1-Pct) * OldFogColor.R;
//		Region.Zone.DistanceFogColor.G = Pct * Dest.DistanceFogColor.G + (1-Pct) * OldFogColor.G;
//		Region.Zone.DistanceFogColor.B = Pct * Dest.DistanceFogColor.B + (1-Pct) * OldFogColor.B;
//	}
}

defaultproperties
{
	Physics=PHYS_Interpolating
}