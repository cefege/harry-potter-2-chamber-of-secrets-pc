//===============================================================================
//  SnailTrail 
//
//  The SnailTrail objects are used in conjunction with the OrangeSnail.  As the
//  OrangeSnail moves along, he spawns SnailTrail objects.  The SnailTrail
//  objects are given a lifetime causing them to disappear after that period
//  of time.
//
//  The SnailTrail has a reference to the parent OrangeSnail (set by the snail
//  when he spawns a trail object.  The trail passes touch and destroy 
//  information back to the snail using this reference.
//
//===============================================================================

class SnailTrail extends HProp;

// Props the orangesnail sets
var OrangeSnail parentSnail;    // Reference back to snail itself.  Setup in
                                // the snail after this trail is spawned.
var float fTrailDuration;		// Trail stays visible this many seconds
var float fShrinkAfter;         // Start shrinking the trail after this many secs

// Internal props
var float fStartDrawScale;      // Draw scale is initially this
var float fCountDown;           // Countdown how long the snails been around

// Props snail sets when trail spawned.  fTrailDuration and fShrinkAfter are
// exposed to this function so the designer can change trail props through
// the snail object.
function SetSpawnProps(OrangeSnail parentInSnail, float fInTrailDuration, float fInShrinkAfter)
{
	parentSnail    = parentInSnail;
	fTrailDuration = fInTrailDuration;
	fShrinkAfter   = fInShrinkAfter;
}

// Trail segments get reused.  Snail calls this when it wants to re-use a
// trail segment at a different location.
function StartUsing(vector vLoc)
{
	fCountdown = fTrailDuration;   // Set countdown to lifetime of trail
	DrawScale = fStartDrawScale;   // Set initial draw scale
	SetCollision(true);            // Harry needs to collision detection on trail
	SetLocation(vLoc);             // trail placement
	bHidden = false;               // show trail
}

// Stop using this trail segment
function StopUsing()
{
	bHidden = true;                // Hide trail when not using
	SetCollision(false);           // Don't collide when not using
}

// Stop using this trail segment when fTrailDuration is up.  Also have
// the segment shrink over time.
event Tick(float fDelta)
{	
	if (fCountDown <= 0)
		StopUsing();
	else if (bHidden == false)
	{
		// After fShrinkAfter seconds, start shrinking the trail.
		if ((fTrailDuration - fCountDown) >= fShrinkAfter)
		{
			// Shrink consistently over the remaining time.
			DrawScale = (fCountdown / (fTrailDuration - fShrinkAfter)) * fStartDrawScale;
		}

		// Continue countdown
		fCountDown -= fDelta;
	}
}

// Waiting for Harry to come step on the poisonous trail.
auto state ComeStepOnMeHarry
{

	function Touch(actor other)
	{
		Super.Touch(other);

		// Nothing to do if Harry's not the one touching.
		if( harry(other) == None)
			return;

		// Let parent snail handle damage for stepping on trail.  Mostly we're
		// letting the snail handle the damage so that it can keep other
		// trail segments from damaging Harry momentarily.
		parentSnail.HarrySteppedOnTrail(Location);
	}
}

defaultproperties
{
     fTrailDuration=2
     fCountDown=2
	 fShrinkAfter=1
     Rotation=(Roll=-16320)
     Style=STY_Translucent
     Mesh=SkeletalMesh'HProps.skSheetTestMesh'
     DrawScale=0.1
	 fStartDrawScale=0.1
     bUnlit=True
     MultiSkins(0)=FireTexture'HPParticle.hp_fx.Particles.Silverslime'
     CollisionRadius=10
     CollisionHeight=5
     bCollideWorld=False
     bBlockActors=False
     bBlockPlayers=False
	 bBlockCamera=false
}
