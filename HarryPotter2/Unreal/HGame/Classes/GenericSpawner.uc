class GenericSpawner expands HPawn;

const MAX_SPAWNED_GOODIES = 8;

struct Animations
{
	var()	name  		Opening;			// Anim to play when spelled to open
	var()	name  		Closing;			// Anim to play when closing
	var()	name  		Start;				// Idle anim in the beginning
	var()	name  		End;				// Idle anim in the end
};

struct Sounds
{
	var()	sound  		Opening;			// Sound to play when opening
	var()	sound  		Closing;			// Sound to play when closing
	var()	sound  		Spawning;			// Sound to play when spawning something
};

var()	class<Actor>	GoodieToSpawn[8]; 	// What goodies to spawn

struct MaxMin
{
	var() int Max;
	var() int Min;
};

var()	Animations		Anims;
var()	Sounds			Snds;
var()	MaxMin			Limits;

var()	name  			StartBone; 			// Bone name for where goodies spawn from
var()	vector			StartPos;			// Positional offset for where goodies spawn from
var()	vector			StartVel;			// Absolute start velocity (if (0,0,0), use random velocity).

//var()	GenericDebris	BaseDebris;			// Debris to generate when open (if any)

var() class<ParticleFX>	BaseParticles;		// Particles to generate when open (if any)

var()	float			BaseDelay;			// Time delays before spawn
var()	float			GoodieDelay; 		// Time delays between each spawn

var()	int				Lives;				// How many times you can use it

var()	bool			bDestroable;		// Should be destoyed at the end

var		vector			BaseParticlePos;

var		int				HowManyObjectsToSpawn;
var		int				RandomNums;
var		int				CurrentNum;
var		ESpellType		eVulnerableToSpellSaved;

auto state stateStart
{
	begin:

		if(	Anims.Start != '')
			LoopAnim(Anims.Start);
}

state stateEnd
{
	begin:

		if(bDestroable)
			Destroy();

		if(	Anims.End != '')
			LoopAnim(Anims.End);
}

state stateHitBySpell
{
	begin:

		// set spell type to none, to prevent casting during this state
		eVulnerableToSpell = SPELL_None;

		if(Lives > 0)
			Lives--;

		// finish idle animation
		FinishAnim();

		if(	Anims.Opening != '')
		{
			if( Snds.Opening != none )
				PlaySound(Snds.Opening, SLOT_None);

			PlayAnim(Anims.Opening);
			Sleep(BaseDelay);
			FinishAnim();
		}

		if(	BaseParticles != none )
		{
			FindBaseParticlePos();
			spawn(BaseParticles, [spawnlocation] BaseParticlePos);
		}

		// spawn..............................................
		if(Limits.Min >= Limits.Max)
			RandomNums = Limits.Min;
		else
			RandomNums = RandRange(Limits.Min, Limits.Max);

		for (CurrentNum = 0; CurrentNum < RandomNums; CurrentNum++)
		{
			Sleep(GoodieDelay);
			SpawnObject();
		}

		if( Snds.Spawning != none )
			PlaySound(Snds.Spawning, SLOT_Misc);

		if(	Lives > 0 )
		{
			if(Anims.Closing != '') 
			{
				if( Snds.Closing != none )
					PlaySound(Snds.Closing, SLOT_None);

				PlayAnim(Anims.Closing);
				FinishAnim();
			}

			// reset spell type back
			eVulnerableToSpell = eVulnerableToSpellSaved;

			gotostate('stateStart');

		}
		else
		{
			// could not do anything anymore in future
			eVulnerableToSpell = SPELL_None;
			gotostate('stateEnd');
		}
}

function PostBeginPlay()
{
	local int i;

	Super.PostBeginPlay();

	// save spell type. 
	eVulnerableToSpellSaved = eVulnerableToSpell;

	HowManyObjectsToSpawn = 0;
	for( i = 0; i < MAX_SPAWNED_GOODIES; i++)
	{
		if( GoodieToSpawn[i] == none )
			break;
	}

	HowManyObjectsToSpawn = i;

	// if there is nothing to spawn, do not allow to cast at
	if(	HowManyObjectsToSpawn <= 0 )
		eVulnerableToSpell = SPELL_None;

}

function FindBaseParticlePos()
{
	local vector dir;
	local int bNum;

	// set 'dir' using start position first
	dir = StartPos;
	dir = dir >> rotation;

	// if there is start bone, reset 'dir'
	if(	StartBone != '')
	{
		bNum = BoneNumber( StartBone );

		if(bNum >= 0)
		{
			dir = BonePos(StartBone);
			dir = dir - location;
		}
	}

	BaseParticlePos = dir + location;
}

function SpawnObject()
{
	local vector dir, vel;
	local actor newspawn;
	local int bNum;

	local vector	v, n;

	local float	length, angle;

	if(	HowManyObjectsToSpawn <= 0 )
		return;

	// normal vector looking in direction of the object
	n = vector(rotation); 
	n.z = 0;

	// looking for the random vector, pointing in about the same direction as vector n
	while(true)
	{
		angle	= RandRange(0.0000, 6.2832);
		v.x = cos(angle);
		v.y = sin(angle);
		v.z = 0;

		// to prevent infinite loop
		if((n.x == 0.0) && (n.y == 0.0))
			break;

		if((v dot n) / VSize2D(n) > 0.7)	// about 45 degrees in each direction
			break;
	}

	// spread it a little bit
	length	= RandRange(50, 100);
	vel.x	= length * cos(angle);
	vel.y	= length * sin(angle);
	vel.z	= 100 + FRand() * 100;	

	// set 'dir' using start position first
	dir = StartPos;
	dir = dir >> rotation;

	// if there is start bone, reset 'dir'
	if(	StartBone != '')
	{
		bNum = BoneNumber( StartBone );

		if(bNum >= 0)
		{
			dir = BonePos(StartBone);
			dir = dir - location;
		}
	}

	dir = dir + location;

	newSpawn=Spawn(GoodieToSpawn[Rand(HowManyObjectsToSpawn)], [spawnlocation] dir);

	// If StartVel = vec(0,0,0), use random velocity)
	if( (StartVel.X == 0) && (StartVel.Y == 0) && (StartVel.Z == 0) )
		newSpawn.Velocity = vel;
	else
		newSpawn.Velocity = StartVel;

	newSpawn.SetPhysics( PHYS_Falling );

	switch( Rand(3) )
	{
		case 0:   	spawn(class'Spawn_Flash_1', [spawnlocation] dir);   break;
		case 1:   	spawn(class'Spawn_Flash_2', [spawnlocation] dir);   break;
		case 2:   	spawn(class'Spawn_Flash_3', [spawnlocation] dir);   break;
	}

	switch( Rand(3) )
	{
		case 0:   	PlaySound( sound'HPSounds.Magic_sfx.spawn_bean01');   break;
		case 1:   	PlaySound( sound'HPSounds.Magic_sfx.spawn_bean02');   break;
		case 2:   	PlaySound( sound'HPSounds.Magic_sfx.spawn_bean03');   break;
	}
}

function bool HandleSpellFlipendo( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellFlipendo( spell, vHitLocation);

	gotostate('stateHitBySpell');
	
	return true; // true == create spell effects
}

function bool HandleSpellAlohomora( optional baseSpell spell, optional vector vHitLocation )
{ 
	Super.HandleSpellAlohomora( spell, vHitLocation );

	gotostate('stateHitBySpell');
	
	return true; // true == create spell effects
}

function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellDiffindo( spell, vHitLocation );

	gotostate('stateHitBySpell');
	
	return true; // true == create spell effects
}

function bool HandleSpellEcto( optional baseSpell spell, optional vector vHitLocation )
{ 
	Super.HandleSpellEcto( spell, vHitLocation );

	gotostate('stateHitBySpell');
	
	return true; // true == create spell effects
}

function bool HandleSpellLumos( optional baseSpell spell, optional vector vHitLocation )
{ 
	Super.HandleSpellLumos( spell, vHitLocation );

	gotostate('stateHitBySpell');
	
	return true; // true == create spell effects
}

function bool HandleSpellRictusempra( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellRictusempra( spell, vHitLocation );

	gotostate('stateHitBySpell');
	
	return true; // true == create spell effects
}

function bool HandleSpellSkurge( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellSkurge( spell, vHitLocation );

	gotostate('stateHitBySpell');
	
	return true; // true == create spell effects
}

function bool HandleSpellSpongify( optional baseSpell spell, optional vector vHitLocation )
{
	Super.HandleSpellSpongify( spell, vHitLocation );

	gotostate('stateHitBySpell');
	
	return true; // true == create spell effects
}

defaultproperties
{
    DrawType=DT_Mesh
	Mesh=SkeletalMesh'HPModels.skCigarBoxMesh'

	Physics=PHYS_Falling

	CollideType=CT_Shape
	bCollideWorld=true

	Lives=1
	eVulnerableToSpell=SPELL_Flipendo

	Anims=(Opening=Open,Closing=Close,Start=Start,End=End)

	StartPos=(X=0,Y=0,Z=40)
	StartVel=(X=0,Y=0,Z=0)

	Limits=(Min=2,Max=6)
}
