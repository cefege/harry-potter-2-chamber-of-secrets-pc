class SavePoint extends HPawn;

var bool bActive;

var() bool bSaveOnce;	// if true this book will destroy itself once it is used.

var vector   vLoc;
var float    fBobAmount;

function Tick(float dtime)
{
	super.Tick(dtime);
	SetLocation( vLoc + (vec( fBobAmount * 0.2 * Cos( Level.TimeSeconds * 1 * 3), 0, fBobAmount * Sin( Level.TimeSeconds * 2 * 3) ) >> Rotation) );
}

function PostBeginPlay()
{
	Super.PostbeginPlay();
	
	// on startup the save point should always be inactive
	bActive = false;
	SetTimer( 5.0f , true );

	vLoc = location;
}

function Timer()
{
	if( VSize2d(playerHarry.Location - Location) > 100 )
		bActive = true;
	
	// DEBUG
//	playerHarry.ClientMessage("" $self $" timer" );
}

function OnSaveGame()
{
	// We are no longer active
	bActive = false;
	
	// Destroy ourselves if we only save once
	if( bSaveOnce )
		destroy();
	
	// --- Save our game ( after we called destroy, so we don't save that we have a save point )
    playerHarry.SaveGame(0);
}


function Touch( actor other )
{
	playerHarry.ClientMessage("" $self $" touch, other: " $other );

	if( other == playerharry && bActive )
	{
		OnSaveGame();
	}
}

defaultproperties
{
	// --- Save point
	bSaveOnce=true
	
	// ---
	bStatic=False
	Rotation=(Pitch=16384)
	DrawType=DT_Mesh
	Texture=Texture'Engine.S_Pawn'
	Mesh=SkeletalMesh'HPModels.SavePointFloatBookMesh'
	AmbientGlow=75
	
	bCollideActors=true
	bCollideWorld=false
	bBlockActors=false
	bBlockPlayers=false

	fBobAmount=10
}
