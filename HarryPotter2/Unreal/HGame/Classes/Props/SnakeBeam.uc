//===============================================================================
//  [skSnakeBeam] 
//===============================================================================

class SnakeBeam extends HProp;


defaultproperties
{
    Mesh=skSnakeBeamMesh
    DrawType=DT_Mesh
    bStatic=False

	bCollideActors=true
	bBlockActors=false
	bBlockPlayers=false
	bBlockCamera=false
	bCollideWorld=false

	AmbientGlow=200
	MultiSkins(0)=WetTexture'HPParticle.hp_fx.General.SnakeEyesWet'
	style=sty_translucent   
}

