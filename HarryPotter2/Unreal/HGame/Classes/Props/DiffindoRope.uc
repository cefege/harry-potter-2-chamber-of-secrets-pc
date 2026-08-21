//===============================================================================
//  [DiffindoRope] 
//===============================================================================

class DiffindoRope extends HDiffindo;

var ParticleFX			fxShimmer;				// ParticleFX for the cutting the diffindo obj
var class<ParticleFX>	fxShimmerClass;			// class that the cutFX is.


function PreBeginPlay()
{
//	BoundingBox bbox;

	Super.PreBeginPlay();
	
	// SetCollision( collide actors, block actors, block players )
	SetCollision( true, true, true );
	
	// Create a simmer to highlight the rope
	if( fxShimmer == None )
	{
		// create the diffindo reaction
		fxShimmer = spawn( fxShimmerClass );	
		fxShimmer.SetLocation( location );
		fxShimmer.SetOwner( Self );
		
		// Set our sourceHeith,width and depth to be the same as the collision box
		fxShimmer.SourceHeight.Base	= CollisionHeight;
		fxShimmer.SourceWidth.Base	= CollisionWidth;
		fxShimmer.SourceDepth.Base	= CollisionRadius;
		
		// Compute the proper lifetime
//		bbox = GetCollisionBoundingBox();
//		Lifetime.Base = Speed.Base 
	}
}


event Destroyed()
{	
	// make sure our fx is shutdown
	if( fxShimmer != None )
		fxShimmer.Shutdown();

	Super.Destroyed();
}

defaultproperties
{
	fxShimmerClass=class'diffindo_RopeFx'

	// --- HDiffindo
	fxExplodeClass0=class'diffindo_hit'
	fxExplodeClass1=None
	fxExplodeClass2=None
	fxExplodeClass3=None

	fDiffindoTimer=2.0f
	fSingleCutTimer=0.2f

	DiffindoImpactSound=Sound'HPSounds.magic_sfx.DFO_hit_rope'
	DiffindoCutSound=Sound'HPSounds.magic_sfx.DFO_hit_rope'

	// --- 
	Mesh=SkeletalMesh'HProps.skDiffindoRope64Mesh'
	CollideType=CT_Box
}
