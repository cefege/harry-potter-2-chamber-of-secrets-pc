//===============================================================================
// #79	Oswald Beamish
//===============================================================================

class  WCBeamish extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardBeamishTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Beamishsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardBeamishBigTexture FILE=TEXTURES\menu\Folio\Cards\Beamishbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Oswald Beamish";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=79
    skin(0)=Texture'HProps.skins.WizardCardBeamishTex0'
	textureBig=Texture'WizCardBeamishBigTexture'
	strDescriptionId="WizCard_0073"
}
