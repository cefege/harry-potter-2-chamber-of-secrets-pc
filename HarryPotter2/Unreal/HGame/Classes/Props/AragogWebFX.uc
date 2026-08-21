
// Class Name  : AragogWebFX
//
// Created on  : 09/03/2002
// Authored by : Janet Weddle
// 
// Description : AragogWebFX. The big effects when Aragog's web is finally falling
// 
// Copyright 2002, Amaze Entertainment, all rights reserved.
// --------------------------------------------------------------------------------------------

class AragogWebFX extends HiddenHpawn;


var (VisualFX)ParticleFX		fxHitParticleEffect;			
var (VisualFX)class<ParticleFX>	fxHitParticleEffectClass;

var (VisualFX)ParticleFX		fxReactParticleEffect;			
var (VisualFX)class<ParticleFX>	fxReactParticleEffectClass;

function Trigger( Actor Other, Pawn EventInstigator )
{
	gotoState('stateTriggered');
}

state auto stateIdle
{
	begin:
}

state stateTriggered
{

begin:

	fxHitParticleEffect = spawn( fxHitParticleEffectClass,,,Location );
	fxReactParticleEffect = spawn( fxReactParticleEffectClass,,,Location );
		
	sleep(0.2);

	fxHitParticleEffect.ShutDown();
	fxReactParticleEffect.ShutDown();

	bHidden = true;

}

defaultproperties
{
     Style=STY_Translucent
     Mesh=SkeletalMesh'HPModels.skAragogWebMesh'
 
	 fxHitParticleEffectClass=class'HPParticle.WebFxAragog
     fxReactParticleEffectClass=class'HPParticle.WebDustAragog'
}
