
// Class Name  : AragogWebAnchor
//
// Created on  : 06/26/2002
// Authored by : Janet Weddle
// 
// Description : AragogWebAnchor. The anchor points of Aragogs web. They report Aragog and when
//				 all of them are cut Aragog will fall
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class AragogWebAnchor extends HiddenHpawn;


// variables
var Aragog spider;

// editor variables
var() int iLocation;	// set this so the anchors are in order (0,1,2,3 etc). Used to know which is hit

var (VisualFX)ParticleFX		fxHitParticleEffect;			
var (VisualFX)class<ParticleFX>	fxHitParticleEffectClass;

var (VisualFX)ParticleFX		fxReactParticleEffect;			
var (VisualFX)class<ParticleFX>	fxReactParticleEffectClass;

var (VisualFX)ParticleFX		fxHitMeParticleEffect;			
var (VisualFX)class<ParticleFX>	fxHitMeParticleEffectClass;

var sound hitSound;

//**********************************************************************
function postBeginPlay()
{
	Super.postBeginPlay();

	foreach AllActors( class'Aragog', spider )
		break;

}

//**********************************************************************
function bool HandleSpellDiffindo( optional baseSpell spell, optional vector vHitLocation )
{

	eVulnerableToSpell = SPELL_None;

	// Tell Aragog that this object has been hit by a spell
	spider.AnchorHitBySpell(iLocation);

	gotoState('HitBySpell');

	return true;
//	return false;
}

state HitBySpell
{
	begin:
		
	hitSound = sound'HPSounds.Adv9Aragog.ss_Ara_incendiohit_0005';

	PlaySound( hitSound, SLOT_None, [Volume]RandRange(0.7, 1.0), [Radius]150000, [Pitch]RandRange(0.8, 1.0),, false );


	fxHitParticleEffect = spawn( fxHitParticleEffectClass,,,Location );
	fxReactParticleEffect = spawn( fxReactParticleEffectClass,,,Location );

	sleep(0.1);

	fxHitParticleEffect.ShutDown();
	fxReactParticleEffect.ShutDown();
	KillAttachedParticleFX(0.0);
 
	bHidden = true;
}

//**********************************************************************//

defaultproperties
{
	Mesh=SkeletalMesh'HPModels.skAragogWebBaseMesh'
	drawType=DT_MESH
	fxHitParticleEffectClass=class'HPParticle.WebFxBase'
	fxReactParticleEffectClass=class'HPParticle.WebDustBase'
	attachedParticleClass(0)=class'HPParticle.Diffindo_WebFx'
    DrawScale=1
    CollisionRadius=40
    CollisionHeight=110
	bCollideWorld=True
	bCollideActors=True
    bBlockActors=True
    bBlockPlayers=True

	eVulnerableToSpell=SPELL_Diffindo

	bHidden=False

}
