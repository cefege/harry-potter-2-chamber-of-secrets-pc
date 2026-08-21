//class Aragog extends HChar;
class Aragog extends baseBoss;

// variables

// general
var vector vDir;
var int counter;
var int iterations;
var int cutSceneCounter;
var AragogHome aHome;
var vector vHome;
var bool bMove;
var vector vLocation;
var float distanceFromHarry;
var baseCam cam;
var vector vBiteVector;
var bool bOnMyWeb;
var bool bOnStickyWeb;
var rotator rRotationTowardHarry;
var int	attackType;
var bool bPhaseOneOver;
var float tempDamage;
var bool bCanBiteHarry;
var AragogCenterWeb centerWeb;
var vector nipLocation;
var bool bBigBite;
var int countI;
var int countJ;

// rotation
var rotator vRot;
var float pitch;
var float changeRotation;
var rotator moveToRotation;
var float rotationDirection;
var rotator rLastRotation;

// anchors
var int numAnchorsAttached;
var AragogWebAnchor fixingAnchor;
struct anchors
{
	var AragogWebAnchor anchor;
	var float HitTime;
};
var anchors webAnchors[8];
var int aBefore, aAfter;

// general spell 
var float attackTime;

// web Spell
var AragogSpellWeb webSpell;
var AragogStickyWeb stickyWeb[7];
var int		maxWebs;
var vector spellOrigin;

// attack Spell
var AragogSpellAttack attackSpell;
var vector spellLocation;


// spider Spawner (there will be more of these
var Trigger	 SpiderTrigger[4];
var SpiderMarker  marker;

// sound variables
var Sound aragogVoice;			// Aragogs voice
var string  sSoundID;			// The name of the sound file
var float   randDialog; // The amount of time between random dialog

// editor variables
var float minTimeBetweenHits;
var() int numAnchors;
var() int spiderAnchorRatio;	// The number of anchors that need to be hit before incrementing the number of spiders in a particular marker
var() int largeSpiderIncrement;	// The number of large spiders to increment by as anchors get hit
var() int smallSpiderIncrement; // The number of small spiders to increment by as anchors get hit
var() float randDialogInterval; // The amount of time between random dialog
var() float BiteStartFrame;		// The frame where the bite anim should start doing damage to Harry
var() float BiteEndFrame;		// The frame where the bite anim should stop doing damage to Harry
var   float RearUpTime;			// The amount of time Aragog spends in a reared up position (vulnerable)
//var() float RearUpTimeDecrement;// The amount of time to subtract from RearUpTime with each hit
var() float RearUpTimeStart;    // Time with full health
var() float RearUpTimeEnd;      // Time with no health
var() int   maxAttendentsInLevel;// maximum number of attendents in level 

var() float StompAnimRateStart;
var() float StompAnimRateEnd;

var() float SpitAnimRateStart;
var() float SpitAnimRateEnd;

var() float TimeBetweenSPELL_LINEShotsStart;
var() float TimeBetweenSPELL_LINEShotsEnd;

var() float WebLifetime;		// How long does the web stick around

var() float timeBetweenAttacks; // The amount of time between each attack sequence
//var   float timeBetweenAttacksDecrement;  // The amount of time to subtract with each hit
var() float damageVulnerable; // The amount of damage done while Aragog is vulnerable
var() float damageNormal;	  // The amount of damage done at all other times. 
var() float WebDamageTimer;	  // The amount of time between damage hit while Harry is walking on the web
var() int   WebDamage;		  // The amount of damage to Harry each hit for walking on the web
var() float SpellDamage;	  // The amount of damage to Harry when the spell that spawns the web hits him
var() int   SpellSpraySpreadAmount;//The angle inbetween each of the spray spells.
var() float BiteDamage;		  // The amount of damage to Harry when he's bitten
var() float BigBiteDamage;	  // The amount of damage to Harry if he's bitten for actually touching Aragog's collision cylinder
var() float webCollisionRadius; // The collision radius of the sticky web that Aragog shoots.
								// If there is a web within collision radius of Harry the next web will not land on top of that one
var() int	numberOfSpells;		// The number of spells to shoot at the beginning of the game. One is usual. MAX NUMBER OF SPELLS IS THREE!
var() int	HealthAddSpellTwo;	// The health of Aragog when you add the second spell
var() int	HealthAddSpellThree;// The health of Aragog when you add the third spell

enum enumSpellAttackType
{
	SPELL_DIRECT,		// the spells are aimed directly at Harrys head
	SPELL_SPRAY,		// the spells are aimed in a spray pattern around Harry
	SPELL_LINE,			// the spells are aimed in a line in front of Harry
};

var enumSpellAttackType spellType;

// constants

const CHECK_ROTATION = 5;
const ATTACK_BITE = 0;
const ATTACK_SPELL = 1;

var  float   TempFloat;
var  float   TempTime;
var  int     TempInt;
var  int     TempCount, TempCount2;
var  Rotator TempRotator;
var  vector  TempVector;

function postBeginPlay()
{
	local AragogWebAnchor a;
	local int triggerCounter;
 
	// Set Harry to be very afraid. So he will play his look_frantic anim
	playerHarry.bVeryAfraid = true;

	Super.PostBeginPlay();

	changeRotation = CHECK_ROTATION;

	numAnchorsAttached = numAnchors;
	randDialog =  randDialogInterval;

	vHome = location;

	bPhaseOneOver = false;

	// Spawn all of the sticky webs that will be needed
	for ( counter = 0; counter < maxWebs; counter++ )
	{
		stickyWeb[counter] = spawn(class'AragogStickyWeb',self,,location+vec(50*counter,0,0),rot(0,0,0));
		stickyWeb[counter].fDamageTimer = WebDamageTimer;
		stickyWeb[counter].iDamage = WebDamage;
	}


	foreach allActors(class'AragogHome', aHome)
			break;
	vHome = location;
	

	foreach AllActors( class'AragogWebAnchor', a )
	{
		webAnchors[a.iLocation].anchor = a;
		webAnchors[a.iLocation].HitTime = -1;
	}

	 RearUpTime = RearUpTimeStart;

}

function BeatBoss()
{
	Health = 0;
	HandleSpellRictusempra( );
}

function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{
	local bool  bPlayFoiledSound;

	Super.HandleSpellRictusempra( spell, vHitLocation );

	if( AnimSequence == 'rearsUpLoop' )
	{
		tempDamage = damageVulnerable;

		bPlayFoiledSound = true;

		// Decrement the amount of time Aragog spends in a vulnerable position (RearedUp)
		//RearUpTime -= RearUpTimeDecrement;
	}
	else
	{
		tempDamage = 1;//damageNormal;


		// Decrement the amount of time Aragog spends in a vulnerable position (RearedUp)
		//RearUpTime -= RearUpTimeDecrement / (damageVulnerable/DamageNormal);
	}

	// make sure it never gets to be less than zero
	//if ( RearUpTime < RearUpTimeDecrement )
	//	RearUpTime = RearUpTimeDecrement;

	Health -= tempDamage;

	if( Health > 0 )
	{
		if( bPlayFoiledSound )
			PlayFoiledSound();
		else
			PlayOuchSound();
	}

	//Set RearUpTime based on health
	RearUpTime = RearUpTimeStart + (RearUpTimeEnd-RearUpTimeStart)*(1-GetHealth());

	playerHarry.ClientMessage("******* Gog hit by spell.  AnimSequence:"$AnimSequence$" tempDamage:"$tempDamage$" Health:"$Health);

	//end the game if it's over
	if( Health <= 0 )
	{
		Health = 0;
		gotoState('stateBeatAragog');
	}
	else
	{
		if ( Health <= HealthAddSpellThree )
		{
			numberOfSpells = 3;
		}
		else if ( Health <= HealthAddSpellTwo ) 
		{
			numberOfSpells = 2;
		}

		//if ( IsInState('stateHarryHunting') ||
		//	 IsInState('statePhysicalAttack') ||
		//	 IsInState('stateGoHome') ||
		//	 IsInState('stateSpellAttack') )
		if( TempDamage == damageVulnerable )
		{
			gotoState('stateHitByRictusempra');
		}
	}
	
	return true; // true == create spell effects
}


function PlayOuchSound()
{
// WAS "HPSounds.PC_PVS_Chal2Skurge_22"

	switch( Rand(6) )
	{
		case 0:	sSoundID = "PC_ARA_ArragogFight_02a"; break;
		case 1:	sSoundID = "PC_ARA_ArragogFight_02b"; break;
		case 2:	sSoundID = "PC_ARA_ArragogFight_02c"; break;
		case 3:	sSoundID = "PC_ARA_ArragogFight_03a"; break;
		case 4:	sSoundID = "PC_ARA_ArragogFight_03b"; break;
		case 5:	sSoundID = "PC_ARA_ArragogFight_03c"; break;
	}

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	aragogVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( aragogVoice, SLOT_Talk, 256,[Radius]1000000); 

}

function PlayRandomSound()
{

	switch( Rand(6) )
	{
		case 0:	sSoundID = "PC_ARA_ArragogFight_04a"; break;
		case 1:	sSoundID = "PC_ARA_ArragogFight_04b"; break;
		case 2:	sSoundID = "PC_ARA_ArragogFight_04c"; break;
		case 3:	sSoundID = "PC_ARA_ArragogFight_05"; break;
		case 4:	sSoundID = "PC_ARA_ArragogFight_06"; break;
		case 5:	sSoundID = "PC_ARA_ArragogFight_07"; break;
	}

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	aragogVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( aragogVoice, SLOT_Talk, 256,[Radius]1000000); 

}

function PlayMinionSound()
{
	switch( Rand(4) )
	{
		case 0:	sSoundID = "PC_ARA_ArragogFight_10"; break;
		case 1:	sSoundID = "PC_ARA_ArragogFight_11"; break;
		case 2:	sSoundID = "PC_ARA_ArragogFight_12"; break;
		case 3:	sSoundID = "PC_ARA_ArragogFight_13"; break;
	}

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	aragogVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( aragogVoice, SLOT_Talk, 256,[Radius]1000000); 
}

function PlayAttackingSound()
{

	switch( Rand(6) )
	{
		case 0:	sSoundID = "PC_ARA_ArragogFight_16a"; break;
		case 1:	sSoundID = "PC_ARA_ArragogFight_16b"; break;
		case 2:	sSoundID = "PC_ARA_ArragogFight_16c"; break;
		case 3:	sSoundID = "PC_ARA_ArragogFight_17a"; break;
		case 4:	sSoundID = "PC_ARA_ArragogFight_17b"; break;
		case 5:	sSoundID = "PC_ARA_ArragogFight_17c"; break;
	}

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	aragogVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( aragogVoice, SLOT_Talk, 256,[Radius]1000000);  
}

function PlayFoiledSound()
{

	switch( Rand(3) )
	{
		case 0:	sSoundID = "PC_ARA_ArragogFight_15a"; break;
		case 1:	sSoundID = "PC_ARA_ArragogFight_15b"; break;
		case 2:	sSoundID = "PC_ARA_ArragogFight_15c"; break;
	}

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	aragogVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( aragogVoice, SLOT_Talk, 256,[Radius]1000000); 

}

function PlayConfidentSound()
{
	switch( Rand(3) )
	{
		case 0:	sSoundID = "PC_ARA_ArragogFight_14a"; break;
		case 1:	sSoundID = "PC_ARA_ArragogFight_14b"; break;
		case 2:	sSoundID = "PC_ARA_ArragogFight_14c"; break;
	}

	//to localize get the string that corrosponds to the sSoundID. 
	Localize( "all",sSoundID,"HPdialog" );

	//get the sound that corrosponds to the sSoundID.
	aragogVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );

	PlaySound( aragogVoice, SLOT_Talk, 256,[Radius]1000000); 
}



// Returns 1 or -1 depending on what direction you should turn to get to the requested rotation
function float SetRotationDirection(float y)
{
	local float currentYaw;
	local float requestedYaw;
	local float newYaw, temp;
	local float delta;
	local float dir;

	currentYaw = rotation.yaw;
	currentYaw = currentYaw & 0x0000ffff;
	requestedYaw = y;
	requestedYaw = requestedyaw & 0x0000ffff;

	if ( currentYaw > requestedYaw )
	{
		dir = -1;
		newYaw = currentYaw - requestedYaw;
		if ( currentYaw - requestedYaw > 32768 )
		{
			dir = 1;
			newYaw = ( (65536 - currentYaw) + requestedYaw );
		}
	}
	else
	{
		dir = 1;
		newYaw = requestedYaw - currentYaw;
		if ( requestedYaw - currentYaw > 32768 )
		{
			dir = -1;
			newYaw = ( (65536 - requestedYaw) + currentYaw );
		}
	}

	temp = newYaw / 50;

	if ( temp < 50 )
	{
		temp = newYaw / 25;
	}

	dir = temp * dir;

	return dir;

}

/*
function SpawnSomeSpiders()
{
	local int counter;
	local float dist, farthestDist;
	local SpiderSpawner ss;

	farthestDist = 99999999999;

	for ( counter=0; counter<4; counter++ )
	{
		if ( Vsize(spawner[counter].location - playerHarry.location) < farthestDist )
		{
			ss = spawner[counter];
			farthestDist = Vsize(spawner[counter].location - playerHarry.location);
		}
	}

	TriggerEvent( ss.Event, None, None );

}
*/

function PlayerCutCapture()
{
	gotoState('CutIdle');
}

function PlayerCutRelease()
{
	if ( cutSceneCounter == 1 )
	{
		gotoState('PhaseOne');
	}
	else if ( cutSceneCounter == 2 )
	{
		bPhaseOneOver = true;
		bOnMyWeb = false;
//		vHome = location;
		attackTime = timeBetweenAttacks;
		centerWeb = spawn(class'AragogCenterWeb',self,,location+vect(0,0,-25),rot(0,0,0));
		centerWeb.SetLocation( location+vect(0,0,-25) );
		//playerHarry.SpellCursor.SetLOSDistance( 1000 );

		playerHarry.GroundRunSpeed = playerharry.default.GroundRunSpeed * 1.25;
		playerHarry.GroundSpeed = playerHarry.GroundRunSpeed;

		gotoState('stateHarryHunting');
	}
}

function bool IsAttacking()
{
	local bool ret;

	ret = true;

	if ( IsInState('stateHarryHunting') ||
		 IsInState('stateGoHome') )
	{
		ret = false;
	}

	return ret;

}

function Touch(actor other)
{
	// Harry has touched Aragog. That's pretty hard to do since Aragog's collision radius is tiny.
	// You would have to be inside him. That's ballsy, so let's reward the little nipper :)
	if ( other.IsA('Harry') )
	{
		gotoState('NipHarry');
	}
}

function Tick(float DeltaTime)
{

	Super.Tick(DeltaTime);

	// Check if Harry is on Aragog's web
	if ( bPhaseOneOver == true && IsAttacking() == false)
	{
		if ( bOnMyWeb == true )
		{ 
			bOnMyWeb = false;
			gotoState('stateTurnToBite');
		}

		/*
		if ( bOnStickyWeb == true )
		{
			bOnStickyWeb = false;

			// Play the confident chuckle randomly
			if ( rand(3) == 0 )
			{
				PlayConfidentSound();
			}

			vDir = normal(playerHarry.location - location);
			vRot = rotator(playerHarry.location - location);

			DesiredRotation = vRot;
			SetRotation(vRot);

			gotoState('statePhysicalAttack');
		}
		*/
	}

	
	// If you're in PhaseOne. Check if you should yell at your kids
	if ( IsInState('PhaseOne') )
	{
		randDialog -= DeltaTime;

		if ( randDialog <= 0 )
		{
			// play a random dialog. Aragog calling to her children
			PlayMinionSound();
			randDialog =  randDialogInterval;
			gotoState('PhaseOne');
		}
	}

	// Decrement the attack counter
	attackTime -= DeltaTime;

	// Set Harry's rotation from Aragog
	rRotationTowardHarry = rotator((playerHarry.location)-location);

	// Check if it's time to attack. Only check in stateHarryHunting because
	// I want Aragog to be back in the center before attacking again
	if ( IsInState('stateHarryHunting') )
	{
		if ( attackTime <= 0 )
		{

			attackTime = timeBetweenAttacks;
			if ( attackType == 0 )
			{
				attackType = 1;
			}
			else
			{
				attackType = 0;
			}

			switch (attackType)
			{
			case ATTACK_BITE:
				gotoState('statePhysicalAttack');
				break;
			case ATTACK_SPELL:
				gotoState('stateSpellAttack');
				break;
			default:
				gotoState('statePhysicalAttack');
				break;
			}

		}
		else
		{
			// Check if Harry is more than 22.5 degrees away from Aragogs rotation. If so rotate him
			// Randomly play a dialog
			if ( abs((rRotationTowardHarry.yaw & 0x0000ffff) - (rotation.yaw & 0x0000ffff)) > 4096 ||
				abs((rRotationTowardHarry.yaw & 0x0000ffff) - (rotation.yaw & 0x0000ffff)) > 61439 )
			{
				if ( rand(3) == 0 )
				{
					PlayRandomSound();
				}

				loopAnim('rotate', 1.0, 0.2);
				rRotationTowardHarry.pitch = 0;//rotation.pitch;
				rRotationTowardHarry.roll = rotation.roll;

				DesiredRotation = rRotationTowardHarry;
				rLastRotation = rRotationTowardHarry;
			}
		}
	}

	// check if you should stop rotating to face Harry
	if ( rLastRotation != rot(0,0,0) )
	{
		if ( rLastRotation == rotation )
		{
			loopAnim('idle', 1.0, 0.2);
			rLastRotation = rot(0,0,0);
		}
		else
		{
			rLastRotation = rotation;
		}
	}
	
	// Check if it's time to move forward toward Harry
	if ( IsInState('stateMoveToAttack') )
	{
		if ( bMove == true )
		{
			MoveSmooth( vDir * groundSpeed * DeltaTime);
			if ( vSize2D(location - vHome) > 500 )
			{
				gotoState('stateGoHome');
			}

			if ( vSize2D(location - playerHarry.location) < 390+playerHarry.collisionRadius )
			{
				gotoState('stateBiteHarry');
			}
		}

	}

	// Check if the bite sequence is in the right frames to bite Harry and check if
	// Aragog is close enough to Harry to cause damage.
	// This is where biting damages Harry!
	if ( IsInState('stateBiteHarry') )
	{
		if(   AnimSequence == 'bite'
		   && AnimFrame >= BiteStartFrame  &&  AnimFrame <= BiteEndFrame
		   && bCanBiteHarry == true)
		{
			vBiteVector = location + (vector(rotation) * (390));

			if ( Vsize2D(vBiteVector - playerHarry.location) < 40 )
			{
				bCanBiteHarry = false;
				
				if ( bBigBite == true )
				{
					bBigBite = false;
					playerHarry.TakeDamage (BigBiteDamage, Pawn(Owner), location, velocity*1, 'Aragog');
				}
				else
				{
					playerHarry.TakeDamage (BiteDamage, Pawn(Owner), location, velocity*1, 'Aragog');
				}
				
				playerHarry.clientMessage("BITE HARRY");

			}
		}
	}

	// Check if it's time to backward to the center of the web
	if ( IsInState('stateGoHome') )
	{
		if ( bMove == true )
		{
			MoveSmooth( vDir * groundSpeed * DeltaTime);

			if ( vSize2D(location - vHome) < 20 )
			{
				bMove = false;
				loopAnim('idle', 1.0, 0.2);

				gotoState('stateHarryHunting');

			}
		}
	}
}

// The function used by the enemyHealthBar for the boss battle. A return value of 0 will result in the
// HealthBar disappearing.
function float GetHealth()
{
	return float(Health) / 100;
}


// An anchor has been hit by a spell. IF:
// all of the anchors have been hit goto the Aragog is falling state
// if there are more anchors to hit (but not all of them) check if enough have been hit to increment the
// numbers of spiders per marker. If so, go through all of the markers and increment the number of spiders
// for the large and then the small spiders
function AnchorHitBySpell(int iLocation)
{
	local SpiderMarker sm;
	local AragogAttendentSpawner theSpawner;
	local int totalAttendentsInLevel;
	local SpiderAttendent at;

	playerHarry.clientMessage("Location Hit : " $iLocation);

	PlayOuchSound();

	numAnchorsAttached--;

	if ( numAnchorsAttached <= 0 )
	{
		gotoState('stateFalling');
	}
	else if ( numAnchorsAttached % spiderAnchorRatio == 0 ) 
	{
		TriggerEvent( 'WebAnchorHit', self, None );

		// for now I'm leaving the small spiders as they are. It's ugly but.. there's no time
		for ( counter=0; counter<smallSpiderIncrement; counter++ )
		{
			foreach AllActors( class'SpiderMarker', sm )
			{
				sm.incrementNumSmallSpiders();
			}
		}
	}
	else
	{
		TriggerEvent( 'WebAnchorHit', self, None );
	}

	foreach AllActors( class'SpiderAttendent', at )
	{
		totalAttendentsInLevel++;
	}

	// check to make sure that there aren't too many spiders in the level 
	if ( totalAttendentsInLevel < maxAttendentsInLevel )
	{ 
		// Large spiders are spawned from a different kind of spawner. 
		theSpawner = FindClosestSpawner(webAnchors[iLocation].anchor.location);
		theSpawner.SpawnSpiders((numAnchors-numAnchorsAttached)-1);

	}

}


function AragogAttendentSpawner FindClosestSpawner(vector loc)
{
	local int counter;
	local float dist, farthestDist;
	local AragogAttendentSpawner ss;
	local AragogAttendentSpawner retSpawner;
	local vector AnchorLocation;

	AnchorLocation = loc;

	farthestDist = 99999999999;

	foreach AllActors( class'AragogAttendentSpawner', ss )
	{
		if ( Vsize(ss.location - AnchorLocation) < farthestDist )
		{
			retSpawner = ss;
			farthestDist = Vsize(ss.location - AnchorLocation);
		}
	}

	return retSpawner;

}

function destroyAllSpiders()
{
	local SpiderSmall smallSpider;
	local SpiderLarge largeSpider;
	local SpiderAttendent attendentSpider;

	// We are in Phase Two. Destroy all of the Phase One spiders

	foreach AllActors( class'SpiderSmall', smallSpider )
	{
		smallSpider.Destroy();
	}

	foreach AllActors( class'SpiderLarge', largeSpider )
	{
		largeSpider.Destroy();
	}

	foreach AllActors( class'SpiderAttendent', attendentSpider )
	{
		attendentSpider.Destroy();
	}
}

function MoveAwayFromThePit()
{
	local AragogHarrySafeZone safeZone;
	local vector TowardHarryDirection;
	local vector NewLocation;

	foreach AllActors( class'AragogHarrySafeZone', safeZone )
		break;

	TowardHarryDirection = normal(playerHarry.location - vHome);
	NewLocation = vHome + (TowardHarryDirection * 650);
	NewLocation.z = playerHarry.location.z;

	safeZone.SetLocation( NewLocation );

//playerHarry.clientMessage("Called MoveAwayFromThePit. SafeZone new Location  :  " $safeZone.location);

}


function createWeb(vector loc)
{
	local int webCounter;
	local bool isHidden;
	local int webIndex;

	// go through each web and get one that's hidden
	for ( webCounter = 0; webCounter < maxWebs; webCounter++ )
	{
		if ( stickyWeb[webCounter].bHidden == true )
		{
			webIndex = webCounter;
		}
	}

	stickyWeb[webIndex].SetLocation(loc);
	stickyWeb[webIndex].BeginToGrow();
	stickyWeb[webIndex].fLifetime = WebLifetime;

}


auto state AragogBegin
{	
	begin:

//	gotoState('PhaseOne');

}


state CutIdle
{
	function Trigger( Actor Other, Pawn EventInstigator )
	{
		if ( cutSceneCounter == 1 )
		{
			// Initial cut scene introducing Aragog
		}
		else if ( cutSceneCounter == 2 )
		{
			// Second cut scene after Aragog falls
			MoveAwayFromThePit();
			destroyAllSpiders();
		}
		else if ( cutSceneCounter == 2 )
		{
			// Ending cut scene that explains everything
		}
	}

	begin:

	velocity = vec(0,0,0);
	acceleration = vec(0,0,0);

	if ( cutSceneCounter == 0 )
	{
		// Initial cut scene introducing Aragog
		cutSceneCounter = 1;
	}
	else if ( cutSceneCounter == 1 )
	{
		// Second cut scene where Aragog falls
		vHome = aHome.location;  // the location of the actor AragogHome
		cutSceneCounter = 2;
	}
	else if ( cutSceneCounter == 2 )
	{
		// Ending cut scene that explains everything
		cutSceneCounter = 3;
	}
	
}

state PhaseOne
{

	begin:

	pitch = 0;//rotation.pitch;

	loopAnim('rotate', 1.0, 0.2);

	// turn toward Harry. The spider is higher up so keep the same pitch
	vDir = normal(playerHarry.location - location);
	vRot = rotator(playerHarry.location - location);
	vRot.pitch = pitch;

	DesiredRotation = vRot;

	// Play the first dialog to his minions right away
	// 'bite him my children' seemed appropriate
	sSoundID = "PC_ARA_ArragogFight_13";
	Localize( "all",sSoundID,"HPdialog" );
	aragogVoice = Sound( DynamicLoadObject("AllDialog."$sSoundID, class'Sound') );
	PlaySound( aragogVoice, SLOT_Talk, 256,[Radius]1000000); 

	sleep(1.0);

	loopAnim('Idle', 1.0, 0.2);

// TEMP
//gotoState('stateHarryHunting');

}


state stateFalling
{

	begin:

	playerHarry.clientMessage("Aragog is Falling");

	foreach AllActors( class'SpiderMarker', marker )
	{
		marker.disableMarker();
	}

	sleep(0.1);

	TriggerEvent( 'AragogIsFalling', self, None );

	// SHOULD GO TO CUTSCENE FROM HERE (SEE CUTCAPTURE)

}


state stateHarryHunting
{

	begin:

	loopAnim('idle', 1.0, 0.2);
}


state statePhysicalAttack
{
	
	begin:

	distanceFromHarry = 450;

	// look at harry
	loopAnim('rotate',1.5, 0.2);

	//Turn toward Harry
	TurnToward(playerHarry);

	acceleration = vec(0,0,0);
	velocity = vec(0,0,0);

	//playAnim('rearsUp');
	//finishAnim();
	//
	//loopAnim('rearsUpLoop');
	//DesiredRotation.Yaw = rotator(playerHarry.Location-Location).yaw;
	//sleep( RearUpTime / 2 );  //Do half, so it's not so easy during his lunge rear up

	playAnim('UpToRun', 1.0, 0.2);
	DesiredRotation.Yaw = rotator(playerHarry.Location-Location).yaw;
	Sleep( 10.0/30.0 );
	PlaySound( sound'HPSounds.Adv1Willow.whomp01', Slot_none, RandRange(0.8,1.0), [Radius]1000000, [Pitch]RandRange(0.8,1.2) );
	Sleep( 5.0/30.0 );
	DoStomp( true );
	FinishAnim();

	gotoState('stateMoveToAttack');

}

state stateMovetoAttack
{
	function BeginState()
	{
		bMove = false; 
	}

	begin:

	// Play the attack dialog randomly
	if ( rand(3) == 0 )
	{
		PlayAttackingSound();
	}

	// look at harry
	loopAnim('rotate',1.5, 0.2);

	//Turn toward Harry
	TurnToward(playerHarry);

	// get the current attack location
	vLocation = (playerHarry.location + (normal(location - playerHarry.location) * (distanceFromHarry)));
	vDir = normal(((playerHarry.location + (normal(location - playerHarry.location) * (distanceFromHarry))) - location)*vec(1,1,0));

	// move toward Harry at a walk (or a run...)
	loopAnim('run', 1.0, 0.2);
	bMove = true;

}

state stateBiteHarry
{

	begin:

	BiteStartFrame = 15.0/36.0;
	BiteEndFrame = 24.0/36.0;

	// stop when near Harry
	acceleration = vec(0,0,0);
	velocity = vec(0,0,0);

	// Only bite Harry once. This is always false until Aragog bites harry (keeps damage to a minimum)
	bCanBiteHarry = true;

	// bite Harry
	playAnim('bite', 1.0, 0.2);
	PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack01', Slot_none, [Radius]1000000, [Pitch]1.0);
	Sleep( 8.0/30.0 );
	PlaySound( sound'HPSounds.Adv1Willow.whomp01', Slot_none, RandRange(0.8,1.0), [Radius]1000000, [Pitch]RandRange(0.8,1.2) );
	finishAnim();

	if( GetHealth() < 0.5 )
	{
		//Do the PreAttack1 side to side stomp anim
		TempFloat = StompAnimRateStart + (StompAnimRateEnd-StompAnimRateStart) * (1-GetHealth());
		playAnim('PreAttack1', TempFloat, 0.2);
		PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack03', Slot_none, [Radius]1000000, [Pitch]TempFloat);
		//Sleep( GetSoundDuration( sound'HPSounds.critters_sfx.Arragog_Attack03' ) - 0.2 );
		Sleep( 15.0/30.0/TempFloat );
		DoStomp( true );//false );
		if( VSize2d((location+vector(Rotation-rot(0,9000,0))*350) - playerHarry.location) < 200 )
			playerHarry.TakeDamage(30, Pawn(Owner), location, vect(0,0,0), 'Aragog');
		PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack03', Slot_none, [Radius]1000000, [Pitch]TempFloat);
		Sleep( 20.0/30.0/TempFloat );
		DoStomp( true );//false );
		if( VSize2d((location+vector(Rotation+rot(0,9000,0))*350) - playerHarry.location) < 200 )
			playerHarry.TakeDamage(30, Pawn(Owner), location, vect(0,0,0), 'Aragog');

	}

	playAnim('rearsUp', 1.0, 0.2 );
	PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack04', Slot_none, 0.4, [Radius]1000000, [Pitch]0.5);
	finishAnim();

	loopAnim('rearsUpLoop', 1.0, 0.2 );
	PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack04', Slot_none, 1.0, [Radius]1000000, [Pitch]0.5);
	DesiredRotation.Yaw = rotator(playerHarry.Location-Location).yaw;
	sleep( RearUpTime * 0.75 );  //Do less, so it's not so easy during his lunge rear up

	LoopAnim('idle', 1.0, 2.0);
	Sleep( 1.0 );

	gotoState('stateGoHome');

}

state stateGoHome
{

	function BeginState()
	{
		bMove = false; 
	}

	begin:

	vDir = normal(vHome - location);

	loopAnim('runBack', 1.0, 0.2 );
	bMove = true;

}

state stateTurnToBite
{

	begin:

	vDir = normal( (playerHarry.location - location)*vect(1,1,0) );
	vRot = rotator( vDir );

	DesiredRotation = vRot;
	//SetRotation(vRot);
	
	gotoState('stateMovetoAttack');

}

state stateSpellAttack
{
	begin:

	acceleration = vec(0,0,0);
	velocity = vec(0,0,0);

	// look at harry
	loopAnim('rotate',,1.5);

	//Turn toward Harry
	TurnToward(playerHarry);

	TempFloat = StompAnimRateStart + (StompAnimRateEnd-StompAnimRateStart) * (1-GetHealth());

	//Pick which attack we're going to do
	switch( Rand(3) )
	{
		case 0:
			spellType = SPELL_DIRECT;
			PlayAnim( 'PreAttack1', TempFloat, 0.2 );
			PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack03', Slot_none, [Radius]1000000, [Pitch]TempFloat);
			//Sleep( GetSoundDuration( sound'HPSounds.critters_sfx.Arragog_Attack03' ) - 0.2 );
			Sleep( 15.0/30.0/TempFloat );
			DoStomp( true );//false );
			PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack03', Slot_none, [Radius]1000000, [Pitch]TempFloat);
			Sleep( 20.0/30.0/TempFloat );
			DoStomp( true );//false );
			break;
		case 1:
			spellType = SPELL_SPRAY;
			PlayAnim( 'PreAttack2', TempFloat, 0.2 );
			PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack04', Slot_none, [Radius]1000000, [Pitch]1.5*TempFloat);
			//Sleep( GetSoundDuration( sound'HPSounds.critters_sfx.Arragog_Attack03' )*0.2 );
			Sleep( 31.0/30.0/TempFloat );
			PlaySound( sound'HPSounds.Adv1Willow.whomp01', Slot_none, RandRange(0.8,1.0), [Radius]1000000, [Pitch]RandRange(0.8,1.2)*TempFloat );
			Sleep( 4.0/30.0/TempFloat );
			DoStomp( true );
			PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack04', Slot_none, [Radius]1000000, [Pitch]TempFloat);
			break;
		case 2:
			spellType = SPELL_LINE;
			PlayAnim( 'PreAttack3', TempFloat, 0.2 );
			PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack06', Slot_none, [Radius]1000000, [Pitch]0.75*TempFloat);
			Sleep( 21.0/30.0/TempFloat );
			PlaySound( sound'HPSounds.Adv1Willow.whomp01', Slot_none, RandRange(0.8,1.0), [Radius]1000000, [Pitch]RandRange(0.8,1.2)*TempFloat );
			Sleep( 4.0/30.0/TempFloat );
			DoStomp( true );//false );
			Sleep( 12.0/30.0/TempFloat );
			DoStomp( true );
			//Sleep( GetSoundDuration( sound'HPSounds.critters_sfx.Arragog_Attack03' )/1.5 - 0.2 );
			//PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack04', [Radius]1000000);
			break;
	}

	FinishAnim();

	gotoState('stateShootSpell');

}

//******************************************************************************************************************
function DoStomp( bool bBigStomp, optional bool bDeepStomp )
{

	if( bDeepStomp )
	{
		playerHarry.ShakeView( 1.7, 200, 200 );

		//if( FRand() < 0.5 )
		//	//PlaySound( sound'HPSounds.Adv9Aragog.Aragog_thump_screen_shake', Slot_none, [Volume]0.333, [Radius]1000000, [Pitch]RandRange(0.7,1.0) );
		//	PlaySound( sound'HPSounds.Adv1Willow.Big_whomp4', Slot_none, [Radius]1000000, [Pitch]RandRange(0.5,0.8) );
		//else
			PlaySound( sound'HPSounds.Ch2skurge.Big_block_fall', Slot_none, [Radius]1000000, [Pitch]RandRange(0.3,0.5) );
			PlaySound( sound'HPSounds.Adv1Willow.Big_whomp4', Slot_none, [Radius]1000000, [Pitch]RandRange(0.3,0.5) );
	}
	else
	if( bBigStomp )
	{
		playerHarry.ShakeView( 0.7, 200, 200 );

		if( FRand() < 0.5 )
			//PlaySound( sound'HPSounds.Adv9Aragog.Aragog_thump_screen_shake', Slot_none, [Volume]0.333, [Radius]1000000, [Pitch]RandRange(0.7,1.0) );
			PlaySound( sound'HPSounds.Adv1Willow.Big_whomp4', Slot_none, [Radius]1000000, [Pitch]RandRange(0.5,0.8) );
		else
			PlaySound( sound'HPSounds.Ch2skurge.Big_block_fall', Slot_none, [Radius]1000000, [Pitch]RandRange(0.5,0.8) );
	}
	else
	{
		playerHarry.ShakeView( 0.4, 100, 100 );
		PlaySound( sound'HPSounds.Adv1Willow.lil_whomper_hit1', Slot_none, [Volume]0.333, [Radius]1000000, [Pitch]RandRange(0.5,0.8) );
	}
}

//******************************************************************************************************************
state stateShootSpell
{
	function BeginState()
	{
		// set spell location to vec(0,0,0) so we start fresh
		spellLocation = vect(0,0,0);
		TempCount = 0;
	}

  begin:

	TempFloat = SpitAnimRateStart + (SpitAnimRateEnd-SpitAnimRateStart)*(1-GetHealth());

	playAnim('UpToSpit', TempFloat, 0.2);
	DesiredRotation.Yaw = rotator(playerHarry.Location-Location).yaw;
	Sleep( 11.0/30.0 / TempFloat );
	PlaySound( sound'HPSounds.Adv1Willow.whomp01', Slot_none , RandRange(0.8,1.0), [Radius]1000000, [Pitch]RandRange(0.8,1.2)*TempFloat );
	Sleep( 4.0/30.0 / TempFloat );
	DoStomp( true );
	//sleep(0.9);
	Sleep(0.3/TempFloat);

	switch (spellType)
	{
		case SPELL_DIRECT:
		case SPELL_SPRAY:

			TempFloat =   VSize(playerHarry.location - Location);
			TempRotator = rotator(playerHarry.location - Location);
			TempInt =     TempRotator.yaw;

			if( spellType == SPELL_SPRAY )
				TempInt += (Rand(2)*2-1)*SpellSpraySpreadAmount/2;
			
			for ( counter=0; counter<5; counter++ )
			{
				//spellLocation = findSpellLocation();
				TempRotator.yaw = TempInt + (counter-2) * SpellSpraySpreadAmount;
				spellLocation = Location  +  vector( TempRotator ) * TempFloat;
				spellLocation -= vect(0,0,5);

				spellOrigin = location + (vector(rotation) * 155);
				spellOrigin = spellOrigin + vec(0,0,160);
				attackSpell = AragogSpellAttack( FancySpawn(class'AragogSpellAttack',self,,spellOrigin,rotator(vDir) ) );
				attackSpell.iDamage = SpellDamage;
				attackSpell.hitTarget = spellLocation;
			}
		
			break;

		case SPELL_LINE:

			for( counter = 0; counter < 4; counter++ )
			{
				if( GetHealth() < 0.5 )
					TempCount2 = 2;
				else
					TempCount2 = 1;

				for( TempCount2=TempCount2; TempCount2>0; TempCount2-- )
				{
					DesiredRotation.Yaw = rotator(playerHarry.Location-Location).yaw;

					if( counter < 4-1 )
					{
						spellLocation = playerHarry.Location - vect(0,0,5) + VRand()*10;
						TempVector = spellLocation;
					}
					else
					{
						TempRotator = rotator(playerHarry.location - Location);
						
						//See which way we should look ahead
						if( ((TempVector - Location) cross (TempVector - playerHarry.Location)).z < 0 )
							TempRotator.yaw += SpellSpraySpreadAmount*0.95;
						else
							TempRotator.yaw -= SpellSpraySpreadAmount*0.95;

						spellLocation = Location  +  vector( TempRotator ) * VSize(playerHarry.location - Location);
						spellLocation -= vect(0,0,5);
					}

					spellOrigin = location + (vector(rotation) * 155);
					spellOrigin = spellOrigin + vec(0,0,160);
					attackSpell = spawn(class'AragogSpellAttack',self,,spellOrigin,rotator(vDir) );
					attackSpell.iDamage = SpellDamage;
					attackSpell.hitTarget = spellLocation;

					if( TempCount2 > 1 )
						Sleep( 0.15 );
					else
						Sleep( TimeBetweenSPELL_LINEShotsStart+(TimeBetweenSPELL_LINEShotsEnd-TimeBetweenSPELL_LINEShotsStart)*(1-GetHealth()) );
				} //end for TempCount2
			}

			break;
	}
	
	sleep(0.5);
	TempCount++;

	if( TempCount < 2  &&  GetHealth() < 0.5  &&  (spellType==SPELL_DIRECT || spellType==SPELL_SPRAY) )
		Goto 'begin';

	playAnim('rearsUp', 1.0, 1.0 );
	PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack04', Slot_none, 0.4, [Radius]1000000, [Pitch]0.5);
	DesiredRotation.Yaw = rotator(playerHarry.Location-Location).yaw;
	FinishAnim();

	loopAnim('rearsUpLoop', 1.0, 0.2 );
	PlaySound(sound'HPSounds.critters_sfx.Arragog_Attack04', Slot_none, 1.0, [Radius]1000000, [Pitch]0.5);
	DesiredRotation.Yaw = rotator(playerHarry.Location-Location).yaw;
	sleep( RearUpTime * 1.0 );

	LoopAnim('idle', 1.0, 1.0 );

	gotoState('stateHarryHunting');

}

//***************************************************************************************************************************************
state stateHitByRictusempra
{
	function Timer()
	{
		PlaySound( sound'HPSounds.critters_sfx.Basilisk_attack3', Slot_none , 1.0, [Radius]1000000, [Pitch]0.7 );
	}

  Begin:

	TempFloat = 0.45;
	PlayAnim('KnockBack', TempFloat, 0.2);
	PlaySound( sound'HPSounds.critters_sfx.Pig_Squeal1',          Slot_none , 1.0, [Radius]1000000, [Pitch]0.8 );
	PlaySound( sound'HPSounds.critters_sfx.Basilisk_attack3',     Slot_none , 1.0, [Radius]1000000, [Pitch]0.9 );
	PlaySound( sound'HPSounds.critters_sfx.BasilAttackWarning00', Slot_none , 1.0, [Radius]1000000, [Pitch]0.8 );
	SetTimer( 0.5, false );
	Sleep( 12.0/30.0 / TempFloat );
	PlaySound( sound'HPSounds.critters_sfx.Basilisk_attack3', Slot_none , 1.0, [Radius]1000000, [Pitch]0.8 );
	DoStomp( true );
	//Sleep( 10/30.0 );
	do { sleep(0.00001); } until( AnimFrame >= 17.0/46.0 );
	AnimRate = 0.75;
	PlaySound( sound'HPSounds.critters_sfx.Arragog_Attack05', Slot_none , 1.0, [Radius]1000000, [Pitch]RandRange(0.8,1.2) );
	do { sleep(0.00001); } until( AnimFrame >= 24.0/46.0 );
	AnimRate = 1.0;
	FinishAnim();

	PlayAnim('React', 1.0, 0.2);
	PlaySound( sound'HPSounds.critters_sfx.Arragog_Attack02', Slot_none , 1.0, [Radius]1000000, [Pitch]RandRange(0.8,1.0) );
	Sleep( 7.0/30.0 );
	PlaySound( sound'HPSounds.critters_sfx.Arragog_Attack02', Slot_none , RandRange(0.5,0.7), [Radius]1000000, [Pitch]RandRange(0.5,0.7) );
	FinishAnim();

	gotoState('stateGoHome');

}


//***************************************************************************************************************************************
state stateBeatAragog
{

	begin:
	
	PlayAnim('Slump', 0.7, 0.2);

	// Play a 'I've been beaten' dialog/sound
	PlaySound( sound'HPSounds.Adv9Aragog.SS_ARA_Hurtscream_0002', Slot_none , 1.0, [Radius]1000000 );
	PlaySound( sound'HPSounds.critters_sfx.Pig_Squeal1',          Slot_none , 1.0, [Radius]1000000, [Pitch]0.7 );
	PlaySound( sound'HPSounds.Adv9Aragog.SS_ARA_Growl_0001b', Slot_none , 1.0, [Radius]1000000 );
	Sleep( 1 );
	//FinishAnim();
	LoopAnim('RearsUpLoop', 1.0, 0.8);
	PlaySound( sound'HPSounds.critters_sfx.Pig_Squeal1',          Slot_none , 1.0, [Radius]1000000, [Pitch]1.0 );
	PlaySound( sound'HPSounds.Adv9Aragog.SS_ARG_BigDeathScream_05b', Slot_none , 1.0, [Radius]1000000, [Pitch]1.3 );
	Sleep( 0.5 );
	PlaySound( sound'HPSounds.Adv9Aragog.SS_ARA_Growl_0001b', Slot_none , 0.5, [Radius]1000000 );
	PlaySound( sound'HPSounds.critters_sfx.Pig_Squeal1',          Slot_none , 0.5, [Radius]1000000, [Pitch]0.4 );
	TempFloat = 0.6; //anim rate of slump
	TempTime = 43.0/30.0 / TempFloat;
	PlayAnim('Slump', TempFloat, 0.2);
	Sleep( 30.0/30.0 );  //30 animframes
	TempTime -= 30.0/30.0;
	PlaySound( sound'HPSounds.Adv9Aragog.SS_ARG_BigDeathScream_05b', Slot_none , 1.0, [Radius]1000000, [Pitch]1.3 );
	sleep( TempTime );
	//playerHarry.ShakeView( 1.0, 200, 200 );
	DoStomp( true, true );
	sleep( 2.5 );

	eVulnerableToSpell=SPELL_None;

	// make sure that he won't try to attack again after being beaten
	bPhaseOneOver = false;

//	PlayAnim('KnockBack');
//	FinishAnim();


	SendDefeatedTrigger();
	playerHarry.StopBossEncounter();
}

state NipHarry
{

	begin:

	bBigBite = true;

	PlayAnim('knockBack', 1.0, 0.2);
	sleep(0.3);

	nipLocation = location + (normal(playerHarry.location - location)*100);

	SetLocation(nipLocation);

	gotoState('stateMovetoAttack');

}


defaultproperties
{
     Mesh=SkeletalMesh'HPModels.skAragogMesh'
     AmbientGlow=65
//   CollisionRadius=350
	 CollisionRadius=55
     CollisionHeight=45
	 groundSpeed=500
	 eVulnerableToSpell=SPELL_Rictusempra

	 RotationRate=(Pitch=40000,Yaw=40000,Roll=40000)

	 Physics=PHYS_Walking	
	 drawScale=3

	 bCollideWorld=True
	 bCollideActors=True
	 bBlockActors=False
	 bBlockPlayers=False
	 bRotateToDesired=True

	 minTimeBetweenHits=20.0
	 numAnchors=8
	 randDialogInterval=15
	 timeBetweenAttacks=4
	 bOnMyWeb=False
	 RearUpTimeStart=3    // Time with full health
	 RearUpTimeEnd=1      // Time with no health
	 
	 StompAnimRateStart=1.0
	 StompAnimRateEnd=1.5

	 SpitAnimRateStart=1.0
	 SpitAnimRateEnd=1.25

	 TimeBetweenSPELL_LINEShotsStart=0.6
	 TimeBetweenSPELL_LINEShotsEnd=0.4
	 
	 //RearUpTimeDecrement=0.05
	 WebLifetime=3.0
	 Health=100
	 damageVulnerable=25
	 damageNormal=1
	 WebDamageTimer=1.0
	 WebDamage=15
	 SpellDamage=25
	 SpellSpraySpreadAmount=3000
	 BiteDamage=60
	 BigBiteDamage=60
	 numberOfSpells=1
	 HealthAddSpellTwo=75
	 HealthAddSpellThree=50
	 webCollisionRadius=35
	 maxAttendentsInLevel=15
//	 BiteStartFrame=14
//	 BiteEndFrame=28

	 maxWebs=7

     EnemyHealthBar=EnemyBar_Aragog
	 
	 bGestureFaceHorizOnly=true
}
