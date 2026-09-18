class ActorShadow extends Decal;

#exec TEXTURE IMPORT FILE=Textures\Shadow_1.PCX NAME=ShadowT

var() float MoveThreshold;
var() float MaxShadowDist;
var() float ShadowSizeFactor;
var() bool bOriented;

var vector OldOwnerLocation;
var rotator OldOwnerRotation;
var bool bUpdated;

function AttachToSurface()
{
}

function Tick( float DeltaTime )
{
	// If not drawn last frame, bye-bye.
	if( !bUpdated )
	{
		DetachDecal();
		OldOwnerLocation = vec(0,0,0);
	}
	bUpdated = false;
}

event Update(Actor L)
{
	local vector Loc;
	local vector ShadowDir;
	local vector Extent;
	local float Movement;

	if( Owner == none || Owner.bDeleteMe || Owner.Style == STY_Translucent )
		return;

	bUpdated = true;
	Opacity = Owner.Opacity;
	Loc = Owner.BonePos( Owner.BoneName(0) );
	Loc.Z += 1;				// Move above ground, just in case.
	Movement = VSize(OldOwnerLocation - Loc);
	if( Movement <= MoveThreshold )
	{
		if( !bOriented || OldOwnerRotation == Owner.Rotation )
			return;
	}

	DetachDecal();

	if ( L == None )
		ShadowDir = vec(0,0,-1);
	else
	{
		ShadowDir = Normal(Loc - L.Location);
		if ( ShadowDir.Z > 0 )
			ShadowDir.Z *= -1;
	}

	// Set scale to match collision radius.
	Extent = Owner.GetRenderExtent();
	DrawScale = max(Extent.X, Extent.Y) * ShadowSizeFactor / max(Texture.USize, Texture.VSize);

	// Set initial location/rotation so AttachDecal traces in correct direction.
	SetLocation(Loc);
	SetRotation(rotator(-ShadowDir));
	AttachDecal(MaxShadowDist, vector(Owner.Rotation));
	OldOwnerLocation = Loc;
	OldOwnerRotation = Owner.Rotation;
}

defaultproperties
{
     MultiDecalLevel=1
     Style=STY_Modulated
     Texture=texture'ShadowT'
	 MoveThreshold=1
	 MaxShadowDist=8192
	 ShadowSizeFactor=0.75
	 bOriented=false;
}
