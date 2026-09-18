
class BasiliskSpell expands HPawn;

var bool    bActive;

var vector  vMoveDir;
var vector  vStartPos;

var float   SpellDamageAmount;
var float   SpellInitialDrawScale;
var float   SpellEndDrawScale;
var float   SpellTravelDistance;
var float   SpellStartSpeed;
var float   SpellEndSpeed;

const NUM_SPELL_FX = 12;
//var class<ParticleFX>    SpellFXClass[6];
var vector               SpellFXOffset[12];
var ParticleFX           SpellFX[12];
var int                  NumSpellFX;

var int     Damage;

var float   fCollisionSize;

//************************************************************************************
function Init(
	vector Dir,
	float  DamageAmount,
	float  InitialDrawScale,
	float  EndDrawScale,
	float  TravelDistance,
	float  StartSpeed,
	float  EndSpeed,
	int    YawOffset
  )
{
	local rotator r;

	SpellDamageAmount = DamageAmount;
	SpellInitialDrawScale = InitialDrawScale;
	SpellEndDrawScale = EndDrawScale;
	SpellTravelDistance = TravelDistance;
	SpellStartSpeed = StartSpeed;
	SpellEndSpeed = EndSpeed;

	vStartPos = Location;

	r = rotator(Dir);// + Rot(-16384,0,0);
	r.Yaw += YawOffset;
	SetRotation( r + Rot(-16384,0,0) );
	Dir = vector(r);

	vMoveDir = Dir;
	Velocity = vMoveDir * StartSpeed;

	DrawScale = InitialDrawScale;

	//Given StartSpeed, EndSpeed, and TravelDistance, we can calculate a life span, and an accell
	LifeSpan = 2 * TravelDistance / (StartSpeed + EndSpeed);
	//a = (EndSpeed - StartSpeed) / LifeSpan;

	MakeSpellFX(class'HPParticle.Avifors_Wand', RandRange(0.75, 1.5) );
	MakeSpellFX(class'HPParticle.DeathTorch');
	MakeSpellFX(class'HPParticle.Aloh_Fly', RandRange(0.15, 0.35) );
	//MakeSpellFX(class'HPParticle.Avifors_Wand', RandRange(0.75, 1.5) );
	//MakeSpellFX(class'HPParticle.Incend_Fly', RandRange(0.15, 0.35));
	//MakeSpellFX(class'HPParticle.DeathTorch');
	//MakeSpellFX(class'HPParticle.Avifors_Wand', RandRange(0.75, 1.5) );
	//MakeSpellFX(class'HPParticle.DeathTorch');
	//MakeSpellFX(class'HPParticle.Aloh_Fly', RandRange(0.15, 0.35) );
	//MakeSpellFX(class'HPParticle.Avifors_Wand', RandRange(0.75, 1.5) );
	//MakeSpellFX(class'HPParticle.Incend_Fly', RandRange(0.15, 0.35));
	//MakeSpellFX(class'HPParticle.DeathTorch');

	RepositionSpellFXs();
}

//************************************************************************************
function MakeSpellFX(class<ParticleFX> fx, optional float scale)
{
	SpellFX[NumSpellFX] = ParticleFX( FancySpawn(fx) );
	SpellFX[NumSpellFX].SetRotation( Rotation );
	SpellFXOffset[NumSpellFX] = VRand();// * fCollisionSize;

	if( scale > 0 )
	{
		SpellFX[NumSpellFX].SourceWidth.Base *= scale;
		SpellFX[NumSpellFX].SourceWidth.Rand *= scale;
		SpellFX[NumSpellFX].SourceHeight.Base *= scale;
		SpellFX[NumSpellFX].SourceHeight.Rand *= scale;
		SpellFX[NumSpellFX].SourceDepth.Base *= scale;
		SpellFX[NumSpellFX].SourceDepth.Rand *= scale;
		SpellFX[NumSpellFX].SizeWidth.Base *= scale;
		SpellFX[NumSpellFX].SizeWidth.Rand *= scale;
		SpellFX[NumSpellFX].SizeLength.Base *= scale;
		SpellFX[NumSpellFX].SizeLength.Rand *= scale;
	}

	NumSpellFX++;
}

//************************************************************************************
function RepositionSpellFXs()
{
	local int count;

	for(count = 0; count < NUM_SPELL_FX; count++)
	{
		if( SpellFX[count] == none )
			break;

		SpellFX[ count ].SetLocation( Location  +  SpellFXOffset[count] * fCollisionSize * DrawScale );
	}
}

//************************************************************************************
function Tick(float dtime)
{
	local float d;
	local harry h;
	local float d2;

	//Set your velocity based on where you are
	d = VSize( Location - vStartPos );
	Velocity = vMoveDir * ((SpellEndSpeed - SpellStartSpeed) * d / SpellTravelDistance  +  SpellStartSpeed);

	//Set your DrawScale based on where you are
	DrawScale = (SpellEndDrawScale - SpellInitialDrawScale) * d / SpellTravelDistance  +  SpellInitialDrawScale;

	//Look for Harry Explicitly
	if( bActive )
	{
		h = harry(level.PlayerHarryActor);
		d = h.CollisionRadius + fCollisionSize*DrawScale;  // <=-- our magic number for distance...
		if( VSize(h.Location + vec(0,0,h.BaseEyeHeight) - Location)  <  d )
		{
			harry(level.PlayerHarryActor).ClientMessage("Collide with basil spell");
			playerHarry.TakeDamage( Damage, self, Location, vect(0,0,0), 'BasiliskSpell' );
		}
	}

	RepositionSpellFXs();

//Opacity = RandRange(0,1);
//	Wideness = RandRange(100,156);
}

//************************************************************************************
//function HitWall( vector HitNormal, actor HitWall )
//{
//	playerHarry.ClientMessage("HitWall:");
//}

function touch( actor other )
{
	//playerHarry.ClientMessage("Basil Spell touch:"$other);

	if( other.IsA('HPawn') || other.IsA('Projectile') )
		return;

	if( Other.bBlockActors )
		Destroy();
}

function bump( actor other )
{
	playerHarry.ClientMessage("BasilSpell bump:"$other);
	touch( other );
	
}

//************************************************************************************
event Destroyed()
{
	local int count;

	super.Destroyed();

	for(count = 0; count < NUM_SPELL_FX; count++)
		if( SpellFX[count] != none )
			SpellFX[count].ShutDown();
}

//************************************************************************************
defaultproperties
{
	bBlockActors=false
	bBlockPlayers=false
	bCollideActors=true
	bCollideWorld=true

	bAlignBottom=false

	Damage=10
	//Speed=170.0000
	//LifeSpan=10.000000
	//MomentumTransfer=0
	//ImpactSound=Sound'HPSounds.magic_sfx.spell_hit'
	//RemoteRole=ROLE_SimulatedProxy
	
	//Mesh=Mesh'skBucketMesh'

	Mesh=SkeletalMesh'HProps.skSnakeRayMesh'
	AmbientGlow=200
	MultiSkins(0)=WetTexture'HPParticle.hp_fx.General.SnakeEyesWet'

	//CollisionRadius=40
	//CollisionHeight=80
	
	//bStatic=false
	//style=STY_Modulated
	style=sty_translucent   

	//	LightType=LT_Steady
	//	LightEffect=LE_NonIncidence
	//	LightBrightness=201
	//	LightHue=165
	//	LightSaturation=72
	//	LightRadius=10
	//DrawScale=0.3  
	bUnlit=True
//	bUnlit=false
//Opacity=0.5
	//bMeshCurvy=False
	CollisionRadius=10
	CollisionHeight=10
	//bFixedRotationDir=True
	//bNetTemporary=false
	bRotateToDesired=false
	//bFixedRotationDir=true

	Physics=PHYS_Projectile


	fCollisionSize=17
}
