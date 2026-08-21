// Class Name  : Gnome
//
// Created on  : 06/20/2002
// Authored by : Michael Lankerovich
// 
// Description : gnome AI
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class Gnome extends HChar;

// *** Variables
var vector		vHome;			// starting position
var vector		tempHome; 		// for wandering state

var PatrolPoint	pP;				// Closest Patrol Point
var vector		pPLoc;			// Closest Patrol Point Location

var vector		vNewLoc;
var vector		vTargetDir;		// stores our target direction (normalized)
var vector		tempTargetDir; 
var vector		vTargetLocation;

var actor		aTargetGoody; 	// our current target actor

var int			iBeans;
//var int			iPotions;

var	int			Lives;

var	int			LastAnimFrame;
var bool		bPlayThrowAnim;

var bool		bHome;
var bool		bJustCreated;

var float		HitByFlipendoTime;

var float		SoundDuration;
var float		SoundPlayTime;
var bool		bCurrentSoundOver;

var float		savedCollisionRadius;
var float		savedCollisionHeight;

var() bool 		bHasHome;
var() int		HarryBeans;
var() int		HarryPotions;
var() int		MaxGoodies;

var float		hunger;

const		BOOL_DEBUG_AI	= false;	// if true this will spit out debug AI info

enum EObject
{
	OBJECT_BEAN,	
//	OBJECT_POTION,
	OBJECT_MUSHROOM,
	OBJECT_TOTAL,
};

// *** Animation Refrence ***

//	runnormal		19
//	runattack		19
//	runattackbite	19
//	runscared		19
//	runcarryobject	19

//	breath			33
//	downbreath		31

//	look			61
//	knockback		56
//	downdizzy		36
//	getup			51
//	sidestep		36
//	introangry		168

//	tauntass		68
//	tauntjump		61

//	Carry_taunt1	68	// tauntass
//	Carry_taunt2	61	// tauntjump

//  pickup			69

//  eat				79
//  eatloop			71

//  idlecarryobject	33

//  throw			46

// *** Sound Refrence ***

//	gnome_die01			till 06
//	gnome_eat01			till 04
//	gnome_eatloop01		till 04
//	gnome_gets_thrown01 till 10
//	gnome_talk01  		till 16

// *** States Refrence ***

// auto stateIdle
//		stateWasHoldingByHarry
//		stateGotoPatrolPointForce
//		stateGotoPatrolPointBack
//		stateGoHome
//		stateBeingThrown
//		stateRunAway
//		stateHitByFlip
//		stateGettingUp
//		statePatrol
//		stateGotoHarry
//		stateThrowMushroomLump
//		stateGoSomeWhere
//		stateGotoTargetObject
//		stateEatMushroom
//		statePickupTargetObject
//		stateHitWall

// --------------------------------------------------------------------------------------------
// *** Functions

function PlayerCutCapture()
{
	Velocity = vect(0,0,0);
	Acceleration = vect(0,0,0);
	LoopAnim( 'tauntAss', 1.0, 2.0 );
	GotoState( 'stateJustStandThere' );
}

function PlayerCutRelease()
{
	GotoState( 'stateIdle' );
}


function bool VeryCloseToPatrolPoint()
{
	if( VSize2D(location - pPLoc) < 10.0 )
		return true;

	return false;
}

// bool   LineOfSightTo(actor A) - checking just for geometry (Use it just for PatrolPoints, because they do not have collisions)
// bool MyLineOfSightTo(Actor A) - checking for geometry and all actors as well (Use it for beans, potions, mushrooms and Harry)
function bool MyLineOfSightTo( Actor End )
{
	local vector	HitLocation;
	local vector	HitNormal;
	local actor		HitActor;

	// Trace line with ACTORS
	foreach TraceActors(class'actor', HitActor, HitLocation, HitNormal, End.location, location)
	{
		if( HitActor == End )
			return true;

		if( HitActor == self )
			continue;

		if( HitActor == Level )
			return false;

		if(HitActor.bBlockActors)
			return false;

	
	} // end for each actor	
	
	return false;
}

// --------------------------------------------------------------------------------------------
// *** Find Functions

function bool FindClosestPP()
{
	local PatrolPoint ClosestPP, CurPP;
	local float	fClosestDist, fDist;

	ClosestPP = none;
	fClosestDist = 999999;
	foreach AllActors( class'PatrolPoint', CurPP )
	{
		if(LineOfSightTo(CurPP))
		{
 			fDist = vsize(CurPP.location - location);
			if( fDist < fClosestDist )
			{
				ClosestPP = CurPP;
				fClosestDist = fDist;
			}
		}
	}
 	
	if( ClosestPP != none )
	{
		// we found patrol point
		pP		= ClosestPP;
		pPLoc	= ClosestPP.Location;
		return true;
	}

	// if there is no patrol points at all, set it at start location
	pP		= none;
	pPLoc	= location;
	return false;
}

function class<Actor> FindClassType(EObject otype)
{
	local class<Actor> classtype;

	switch(otype)
	{
		case OBJECT_BEAN:
			classtype = Class'JellyBean';
			break;
//		case OBJECT_POTION:
//			classtype = Class'WiggenWell';
//			break;
		case OBJECT_MUSHROOM:
			classtype = Class'Horklumps';
			break;
	}

	return classtype;
}

function class<Actor> FindPotionClassType()
{
	local class<Actor> pclasstype;

	pclasstype = Class'HProps.WWellBlueBottle';
/*
	// later, when green bottle's offset will be fixed, add green bottle as well
 	switch(Rand(3))
	{
		case 0:	 pclasstype = Class'HProps.WWellBlueBottle';	break;
		case 1:	 pclasstype = Class'HProps.WWellOrangeBottle';	break;
		case 2:	 pclasstype = Class'HProps.WWellGreenBottle';	break;
	}
*/
	return pclasstype;
}

function actor FindClosestObject(EObject otype)
{
	local class<Actor> classtype;
	local actor			ClosestActor, CurrActor;
	local float			fClosestDist, fDist;
	
	classtype = FindClassType(otype);

	ClosestActor = none;
	fClosestDist = 999999;
	foreach AllActors( classtype, CurrActor )
	{
		fDist = vsize(CurrActor.location - location);
		if( (fDist < SightRadius) && (fDist < fClosestDist) )
		{
			if(MyLineOfSightTo(CurrActor))
			{
				// store the closest actor and its distance
				if(CurrActor.Owner == none)
				{
					ClosestActor = CurrActor;
					fClosestDist = fDist;
				}
			}
		}
	}
	
	if( ClosestActor != none )
	{
		// we found something!
		return ClosestActor;
	}

	return none;
}

function bool HasTooManyGoodies()
{
	if(	iBeans >= MaxGoodies )
//	if(	iBeans + iPotions >= MaxGoodies )
		return true;

	return false;
}

function actor FindClosestMushroom()
{
	local actor m;

	m = FindClosestObject(OBJECT_MUSHROOM);

	return m;
}

function actor FindClosestGoody()
{
	local float	fDistJB, fDistWW;
	local actor jb, ww, m;

	// look for mushroom first
	m = FindClosestMushroom();
	if( (m != none) && (aHolding == none) )
	{
		aTargetGoody = m;
		return aTargetGoody;
	}

	jb = FindClosestObject(OBJECT_BEAN);
	ww =none;								// ww = FindClosestObject(OBJECT_POTION);

	if((jb == none) && (ww == none))
	{
		aTargetGoody = none;
	}
	else if(ww == none)
		aTargetGoody = jb;

//	else if(jb == none)
//		aTargetGoody = ww;
//
//	else
//	{
//		fDistJB = vsize(jb.location - location);
//		fDistWW = vsize(ww.location - location);
//		if(fDistJB < fDistWW)
//			aTargetGoody = jb;
//		else
//			aTargetGoody = ww;
//	}

	return aTargetGoody;
}

// --------------------------------------------------------------------------------------------
// *** Sound Functions

function PlaySoundAndSetDuration(Sound snd)
{
	local float dist;
	if(!bCurrentSoundOver)
		return;

	// do not speak, if cutscene is played
	if(baseHud(playerHarry.myHud).bCutSceneMode)
		return;

	// do not speak, if too far from Harry
	dist = vsize(playerHarry.location - location);
	if( dist > SightRadius )
		return;

	PlaySound(snd);
	SoundDuration = GetSoundDuration(snd);

	bCurrentSoundOver = false;
}

function PlaySoundGetOff()
{
	switch( Rand(3) )
	{
		case 0:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk01');break;
		case 1:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk02');break;
		case 2:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk03');break;
	}
}

function PlaySoundOuch()
{
	switch( Rand(8) )
	{
		case 0:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_ouch01'); break;
		case 1:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_ouch02'); break;
		case 2:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_ouch03'); break;
		case 3:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_ouch04'); break;
		case 4:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_ouch05'); break;
		case 5:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_ouch06'); break;
		case 6:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_ouch07'); break;
		case 7:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_ouch08'); break;
	}
}

function PlaySoundTalk()
{
	switch( Rand(16) )
	{
		case 0:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk01');break;
		case 1:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk02');break;
		case 2:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk03');break;
		case 3:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk04');break;
		case 4:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk05');break;
		case 5:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk06');break;
		case 6:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk07');break;
		case 7:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk08');break;
		case 8:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk09');break;
		case 9:	 PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk10');break;
		case 10: PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk11');break;
		case 11: PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk12');break;
		case 12: PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk13');break;
		case 13: PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk14');break;
		case 14: PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk15');break;
		case 15: PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_talk16');break;
	}
}

function PlaySoundDie()
{
	switch( Rand(6) )
	{
		case 0:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_die01');break;
		case 1:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_die02');break;
		case 2:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_die03');break;
		case 3:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_die04');break;
		case 4:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_die05');break;
		case 5:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_die06');break;
	}
}

function PlaySoundGetsThrown()
{
	switch( Rand(10) )
	{
		case 0:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown01');break;
		case 1:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown02');break;
		case 2:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown03');break;
		case 3:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown04');break;
		case 4:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown05');break;
		case 5:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown06');break;
		case 6:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown07');break;
		case 7:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown08');break;
		case 8:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown09');break;
		case 9:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_gets_thrown10');break;
	}
}

function PlaySoundEat()
{
	switch( Rand(4) )
	{
		case 0:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_eat01'); break;
		case 1:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_eat02'); break;
		case 2:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_eat03'); break;
		case 3:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gnome_eat04'); break;
	}
}

function PlaySoundCrash()
{
	switch( Rand(5) )
	{
		case 0:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gno_crash1');break;
		case 1:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gno_crash2');break;
		case 2:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gno_crash3');break;
		case 3:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gno_crash4');break;
		case 4:	PlaySoundAndSetDuration(Sound'HPSounds.critters_sfx.gno_crash5');break;
	}
}

// --------------------------------------------------------------------------------------------
// *** Drop / pick up Functions

function DropGoodies(EObject otype, int nums, vector loc, float height, bool backward)
{
	local class<Actor>	classtype;
	local actor			a;
	local vector		sloc, v1, v2;
	local float			angle, length;


	// if throw gnome in the hole, we want beans go back from the hole,
	// so spawn them, outside of the hole, just in front of it (about 40 units)
	if(backward)
		sloc = loc + (playerHarry.Location - loc) * 40.0 / VSize(playerHarry.Location - loc);
	else
		sloc = loc;

	sloc.z += height + 30;

	while (nums > 0)
	{
		classtype = FindClassType(otype);

//		// potion does not have generic type function
//		if(otype == OBJECT_POTION)
//			classtype = FindPotionClassType();

		a = Spawn(classtype, , , sloc, RotRand ());

		// we do not want to despawn goodies (just in case)
		HPawn(a).bDespawnable = false;

		// random velocity, at least 100 up, and 100 sidewise
		angle	= RandRange(0.0000, 6.2832);
		length	= RandRange(50, 100);

		a.velocity.x  = length * cos(angle);
		a.velocity.y  = length * sin(angle);
		a.velocity.z  = 100 + FRand() * 100;	

		// if throw gnome in the hole, we want beans go back from the hole,
		if(backward)
		{
			// exactly direction, we want goodies to come
			v2.x = -velocity.x;
			v2.y = -velocity.y;
			v2.z = 0;

			// looking for the random vector, pointing in front of the hole
			while(true)
			{
				angle	= RandRange(0.0000, 6.2832);
				v1.x = cos(angle);
				v1.y = sin(angle);
				v1.z = 0;

				// to prevent infinite loop
				if((v2.x == 0.0) && (v2.y == 0.0))
					break;

				if(v1 dot v2 > 0)
					break;

			}

			// spread it a little bit
			a.velocity.x  = length * cos(angle);
			a.velocity.y  = length * sin(angle);
		}

		nums --;
	}
}

//function GnomeDropPotions()
//{
//	if(iPotions == 0)
//		return;
//
//	DropGoodies(OBJECT_POTION, iPotions, location, CollisionHeight, false);
//	iPotions = 0;
//}

//function HarryDropPotions(int potions)
//{
//	if(potions == 0)
//		return;
//
//	DropGoodies(OBJECT_POTION, potions, playerHarry.location, playerHarry.CollisionHeight, false);
//}

function GnomeDropBeans()
{
	if(iBeans == 0)
		return;

	DropGoodies(OBJECT_BEAN, iBeans, location, CollisionHeight, false);
	iBeans = 0;
}

function HarryDropBeans(int beans)
{
	if(beans == 0)
		return;

	DropGoodies(OBJECT_BEAN, beans, playerHarry.location, playerHarry.CollisionHeight, false);
}

function GnomePickupObject(actor a)
{
	if(a == none)
		return;

	// do not allow to pick up mushroom (just in case)
	if(a.IsA('Horklumps'))
	{
		a.Destroy();
		a = none;
		return;
	}

	PlaySoundTalk();

	if(a.IsA('JellyBean'))
		iBeans++;
//	else if(a.IsA('WiggenWell'))
//		iPotions++;

	// if holds something, destroy it, to be able to pickup a new stuff
	if( aHolding != none ) 
	{
		aHolding.Destroy();
		aHolding = none;
	}

	ObjectPickup(a, 'GNOME R Hand');
}

function bool HarryHasSomeStuff()
{
	local int stuff;

	stuff = playerHarry.JellyBeansCount();
	if(stuff > 0)
		return true;

//	stuff = playerHarry.PotionsCount();
//	if(stuff > 0)
//		return true;

	return false;
}

// --------------------------------------------------------------------------------------------
// *** Generic Engine Functions

function PreBeginPlay()
{
	Super.PreBeginPlay();
	
	bJustCreated = true;

	bHome = true;

	DrawScale	= RandRange(1.0, 1.5);
	Fatness		= RandRange(112, 160);

	FindClosestPP();

	// set our home location to be were we started
	vHome = location;
	
	playerHarry.addJellyBeansPoints(HarryBeans);		// just for testing
	playerHarry.addPotionsPoints(HarryPotions);  	 	// just for testing

	// save collision information
	savedCollisionRadius = CollisionRadius;
	savedCollisionHeight = CollisionHeight;

	bFlipPushable	= true;
	lockSpell		= true;

	// have our run normal the default animation
	LoopAnim('runNormal');
}

function Bump(actor other)
{
	local int beans, potions;

	if(other != playerHarry)
		return;

 	// get some goodies, only if gnome was looking for them
	if( GetStateName() != 'stateGotoHarry' )
		return;

	beans	= playerHarry.JellyBeansCount();
//	potions = playerHarry.PotionsCount();
  
	GetSomethingFromHarry(beans, 0);

	gotostate('stateGoSomeWhere');	// gotostate('statePatrol');
}

function Touch( actor other )
{
	Super.Touch(other);

	if(other.IsA('JellyBean') || other.IsA('WiggenWell'))
	{
		// do not allow pick up goodies in GettingUpState (and if dead, of course)
		// otherwise, gnome will have something in his hand, while Harry hold 
		if(GetStateName() != 'stateGettingUp' && GetStateName() != 'stateDied')
			gotostate('statePickupTargetObject');
	}

	if( (aHolding == none ) && other.IsA('Horklumps') )
	{
		if(GetStateName() != 'stateGettingUp')
			gotostate('stateEatMushroom');
	}
}

function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellFlipendo( spell, vHitLocation );

	// goto state HitBy Flip
	gotostate('stateHitByFlip');
	
	return true; // true == create spell effects
}

function Tick(float delta)
{
	Super.Tick(delta);

//	playerHarry.ClientMessage("State = " $GetStateName() $" Loc X = " $location.X $" Y = " $location.Y $" Z = " $location.Z $" PP X = " $pPLoc.X $" Y = " $pPLoc.Y $" Z = " $pPLoc.Z $" dist = " $VSize2D(location - pPLoc));
//	log("State = " $GetStateName() $" Loc X = " $location.X $" Y = " $location.Y $" Z = " $location.Z $" PP X = " $pPLoc.X $" Y = " $pPLoc.Y $" Z = " $pPLoc.Z $" dist = " $VSize2D(location - pPLoc));

	// to be able to kill gnome, if touchs despawner after hit by Flipendo
	if(	HitByFlipendoTime > 0)
		HitByFlipendoTime -= delta;

	// check out, if current sound is over to be able to play a new one.
	SoundPlayTime += delta;
	if(SoundPlayTime > SoundDuration)
	{
		bCurrentSoundOver = true;
		SoundPlayTime = 0;
	}

	// allow gnome to be despawnable for a while
	if( (GetStateName() == 'stateBeingThrown') || (HitByFlipendoTime > 0) )
		bDespawnable = true;
	else
		bDespawnable = false;

	if(bDespawned)
	{
		bDespawnable = true;
		if( GetStateName() != 'stateDied' )
			gotostate('stateDied');
	}

	// the only way to support 'Harry holding gnome state'
	if((owner == playerHarry) && (GetStateName() != 'stateWasHoldingByHarry'))
		gotostate('stateWasHoldingByHarry');
}

function Landed( vector HitNormal )
{
	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	Super.Landed(HitNormal);

	SetCollisionSize(savedCollisionRadius, savedCollisionHeight);

	if( GetStateName() != 'stateDied' )
		gotostate('stateGettingUp');
}

function HitWall(vector HitNormal, actor HitWall)
{
	if( IsInState('stateWasHoldingByHarry') )
		return;

	if( IsInState('stateBeingThrown') )
		return;

	if( IsInState('stateHitByFlip') )
		return;

	if( IsInState('stateGettingUp') )
		return;

	if( IsInState('statePickupTargetObject') )
		return;

	if( IsInState('stateThrowMushroomLump') )
		return;

	if( IsInState('stateEatMushroom') )
		return;

	if( IsInState('stateHitWall') )
		return;

	// try to go from the wall a bit
	vNewLoc	= location + 5 * HitNormal / VSize(HitNormal);
	vNewLoc.Z= location.Z;

	gotoState('stateHitWall');
}

// --------------------------------------------------------------------------------------------

function FindTargetLocation()
{
		vTargetLocation = aTargetGoody.location;
/*
	local vector difference;
	local float  fDist;

	if(aTarget.IsA('Horklumps'))
	{
		difference		= location - aTarget.location;
		fDist			= vsize(difference);
		vTargetLocation	= aTarget.location + (aTarget.CollisionRadius + CollisionRadius)* difference / fDist;
	}
	else
		vTargetLocation = aTarget.location;
*/
}

function ShrinkActor(Actor a, float coeff)
{
	a.DrawScale *= coeff;
	a.SetCollisionSize(a.CollisionRadius * coeff, a.CollisionHeight * coeff);
}

function GetSomethingFromHarry(int beans, int potions)
{
	local int howManyBeans, howManyPotions;

	PlaySoundTalk();
	playerHarry.HarryKnockBack();

	if((beans == 0) && (potions == 0))
		return;

	if(beans > 0)
		howManyBeans = Rand(5) + 1;
	else
		howManyBeans = 0;

	if(howManyBeans > beans)
		howManyBeans = beans;

	if(potions > 0)
		howManyPotions = Rand(2) + 1;
	else
		howManyPotions = 0;

	if(howManyPotions > potions)
		howManyPotions = potions;

	// if there is now beans and potions, do nothing
	if((howManyPotions == 0) && (howManyBeans == 0))
		return;

	// if there is no potions, get some beans
	if(howManyPotions == 0)
	{
		HarryDropBeans(howManyBeans);
		playerHarry.addJellyBeansPoints(-howManyBeans);
		return;
	}

//	// if there is no beans, get some potions
//	if(howManyBeans == 0)
//	{
//		HarryDropPotions(howManyPotions);
//		playerHarry.addPotionsPoints(-howManyPotions);
//		return;
//	}
//
//	if( Rand(2) == 0 )
//	{
//		HarryDropBeans(howManyBeans);
//		playerHarry.addJellyBeansPoints(-howManyBeans);
//	}
//	else
//	{
//		HarryDropPotions(howManyPotions);
//		playerHarry.addPotionsPoints(-howManyPotions);
//	}
}

function DestroyHolding()
{
	if(aHolding == none)
		return;

	// clear our holding actor var
	aHolding.SetOwner( None );
	aHolding.Destroy();

	aHolding = none;
}

function DestroyHoldingAndTakeAwayAllGoodies()
{
 	iBeans	= 0;
// 	iPotions= 0;

	DestroyHolding();
}

function PlayIdleAnim()
{
	local int output;

	PlaySoundTalk();

	output = Rand(7);

	if(aHolding != none)
	{
		if( (output == 0) || (output == 1) )
			PlayAnim('IdleCarryObject');
		else if( (output == 2) || (output == 3) )
			PlayAnim('Carry_taunt2');		// TauntJump
		else
			PlayAnim('Carry_taunt1');		// TauntAss
	}
	else
	{
		output = Rand(7);
		if(output == 0)
			PlayAnim('SideStep');
		else if(output == 1)
			PlayAnim('Look');
		else if( (output == 2) || (output == 3) )
			PlayAnim('TauntJump');
		else
			PlayAnim('TauntAss');
	}
}

function LoopRunAnimNormal()
{
	PlaySoundTalk();

	if(aHolding != none)
		LoopAnim('RunCarryObject');
	else
		LoopAnim('RunNormal');
}

function LoopRunAnimAttack()
{
	PlaySoundTalk();

	if(aHolding != none)
		LoopAnim('RunCarryObject');
	else
		LoopAnim('RunAttack');
}

function LoopRunAnimAttackBite()
{
	PlaySoundTalk();

	if(aHolding != none)
		LoopAnim('RunCarryObject');
	else
		LoopAnim('RunAttackBite');
}

state stateJustStandThere
{
  Begin:
	Sleep( RandRange( 1, 2 ) );
	switch( Rand(7) )
	{
		case 0:  PlayAnim( 'look', 1.0, 0.5 );   break;
		case 1:  PlayAnim( 'sidestep', 1.0, 0.5 );   break;
		case 2:  PlayAnim( 'introangry', 1.0, 0.5 );   break;
		case 3:  PlayAnim( 'tauntass', 1.0, 0.5 );   break;
		case 4:  PlayAnim( 'eat', 1.0, 0.5 );   break;
		case 5:  PlayAnim( 'idlecarryobject', 1.0, 0.5 );   break;
		case 6:  PlayAnim( 'carry_taunt1', 1.0, 0.5 );   break;
	}

	FinishAnim();
	Goto 'Begin';
}

// --------------------------------------------------------------------------------------------
// *** States
auto state stateIdle
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": auto stateIdle" );

	// do not stop first time - to fix a bug with Generic Spawner
	if(!bJustCreated)
	{
		// Stop moving
		Acceleration = vect(0,0,0);
		Velocity	 = vect(0,0,0);
	}
	else
		bJustCreated = false;

	// taunt a bit
	PlayIdleAnim();
	FinishAnim();
	
	if(bHome)
	{
		bHome = false;

		// if he has some goodies, hide them
		// if have something in his hand, destroy it
		DestroyHoldingAndTakeAwayAllGoodies();

		// go to patrol point from home
		gotostate('stateGotoPatrolPointForce');
	}
	else
		gotostate('statePatrol');
}

state stateWasHoldingByHarry
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateWasHoldingByHarry" );

	// goto our home location
	PlayAnim('pickup', 1.5);
	FinishAnim();

	PlaySoundGetOff();

	goto 'begin';
}

state stateGotoPatrolPointForce
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateGotoPatrolPointForce" );

	// goto our patrol location
	LoopRunAnimNormal();
	MoveTo(pPLoc);
	
	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// taunt a bit
	PlayIdleAnim();
	FinishAnim();

	// if reached patrol point, start patrolling
	if( VeryCloseToPatrolPoint() )
		gotostate('statePatrol');

	// if could not reach patrol point, go back home
	else
		gotostate('stateGoHome');
}

state stateGotoPatrolPointBack
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateGotoPatrolPointBack" );

	// go back to patrol point 
	LoopRunAnimAttack();
	MoveTo(pPLoc);
	
	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// taunt a bit
	PlayIdleAnim();
	FinishAnim();
	
	// if he has some goodies, hide them
	if( bHasHome && (iBeans > 0) )			// if( bHasHome && ((iPotions > 0) || (iBeans > 0)) )
		gotostate('stateGoHome');

	else
		gotostate('statePatrol');
}

state stateGoHome
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateGoHome" );

	// goto our home location
	LoopRunAnimAttack();
	MoveTo(vHome);
	
	bHome = true;

	// start idling
	gotostate('stateIdle');
}

state stateBeingThrown
{
	function Landed(vector HitNormal)	{ Global.Landed(HitNormal); }

	begin:

	// we could not make CollisionHeight less then MaxStepHeight/2=25/2 = 12.5,
	// because then Harry could step on gnome, instead of picking him up
	// that is why I could not make CollisionHeight less then 13.
	SetCollisionSize(10, 13);

	PlaySoundGetsThrown();

	// allow to play next sound right away
	SoundDuration = 0;

	Lives = 2;

	// it allows us to turn it back from upside down position
	PlayAnim('knockback', 1.5 );
	FinishAnim();

	sleep(1);

	SetCollisionSize(savedCollisionRadius, savedCollisionHeight);

	gotostate('stateGettingUp');
}

state stateRunAway
{
	begin:

	vNewLoc	= location + 100 * VRand();
	vNewLoc.Z= location.Z;

	LoopAnim('runScared');

	TurnTo(vNewLoc);
	MoveTo(vNewLoc);
	desiredRotation.Yaw = Rotation.Yaw;
	
	// taunt a bit
	PlayIdleAnim();
	FinishAnim();

	// start patroling
	gotostate('statePatrol');
}

state stateHitByFlip
{
	function BeginState()
	{
		if(Lives > 0)
			Lives--;

		GnomeDropBeans  ();
//		GnomeDropPotions();
		DestroyHolding();

		PlaySoundOuch();
		
		// start kockedDown animation
		PlayAnim('knockback');

		// get the vector facing harry then face that direction
		vTargetDir		= normal(playerHarry.location - location);
		desiredRotation = rotator(vTargetDir);
		
		HitByFlipendoTime = 2.0;

		// We should get a Landed function call (Maybe not!).
	}

	begin:

	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateHitByFlip" );
}

state stateGettingUp
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateGettingUp" );

	DestroyHoldingAndTakeAwayAllGoodies();

	// just in case
	SetCollisionSize(savedCollisionRadius, savedCollisionHeight);

	if(!bDespawned)
	{
		PlaySoundDie();
		bObjectCanBePickedUp = true;
	}
	else
	{
		bObjectCanBePickedUp = false;
	}

	// breath a little before
	LoopAnim('downbreath');
	Sleep(3.0);
	
	// shake my head 
	PlayAnim('downDizzy');
	FinishAnim();

	// breath a little before getting up
	LoopAnim('downbreath');
	Sleep(2.0);
	
	// shake my head once more before getting up!
	PlayAnim('downDizzy');
	FinishAnim();

	// if he hit despawner, sit till disappear
	if(bDespawned || (Lives == 0))
		goto 'begin';

	// Finnaly... get up.
	PlayAnim('getup');
	FinishAnim();
	
	bObjectCanBePickedUp = false;

	// run away (because we are scared)
	gotostate( 'stateRunAway');
}

state statePatrol
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": statePatrol" );

	// if he has a lot of stuff, go ahead and dump it
	if( HasTooManyGoodies() )
	{
		// if could see patrol point, go
		if(LineOfSightTo(pP))
			gotostate('stateGotoPatrolPointBack');
		else
			gotostate('stateGoSomeWhere');
	}

	// wonder around untill we find harry or a goodies or mushroom
	if( (FindClosestGoody() != none ) )
	{
		// we found something and it is now our target actor
		// set our target direction toward the target actor
		vTargetDir		= normal(aTargetGoody.location - location);
		desiredRotation = rotator(vTargetDir);
		
		// goto state GotoTarget
		gotostate( 'stateGotoTargetObject' );
	}

	if( HarryHasSomeStuff() )
	{
		if(MyLineOfSightTo(PlayerHarry))
		{
			vTargetDir = playerHarry.location - location;
			if( vsize(vTargetDir) < SightRadius)
			{
				// move a bit in Harry's direction
				vNewLoc = location + 20 * vTargetDir / vsize(vTargetDir);

				// we are not holding an object but we see harry so lets attack him!
				gotostate( 'stateGotoHarry' );
			}
			else
				gotostate('stateGoSomeWhere');
		}
		else
			gotostate('stateGoSomeWhere');
	}
	else
	{
		if(Rand(2) == 0)
		{
			if(MyLineOfSightTo(PlayerHarry))
			{
				vTargetDir = playerHarry.location - location;
				if( vsize(vTargetDir) < SightRadius)
				{
				 	vNewLoc = playerHarry.location;
					gotostate( 'stateGotoHarry' );
				}
				else
					gotostate('stateGoSomeWhere');
			}
			else
				gotostate('stateGoSomeWhere');
		}
		else
		{
			// if could see patrol point, go
			if(LineOfSightTo(pP))
				gotostate('stateGotoPatrolPointBack');
			else
				gotostate('stateGoSomeWhere');
		}
	}
	
	// We don't see harry or goodies, so lets wander for a while
	gotostate('stateGoSomeWhere');
}

state stateGotoHarry
{
	begin:

	LoopRunAnimAttackBite();

	TurnTo(vNewLoc);
	MoveTo(vNewLoc);
	desiredRotation.Yaw = Rotation.Yaw;

	gotostate('statePatrol');
}

state stateThrowMushroomLump
{
	function Tick(float dtime)
	{
		local int frame;

		super.Tick(dtime);

 		frame = AnimFrame * 46				;							// 46 - number of frames in throw animation

		if( bPlayThrowAnim && (frame >= 30)  && (LastAnimFrame < 30) )	// 30 - frame to release an object
		{
			ObjectThrow( vTargetDir, true, true );
		}

		LastAnimFrame = frame;
	}

	begin:

	bPlayThrowAnim = false;

	// wander a little bit
	vNewLoc	= location + 100 * VRand();
	vNewLoc.Z= location.Z;

	LoopRunAnimNormal();

	TurnTo(vNewLoc);
	MoveTo(vNewLoc);
	desiredRotation.Yaw = Rotation.Yaw;
	
	// turn to Harry
	TurnTo(playerHarry.location);
	desiredRotation.Yaw = Rotation.Yaw;

	// throw mushroom lump
	vTargetDir = ComputeTrajectoryByTime( location, playerHarry.location, 0.5 );

	bPlayThrowAnim = true;
	PlayAnim( 'throw', 1.0 );
//	ObjectThrow( vTargetDir, true, true );
	FinishAnim();
	bPlayThrowAnim = false;

	// start patroling
	gotostate('statePatrol');
}

state stateGoSomeWhere
{
	begin:

	vNewLoc	= location + 100 * VRand();
	vNewLoc.Z= location.Z;

	LoopRunAnimNormal();

	TurnTo(vNewLoc);
	MoveTo(vNewLoc);
	desiredRotation.Yaw = Rotation.Yaw;

	// taunt a bit
	PlayIdleAnim();
	FinishAnim();

	// start patroling
	gotostate('statePatrol');
}

state stateGotoTargetObject
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateGotoTargetObject" );

	// start to move in random direction a bit, to prevent stuck
	vNewLoc	= location + 20 * VRand();
	vNewLoc.Z= location.Z;

	LoopRunAnimAttack();

	TurnTo(vNewLoc);
	MoveTo(vNewLoc);
	desiredRotation.Yaw = Rotation.Yaw;
	// end to move in random direction a bit, to prevent stuck

	FindTargetLocation();

	// move to our target pos
	LoopRunAnimAttack();
	MoveTo(vTargetLocation);

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// reset desiredRotation;
	desiredRotation.Yaw = Rotation.Yaw;

	// if reached location of goody, go back to patrol
	// it will be picked up on a touch
	gotostate('statePatrol');
}

state stateEatMushroom
{
	function Tick(float dtime)
	{
		super.Tick(dtime);
	 	ShrinkActor(aTargetGoody, hunger);
	}

	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateEatMushroom" );

	// error checking
	if( aTargetGoody == none )
		gotostate('statePatrol');
	
	// Make sure we are not going to move
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	if( aTargetGoody.IsA('Horklumps') )
	{
		PlaySoundEat();
	 	PlayAnim('eatloop');
 		FinishAnim();

		aTargetGoody.Destroy();
		aTargetGoody=none;

		DropGoodies(OBJECT_BEAN, Rand(2) + 1, location, CollisionHeight, false);
	}
	else
		gotostate('statePatrol');

	gotostate('statePatrol');
}

state statePickupTargetObject
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": statePickupTargetObject" );

	// Make sure we are not going to move
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// error checking
	if( aTargetGoody == none )
		gotostate('statePatrol');
	
	// if touch goody, but when to mushroom, do nothing
	if(aTargetGoody.IsA('Horklumps'))
		gotostate('statePatrol');

	// if this goody is taken already
	if( aTargetGoody.Owner != none )
		gotostate('statePatrol');

	// turn to goody, to look better, before picking it up
	TurnTo(aTargetGoody.location);

	GnomePickupObject(aTargetGoody);

	// taunt a bit		// ???
	PlayIdleAnim();		// ???
	FinishAnim();		// ???

	// if too close to harry, back up a bit
	vTargetDir = playerHarry.location - location;
	if( vsize(vTargetDir) < 50)
	{
		vNewLoc = location - 50 * vTargetDir / vsize(vTargetDir);
		gotostate( 'stateGotoHarry' );
	}

	// goto Idle now that we are done getting our object
	gotostate('statePatrol');
}

state stateHitWall
{
	begin:

	// Stop moving
	Acceleration = vect(0,0,0);
	Velocity	 = vect(0,0,0);

	// Turn his back to the wall
	LoopRunAnimNormal();
	TurnTo(vNewLoc);
	MoveTo(vNewLoc);
	desiredRotation.Yaw = Rotation.Yaw;

	// start patroling
	gotostate('statePatrol');
}

state stateDied
{
	begin:
	if( BOOL_DEBUG_AI ) playerHarry.ClientMessage("" $Name $": stateDied" );

	PlaySoundCrash();

	if( Rand(2) == 0)
		spawn(class'dustcloud01_tiny', self, ,location, rotation);
	else
		spawn(class'dustcloud02_small',self, ,location, rotation);

	DropGoodies(OBJECT_BEAN, Rand(3) + 1, location, 0, true);

	Destroy();
}

defaultproperties
{
     Lives=2
     bCurrentSoundOver=True
     bHasHome=True
	 MaxGoodies=15
     hunger=0.9375
     bAccurateThrowing=True
     GroundSpeed=150
     SightRadius=400
     PeripheralVision=1
     BaseEyeHeight=15
     EyeHeight=15
     eVulnerableToSpell=SPELL_Flipendo
     Mesh=SkeletalMesh'HPModels.skgnomeMesh'
     DrawScale=1.25
     AmbientGlow=75
     CollisionRadius=12
     CollisionHeight=20
     bThrownObjectDamage=True
}
