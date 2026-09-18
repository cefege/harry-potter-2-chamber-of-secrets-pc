//===============================================================================
//  OrangeSnail
//
//  The OrangeSnail moves along between patrol points at a slow speed, leaving
//  a poisonous trail behind.  
//
//  When Harry bumps into the snail, the snail's trail will retreat and the
//  snail will move a random distance in the opposite direction it was moving.
//
//  Bumping into the snail also causes Harry damage as does running into the
//  snail's trail.  The damage taken from the trail or the snail itself can be
//  different and is determined by constants in this class.
//
//  The snail is vulnerable to the Rictusempra spell, which causes the snail
//  to be stunned.  While stunned, the snail is vulnerable to Flipendo, which
//  will push the snail.
//
//  When the snail is patrolling, it will "ram" Harry if it can see him 
//  (determined by SightRadius, PeripheralVision, and EyeHeight properties).
//  Ramming means that the snail will move toward Harry's current location at a 
//  faster than normal speed.  The snail will keep this ram speed until it reaches
//  the location Harry was at when the ram was started, it hits Harry, or 
//  it hits a wall.  After this, the snail will go back to patrol mode.  The
//  snail won't attempt to ram Harry again until a few seconds have passed 
//  (determined by constants in this class).
//
//  The following properties are exposed to the level designer:
//
//		bAllowRam
//			Set to true if snail can cut Harry off.
//		
//      fGroundspeedNormal
//      fGroundspeedEcto
//      fGroundspeedRam
//      fGroundspeedEctoRam
//          Regular snail speed, snail speed in ecto, snail speed during normal ram, 
//          snail speed while ramming in ectoplasm.
//
//		nBumpRetreatMin  
//		nBumpRetreatMax   
//          When Harry hits the snail body, the snail will turn and go in the
//          opposite direction for a random distance between min and max.

//      fTrailDuration
//          Number of seconds a trail segment stays visible.
//
//      fTrailShrinkAfter
//          Start shrinking trail segments after this long.
//
//      nMaxTrailSegments
//          Limits the number of trail segments a trail can have.  Trail segments
//          at the end of the trail will disappear to keep the trail length 
//          under this limit (even if the segment's fTrailDuration has not
//          elapsed).  This keeps the trail from getting to long when the
//          snail's speed increases.
//
// 	     SightRadius
//       BaseEyeHeight
//       EyeHeight
//       PeripheralVision
//          The snail will ram Harry when it can see Harry.  These are base class
//          properties that determine when the snail can see Harry.
//
//===============================================================================

class orangesnail extends Characters;

const PATROL_BEFORE_RAM_TIME = 3.0;    // Patrol at least this long before ramming

const TRAIL_ARRAY_SIZE = 50;
var   SnailTrail arrayTrail[50];  // Array of trail segments.  Only nMaxTrailSegments
                                  // of this array will be used.  If nMaxTrailSegments
                                  // is bigger than the size here, the code will 
                                  // put up a warning and set nMaxTrailSegments to
                                  // the size of this array.

var        vector     vLastTrailSpawnLoc;        // Loc of last trail segment
var        int        nEctoplasmTouchCount;      // >0 when in ectoplasm
var        bool       bLeaveTrail;               // True to leave trail as move
var        int        nCurrTrailSlot;            // Position in arrayTrail
var        bool       bAllowSnailDamage;         // True if H can receive snail dam.
var        float      fTimeSinceSnailDamage;     // Time since received snail dam.
var        float      fPatrolTime;               // Time snail has been patrolling
var        vector     vTemp;                     // For temp calculations in states
var        bool       bCutInProgress;            // Cutscene is in progress-- don't attack
var        float      fStunTimeLeft;             // Time left to remain stunned

var        sound	  soundAttackCry;			 // Snail sounds accessed in more than
var        sound	  soundWarningCry;           // one place in the code

var(Snail) float      fGroundspeedNormal;        // Regular snail speed
var(Snail) float      fGroundspeedEcto;          // Snail speed in ectoplasm
var(Snail) float      fGroundspeedRam;           // Speed for normal ramming
var(Snail) float      fGroundspeedEctoRam;       // Ram speed in ectoplasm
var(Snail) bool       bAllowRam;                 // True to allow snail to cut H off
var(Snail) int        nBumpRetreatMin;           // Min dist to retreat when bump H
var(Snail) int        nBumpRetreatMax;           // Max dist to retreat when bump H
var(Snail) float      fTrailDuration;            // Lifetime for trail segment
var(Snail) float      fTrailShrinkAfter;         // Shrink trail seg after this time
var(Snail) int        nMaxTrailSegments;         // Snail will never have more than
                                                 // this many segments behind him--
                                                 // no bigger than TRAIL_ARRAY_SIZE

var(Snail) float      fTrailDamageWait;          // Wait this long after getting trail 
                                                 // damage before can receive more
var(Snail) int        nTrailDamage;              // Damage when run into snail trail
var(Snail) int        nNormalBodyDamage;         // Damage when run into snail body 
var(Snail) int        nRamBodyDamage;            // Damage when hit by snail body during ram 
var(Snail) float      fStunDuration;             // Length of time snail remains stunned
                                                 // after Rictusempra.         


//-----------------------------------------------------------------------------------
//  Functions
//-----------------------------------------------------------------------------------

function PostBeginPlay()
{
	SetTimer( RandRange(3.0, 8.0), false );

	if (nMaxTrailSegments > TRAIL_ARRAY_SIZE)
	{
		playerHarry.ClientMessage("Warning: Need to increase TRAIL_ARRAY_SIZE");
		log("Warning: Need to increase TRAIL_ARRAY_SIZE");

		nMaxTrailSegments = TRAIL_ARRAY_SIZE;
	}
}

event Tick(float fDeltaTime)
{
	Super.Tick(fDeltaTime);
	DoTrailUpdates(fDeltaTime);
    if (fStunTimeLeft > 0)
    {
        fStunTimeLeft -= fDeltaTime;
        if (fStunTimeLeft < 0)
            fStunTimeLeft = 0;
    }
}

// Snail is falling
event Falling()
{	
	Super.Falling();

	// Disable trail while falling
	EndTrail();
}

// Snail landed
event Landed(vector HitNormal)
{
	Super.Landed(HitNormal);

	// Start trail back up again if patrolling or moving toward Harry.
	if (IsInState('Patrol') || IsInState('RamHarry'))
		StartTrail();
}

// Set groundspeed to default values.  States can override this-- like when
// the snail is in ram mode, the speed is faster.
function SetGroundSpeed()
{
	if (nEctoplasmTouchCount >= 1)
		Groundspeed = fGroundSpeedEcto;
	else
		Groundspeed = fGroundspeedNormal;
}

// SnailTrail objects call this when they are stepped on.
function HarrySteppedOnTrail(vector vTrailLocation)
{
	DoSnailDamage('SnailTrail', vTrailLocation, IsInState('RamHarry'));
}

// Handle damage from snail body or trail.
function DoSnailDamage(name nameDamage, vector vDamageLoc, bool bCuttingHarryOff)
{
	local int nDamage;

	// No damage and bail out if cutscene is in progress.
	if (bCutInProgress)
		return;

	if (bAllowSnailDamage == true)
	{
		if (nameDamage == 'SnailTrail')
			nDamage = nTrailDamage;
		else
		{
			if (bCuttingHarryOff)
				nDamage = nRamBodyDamage;
			else
				nDamage = nNormalBodyDamage;
		}

		// Give harry damage
		playerHarry.TakeDamage( nDamage, self, vDamageLoc, Vect(0,0,0), nameDamage);

		if (nameDamage == 'SnailTrail')
		{
			switch( Rand(3) )
			{
			case 0:	PlaySound(sound'HPSounds.footsteps.HAR_acid_burn1');	break;
			case 1:	PlaySound(sound'HPSounds.footsteps.HAR_acid_burn2');	break;
			case 2:	PlaySound(sound'HPSounds.footsteps.HAR_acid_burn3');	break;
			}
			//PlaySound(soundOuch1, SLOT_none, RandRange(0.8, 1.0), [Pitch]RandRange(0.8, 1.1) );
		}
		else
			PlaySound(sound'HPSounds.critters_sfx.snail_ouch2', SLOT_none, RandRange(0.8, 1.0), [Pitch]RandRange(0.8, 1.1) );

		// Temporarily disable damage to trail so Harry can get away.  Tick()
		// will return this to true after some time has passed.
		bAllowSnailDamage = false;		
	}
}

function DoTrailUpdates(float fDeltaTime)
{
	local int      nTrailRadius;
	local int      nTrailHeight;
	local Rotator  rTrailRotation;
	local vector   vTrailLocation;

	nTrailRadius   = class'SnailTrail'.Default.CollisionRadius;

	// Spawn another trail obj. if the snail should be leaving behind trails
	// and the snail has advanced one trail segment since the last trail was left.
	if(bLeaveTrail == true && 
	   VSize(Location - vLastTrailSpawnLoc) > (nTrailRadius * 0.8))
	{
		rTrailRotation = class'SnailTrail'.Default.Rotation;

		// Trail location needs to be at snails "feet"
		vTrailLocation = Location;
		vTrailLocation.z -= CollisionHeight;

		// Spawn the new trail, save it's loc, set lifespan, and save parent
		if (arrayTrail[nCurrTrailSlot] == None)
		{
			arrayTrail[nCurrTrailSlot] = spawn(class'SnailTrail',,,vTrailLocation, rTrailRotation );
			arrayTrail[nCurrTrailSlot].SetSpawnProps(self, fTrailDuration, fTrailShrinkAfter);
		}
		// Debug stuff for seeing if trail slots are being reused before the 
		// trail segment has faded away.
		//else
		//{
		//	if (arrayTrail[nCurrTrailSlot].bHidden == false)
		//	{
		//		playerHarry.ClientMessage("trailslot in use " $nCurrTrailSlot);
		//		log("trailslot in use " $nCurrTrailSlot);
		//	}
		//}
		arrayTrail[nCurrTrailSlot].StartUsing(vTrailLocation);
		vLastTrailSpawnLoc = Location;

		// Set slot that next spawned trail will go in.  We just keep recycling
		// the slots.  Since SnailTrail's have a lifetime, they will be destroyed
		// automatically; however we keep an array of the SnailTrail's created so
		// we can force them to be destroyed early if the trail is retractied.
		nCurrTrailSlot++;
		if (nCurrTrailSlot >= nMaxTrailSegments)
			nCurrTrailSlot = 0;
	}

	// If trail damage is currently disabled, see if it's time to re-enable it.  
	// This is done so Harry doesn't get continual damage while getting damage.
	if (bAllowSnailDamage == false)
	{
		fTimeSinceSnailDamage += fDeltaTime;
		if (fTimeSinceSnailDamage >= fTrailDamageWait)
		{
			bAllowSnailDamage = true;
			fTimeSinceSnailDamage = 0.0;
		}
	}
}

// Spawn the snail trail.
function StartTrail()
{
	bLeaveTrail = true;
}

// Destroy the trail.
function EndTrail()
{
	local int i;

	bLeaveTrail = false;

	for (i=0; i < nMaxTrailSegments ; i++)
		arrayTrail[i].StopUsing();
}

// Snail was flipendo'd.  Let base class handle the flipendo push.  We'll
// pass control along to the push state, which will handle what to do
// when the flipendo action is complete.
function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{	
	local bool bReturn;

	Super.HandleSpellFlipendo(spell, vHitLocation);
	GoToState('Pushed');
	return (true);
}

// Snail was rictusempra'd.  Stun him.
function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{	
	local bool bReturn;

	EndTrail();
	Super.HandleSpellRictusempra( spell, vHitLocation);
	GoToState('Stunned');
	return (true);
}

function PlayerCutCapture()
{
	bCutInProgress = true;

	// If snail is currently ramming Harry, put him back on patrol.
	if (IsInState('RamHarry'))
		GoToState('Patrol');
}

function PlayerCutRelease()
{
	bCutInProgress = false;
}

// Snail hit something.
event Bump(Actor Other)
{
	// If it was Harry that was hit.
	if (playerHarry == Other)
	{
		// Take care of damage here so we can determine if snail is currently
		// ramming Harry (which is used to determine level of damage).
		DoSnailDamage('Snail', Location, IsInState('RamHarry'));

		// Handle bump stuff
		GoToState('BumpedHarryPart1');
	}
}

// Detect Ectoplasm touch.  We keep a touch/untouch count for ecto so we can 
// determine whether we're in ecto or out (needed when ecto objects overlap).
event Touch(Actor other)
{
	if (Ectoplasma(other) != None)
	{
		nEctoplasmTouchCount++;
		SetGroundSpeed();
	}
}

// Decrement ecto count when leave ecto.
event UnTouch(Actor other)
{
	if (Ectoplasma(other) != None)
	{
		nEctoplasmTouchCount--;

		// Shouldn't happen, but make sure don't go below 0
		if (nEctoplasmTouchCount <= 0)
			nEctoplasmTouchCount = 0;

		SetGroundSpeed();
	}
}

//-----------------------------------------------------------------------------------
//  States
//-----------------------------------------------------------------------------------

//-----------------------------------------------------------------------------------
//  State Patrol
//-----------------------------------------------------------------------------------
//
//  Patrol state is the default state.  While in the patrol state, the snail is 
//  moving between patrol points and leaving a trail.  

auto state Patrol
{
	// On tick, spawn new snail trail if snail has moved far enough.  Also keep
	// track of Harry's velocity and whether he should be cut off.
	event Tick(float fDeltaTime)
	{
		Global.Tick(fDeltaTime);

		fPatrolTime += fDeltaTime;

		//playerHarry.ClientMessage("From Harry: " $(VSize(playerHarry.Location - Location)));
		// If snail can see Harry, snail can ram Harry.
		if (bAllowRam           && 
			!bCutInProgress     && 
			CanSee(playerHarry) && 			
			(fPatrolTime >= PATROL_BEFORE_RAM_TIME))
		{
			GoToState('RamHarry');
		}
	}

	// Play slither sounds on timer
	function Timer()
	{
		SetTimer( RandRange(3.0, 8.0), false );

		if( Rand(2) == 0 )
			PlaySound(sound'HPSounds.critters_sfx.snail_slither1', SLOT_none, , [Pitch]RandRange(0.8, 1.1) );
		else
			PlaySound(sound'HPSounds.critters_sfx.snail_slither2', SLOT_none, , [Pitch]RandRange(0.8, 1.1) );
	}

	// Initial state properties.
	event BeginState()
	{		
		Super.BeginState();		
		//EndTrail();
		SetGroundSpeed();     // Set patrol ground speed
		StartTrail();         // Make sure trail is going when start patrol
		fPatrolTime = 0.0;
	}
}

//-----------------------------------------------------------------------------------
//  State Pushed
//-----------------------------------------------------------------------------------
//
//  This state is entered by being Flipendo'd.  When snail is flipendo'd, base 
//  class will handle "pushing" the snail (because we set bFlipPushable to true).  
//  When the base class push is done, we get a Landed event.  Once landed, the snail
//  goes to a recover state.

state Pushed
{
	ignores Bump;

	function Landed(vector HitNormal)
	{
		playerHarry.ClientMessage("Landed");
		Super.Landed(HitNormal);
		GoToState('RecoverFromPush');
	}

begin:
	PlayAnim('Idle');
}

//-----------------------------------------------------------------------------------
//  State RecoverFromPush
//-----------------------------------------------------------------------------------
//
//  Called by Push state once snail has landed.  Recover, then go on.

state RecoverFromPush
{
	ignores Bump;

begin:
	// Stop moving.
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	// Each time snail is flipendo'd while still recovering from being stunned, 
    // snail will loop and sleep in this state for the remaining stun time.  The 
    // snail may be forced out is this state if it is flipendoed before the stun
    // time is up.

	LoopAnim('stunned');
    if (fStunTimeLeft > 0)
	    Sleep(fStunTimeLeft);

	// The state is basically over.  Now vulnerable to Rictusempra again instead
	// of flipendo.  Reset flipendo stun time.
    eVulnerableToSpell = SPELL_Rictusempra;

	// Back to patrol
	GoToState('Patrol');
}

//-----------------------------------------------------------------------------------
//  State Stunned
//-----------------------------------------------------------------------------------
//
//  Snail can be stunned by Rictusempra spell.  While stunned, the snail can be
//  flipendo'd.

state Stunned
{
	ignores Bump;

begin:
    fStunTimeLeft = fStunDuration;

	// Now vulnerable to Flipendo
    eVulnerableToSpell=SPELL_Flipendo;

	// Stop moving.
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);

	//Spin
	PlayAnim('spin');       
	FinishAnim();

	// Sit there for a bit.
	LoopAnim('stunned');
	Sleep(fStunTimeLeft);

	// Back to being vulnerable to Rictusempra and back to patrol.
    eVulnerableToSpell = SPELL_Rictusempra;
	GoToState('Patrol');
}

//-----------------------------------------------------------------------------------
//  State RamHarry
//-----------------------------------------------------------------------------------

state RamHarry
{
	// Set speed to "ram" speed
	function SetGroundSpeed()
	{
		// If in ectoplasma 
		if (nEctoplasmTouchCount >= 1)
			GroundSpeed = fGroundspeedEctoRam;		

		// If not in ectoplasma
		else
			GroundSpeed = fGroundspeedRam;
	}

	// Make sure trail active while ramming.
	event BeginState()
	{
		Super.BeginState();
		StartTrail();
	}

	// If ram into a wall, forget about Harry and patrol again.
	event HitWall(vector HitNormal, actor HitWall )
	{
		GoToState('Patrol');
	}

	event EndState()
	{
		// Make sure sounds are stopped.
		StopSound(soundAttackCry, SLOT_none);
		StopSound(soundWarningCry, SLOT_none);
	}

begin:

	// Turn to Harry
	TurnToward(playerHarry);

	// Update snail ground speed.
	SetGroundSpeed();

	// Retract trail
	//EndTrail();

	// Play random attack sound
	if( Rand(2) == 0 )
		PlaySound(soundAttackCry, SLOT_none, RandRange(0.8, 1.0), [Pitch]RandRange(0.8, 1.1) );
	else
		PlaySound(soundWarningCry, SLOT_none, RandRange(0.8, 1.0), [Pitch]RandRange(0.8, 1.1) );

	// Start the trail up again.
	//StartTrail();

	// Move to cut Harry off	
	MoveTo(playerHarry.Location);

	// When get to the destination, pause for a moment.
	PlayAnim('idle');
	Sleep(0.5);

	GoToState('Patrol');
}

//-----------------------------------------------------------------------------------
//  State BumpedHarryPart1
//-----------------------------------------------------------------------------------
//
//  Part 1 of the sequence to play out when Harry bumps into the snail.  The Bump
//  sequence is divided into 2 states so the snail can move a little bit in a 
//  state that ignores bumps and then move the rest of the way in a state that
//  will let bumping resume.  This is so we don't get continual bumps because Harry
//  ran into the snail and then stopped-- give a little time for the snail to move
//  away before bumping again.

state BumpedHarryPart1
{
	ignores Bump;

	// Get dest location a small distance in opposite direction snail was going.
	function vector GetAboutFaceVector()
	{
		local vector vDest;

		vDest = Velocity;
		vDest = normal( vDest );
		vDest *= -15;
		vDest = Location + vDest;
		return (vDest);
	}

begin:
	// Get vector in opposite direction
	vTemp = GetAboutFaceVector();

	// Retract trail
	EndTrail();

	// Take damage
	//DoSnailDamage('SnailTrail', Location);

	// Start trail again as snail is moving in the opposite dir
	StartTrail();

	// Move a little in the opposite dir
	MoveTo(vTemp);

	// Go to part 2 and move the rest of the way in the opposite dir.
	GoToState('BumpedHarryPart2');
}

//-----------------------------------------------------------------------------------
//  State BumpedHarryPart2
//-----------------------------------------------------------------------------------
//
//  Part 2 of the sequence to play out when Harry bumps into the snail.  In part 1,
//  the snail turned around and moved a little ways away.  In part 2, the snail
//  will move the rest of the way (distance is a random value between a range setup
//  in the snail properties).

state BumpedHarryPart2
{
	// Calculate dest.  Same direction the snail is currently going- distance
	// is random within a range.
	function vector GetDestVector()
	{
		local vector vDest;
		local int    nRetreatScalar;

		vDest = normal(Velocity);
		nRetreatScalar = RandRange(nBumpRetreatMin, nBumpRetreatMax);
		playerHarry.ClientMessage("Bump Retreat: " $nRetreatScalar);
		vDest *= nRetreatScalar;
		vDest = Location + vDest;
		return (vDest);
	}

begin:

	// Move to the new spot
	MoveTo(GetDestVector());

	// Go back on patrol
	GoToState('Patrol');
}

defaultproperties
{
     bAllowSnailDamage=True
     fGroundspeedNormal=25
	 fGroundspeedEcto=10
     fGroundspeedRam=100
     fGroundspeedEctoRam=20
     bAllowRam=True
	 SightRadius=600
     BaseEyeHeight=30
     EyeHeight=30
	 PeripheralVision=0.40
     nBumpRetreatMin=100
     nBumpRetreatMax=200
     bFlipPushable=True
     fFlipPushForceZ=100
     GroundSpeed=25
     WalkAnimName=locomotion
	 RunAnimName=locomotion
	 eVulnerableToSpell=SPELL_Rictusempra
     Mesh=SkeletalMesh'HPModels.skorangesnailMesh'
     DrawScale=3
     AmbientGlow=110
     CollisionRadius=30
     CollisionHeight=30
     fTrailDuration=9.0
	 fTrailShrinkAfter=4.5
	 nMaxTrailSegments=40
	 RotationRate=(Pitch=10500,Yaw=10500,Roll=10500)
	 soundFalling(0)=Sound'HPSounds.Critters_sfx.Snail_Falling'
	 soundFalling(1)=Sound'HPSounds.Critters_sfx.Snail_Falling2'
	 soundAttackCry=sound'HPSounds.critters_sfx.snail_attack_cry'
	 soundWarningCry=sound'HPSounds.critters_sfx.snail_warning_cry'
	 bDoEyeBlinks=false
     bThrownObjectDamage=True
     fTrailDamageWait=2
     nTrailDamage=10
     nRamBodyDamage=25
     nNormalBodyDamage=5
     fStunDuration=30
}
