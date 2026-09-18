//=============================================================================
// TriggerWind.
//=============================================================================>
class TriggerWind expands Wind;

var() float ChangeTime;
var() bool bInitiallyOn;
var() bool bDelayFullOn; // Delay then go full-on.

var bool bWindOn;
var float InitialWindSpeed;
var float Alpha, Direction;

simulated function BeginPlay()
{
// Remember initial light type and set new one.
Disable( 'Tick' );
InitialWindSpeed = WindSpeed;
if( bInitiallyOn )
{
Alpha = 1.0;
Direction = 1.0;
} else {
WindSpeed = 0;
Alpha = 0.0;
Direction = -1.0;
}
}

// Called whenever time passes.
function Tick( float DeltaTime )
{
Alpha += Direction * (DeltaTime / ChangeTime);

if( Alpha > 1.0 )
{
Alpha = 1.0;
Disable( 'Tick' );
} else if( Alpha < 0.0 ) {
Alpha = 0.0;
Disable( 'Tick' );
}

WindSpeed = Alpha * InitialWindSpeed;
// log("WindSpeed "$(Alpha * InitialWindSpeed));

if( !bDelayFullOn )
WindSpeed = Alpha * InitialWindSpeed;
else if( (Direction>0 && Alpha!=1) || Alpha==0 )
WindSpeed = 0;
else
WindSpeed = InitialWindSpeed;
}


state() TriggerTurnsOn
{

function Trigger(Actor Other, Pawn EventInstigator)
{
Direction = 1.0;
Enable( 'Tick' );
}

Begin:

}

state() TriggerTurnsOff
{
function Trigger(Actor Other, Pawn EventInstigator)
{
Direction = -1.0;
Enable( 'Tick' );
}

Begin:
}


state() TriggerToggles
{
function Trigger(Actor Other, Pawn EventInstigator)
{
Direction *= -1;
Enable( 'Tick' );
}

Begin:
}

state() NormalWind
{

}

// Trigger controls the light.
state() TriggerControl
{
function Trigger( actor Other, pawn EventInstigator )
{
if( bInitiallyOn )
Direction = -1.0;
else
Direction = 1.0;
Enable( 'Tick' );
}

function UnTrigger( actor Other, pawn EventInstigator )
{
if( bInitiallyOn )
Direction = 1.0;
else
Direction = -1.0;
Enable( 'Tick' );
}
}

defaultproperties
{
}
