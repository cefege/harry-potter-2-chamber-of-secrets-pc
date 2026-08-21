class SecretAreaMarker expands HiddenHPawn;

#exec Texture Import File=Textures\Secret.pcx Name=SecretTexture Mips=Off Flags=2

var() bool		bUseCollision;

var   bool		bFound;
var   sound		FoundSound;

function OnFound()
{
	if( !bFound )
	{
		cm("Secret Area Found!");
		if( FoundSound != none )
			PlaySound( FoundSound );
	}
	bFound = true;
}

function PreBeginPlay()
{
	if( !bUseCollision )
		SetCollision( false, false, false );
}

function touch(actor other)
{
	if( bUseCollision && other.IsA('PlayerPawn') )
	{
		OnFound();
	}
}

function Trigger( actor Other, pawn EventInstigator )
{
	// If we receive an event then consider it the same as finding the secret
	OnFound();
}

defaultproperties
{
	bUseCollision=true

	bPersistent=true
    bCollideActors=true
	Texture=Texture'SecretTexture'
	
	FoundSound=sound'HPSounds.Magic_sfx.Dueling_MIM_buildup'
}