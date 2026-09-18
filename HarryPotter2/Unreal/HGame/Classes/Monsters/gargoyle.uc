// ***************************************************************
// Gargoyle Class - used in Lumos Animations and elsewhere...
class gargoyle extends HChar;

// ***************************************************************
// Class Variables
var() bool ignoreCutCam;
var() bool bMakeLumosInfinite;
 
// ***************************************************************
//	Functions and Events
function Trigger( actor Other, pawn EventInstigator )
{
	local actor A;
	gotostate('green');

	if( Event != '' )
		foreach AllActors( class 'Actor', A, Event )
			A.Trigger( self, self );
}

function bool HandleSpellLumos( optional baseSpell spell, optional vector vHitLocation )
{
	// If Lumos hits its target then reDirect the target to Harry's wand!
	// Have harry's wand turn on the lumos spell!
	baseWand(playerHarry.weapon).LumosTurnOn();
	
	if( bMakeLumosInfinite )
		baseWand(playerHarry.weapon).TheLumosLight.bInfiniteLumos = true;

	GotoState( 'green' );
	
	return true;
}
// ***************************************************************
//	Gargoyle States
auto state lookaround
{
  begin:
	enable('trigger');
	loopanim('red');
}


// The "Final" State...
state green
{
	function switchCamera()
	{
		local BaseCam c;
		/*
		local hpoint p1;
		local hpoint p2;

		foreach allActors(class'BaseCam', c)
			break;

		foreach allActors(class 'hpoint',p1)
			if(p1.Name=='hpoint0')
				break;

		foreach allActors(class 'hpoint',p2)
			if(p2.Name=='hpoint1')
				break;

		c.setCutCamera (p1, p2);
		*/
	}

	function returnCamera()
	{
		local BaseCam c;

		foreach allActors(class'BaseCam', c)
			break;

		//c.exitCutCamera();
	}

	function AnimEnd()
	{
		if( AnimSequence == 'green' )
			loopanim('red', 1.0, 2.0);
	}

begin:
	playanim('change2green', 1.0, 0.5);	// Play a little Head Shake...

	Sleep(1.0);
	//PlaySound(sound'HPSounds.critters2_sfx.gargoyle_eyes_growl');
	finishanim();

	loopanim('green', 1.0, 0.5);

	sleep(1);
	if( !ignorecutCam )
	{
		switchCamera();
		sleep (2);
		returnCamera();
	}
}

// **********************************************************************
//	Default Properties

defaultproperties
{
	bStatic=true
	ignoreCutCam=True
	Mesh=SkeletalMesh'HPModels.skgargoyleMesh'
	AmbientGlow=75
	CollisionRadius=50
	CollisionHeight=50
	eVulnerableToSpell=SPELL_Lumos
	
	bBlockCamera=True
}
